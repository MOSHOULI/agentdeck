//! 记忆管理命令（M3）
//!
//! 定时采集默认关闭；用户明确启用后立即采集，之后每小时一轮；
//! 每轮采集后把 auto_inject 记忆回写 `~/.agentdeck/memory/<agent>.md`。

use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use aisync_core::memory::{self, MemoryEntry};
use serde::Serialize;
use tauri::State;

/// 定时采集间隔（1 小时）
const COLLECT_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// 定时采集状态（供 UI 展示「上次/下次采集时间」）
#[derive(Default)]
pub struct CollectionState {
    inner: Arc<Mutex<CollectionStateInner>>,
    wake: Arc<(Mutex<WorkerSignal>, Condvar)>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[derive(Default)]
struct WorkerSignal {
    enabled: bool,
    stopped: bool,
    generation: u64,
    active_cancel: Option<Arc<AtomicBool>>,
}

#[derive(Default)]
struct CollectionStateInner {
    enabled: bool,
    last_run_at: Option<i64>,
    last_error: Option<String>,
    next_run_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionStatus {
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub last_error: Option<String>,
    pub next_run_at: Option<i64>,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 采集指定 agent 的记忆
#[tauri::command]
pub async fn collect_memory(agent_id: String) -> Result<Vec<MemoryEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        memory::collect(&agent_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Memory collection worker failed: {e}"))?
}

/// 搜索记忆（FTS5）
#[tauri::command]
pub fn search_memory(query: String) -> Result<Vec<MemoryEntry>, String> {
    memory::search(&query).map_err(|e| e.to_string())
}

/// 注入记忆到指定 agent
#[tauri::command]
pub fn inject_memory(agent_id: String, memory_ids: Vec<String>) -> Result<(), String> {
    memory::inject(&agent_id, &memory_ids).map_err(|e| e.to_string())
}

/// 切换记忆自动注入标签
#[tauri::command]
pub fn toggle_auto_inject(memory_id: String, enabled: bool) -> Result<(), String> {
    memory::toggle_auto_inject(&memory_id, enabled).map_err(|e| e.to_string())
}

/// 最近记忆列表
#[tauri::command]
pub fn recent_memories(agent_id: String) -> Result<Vec<MemoryEntry>, String> {
    memory::recent(&agent_id, 50).map_err(|e| e.to_string())
}

/// 查询定时采集状态
#[tauri::command]
pub fn memory_collection_status(state: State<'_, CollectionState>) -> CollectionStatus {
    // 采集线程若异常 panic 会毒化锁，降级返回空状态而不是连带崩溃
    let inner = state
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    CollectionStatus {
        enabled: inner.enabled,
        last_run_at: inner.last_run_at,
        last_error: inner.last_error.clone(),
        next_run_at: inner.next_run_at,
    }
}

#[tauri::command]
pub fn set_memory_collection_enabled(
    enabled: bool,
    state: State<'_, CollectionState>,
) -> Result<(), String> {
    state.set_enabled(enabled).map_err(|e| e.to_string())
}

/// The worker owns shared state only and cannot collect until explicitly enabled.
impl CollectionState {
    pub fn load_settings(&self) {
        match memory::load_collection_enabled() {
            Ok(enabled) => self.update_enabled(enabled),
            Err(e) => {
                self.update_enabled(false);
                self.inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .last_error = Some(e.to_string());
            }
        }
    }

    fn set_enabled(&self, enabled: bool) -> aisync_core::Result<()> {
        // Serialize persistence and publication: concurrent toggles cannot invert disk/runtime state.
        let (lock, condvar) = &*self.wake;
        let mut signal = lock.lock().unwrap_or_else(|e| e.into_inner());
        memory::save_collection_enabled(enabled)?;
        self.publish_enabled(&mut signal, enabled);
        condvar.notify_all();
        Ok(())
    }

    fn update_enabled(&self, enabled: bool) {
        let (lock, condvar) = &*self.wake;
        let mut signal = lock.lock().unwrap_or_else(|e| e.into_inner());
        self.publish_enabled(&mut signal, enabled);
        condvar.notify_all();
    }

    fn publish_enabled(&self, signal: &mut WorkerSignal, enabled: bool) {
        if signal.enabled != enabled {
            signal.generation = signal.generation.wrapping_add(1);
            if let Some(cancel) = &signal.active_cancel {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        signal.enabled = enabled;
        let mut status = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        status.enabled = enabled;
        status.next_run_at = if enabled && !signal.stopped {
            Some(unix_now())
        } else {
            None
        };
    }

    pub fn start(&self) -> std::io::Result<()> {
        self.start_with(memory::collect_all_and_inject_controlled)
    }

    fn start_with(
        &self,
        collect: impl Fn(&memory::CollectionControl) -> aisync_core::Result<memory::CollectReport>
        + Send
        + 'static,
    ) -> std::io::Result<()> {
        let mut worker = self.worker.lock().unwrap_or_else(|e| e.into_inner());
        if worker.is_some() {
            return Ok(());
        }
        let inner = self.inner.clone();
        let wake = self.wake.clone();
        *worker = Some(
            std::thread::Builder::new()
                .name("memory-collector".into())
                .spawn(move || {
                    let (lock, condvar) = &*wake;
                    loop {
                        let mut signal = lock.lock().unwrap_or_else(|e| e.into_inner());
                        signal = condvar
                            .wait_while(signal, |s| !s.enabled && !s.stopped)
                            .unwrap_or_else(|e| e.into_inner());
                        if signal.stopped {
                            break;
                        }
                        let generation = signal.generation;
                        let cancel = Arc::new(AtomicBool::new(false));
                        signal.active_cancel = Some(cancel.clone());
                        drop(signal);
                        let control =
                            memory::CollectionControl::new(cancel, Duration::from_secs(300));
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            collect(&control)
                        }));
                        let last_error = match result {
                            Ok(Ok(report)) if report.errors.is_empty() => None,
                            Ok(Ok(report)) => Some(report.errors.join("; ")),
                            Ok(Err(e)) => Some(e.to_string()),
                            Err(_) => {
                                Some("Memory collector panicked; collection will retry".into())
                            }
                        };
                        let now = unix_now();
                        let mut signal = lock.lock().unwrap_or_else(|e| e.into_inner());
                        signal.active_cancel = None;
                        {
                            let mut status = inner.lock().unwrap_or_else(|e| e.into_inner());
                            status.last_run_at = Some(now);
                            status.last_error = last_error;
                            status.next_run_at = if signal.enabled && !signal.stopped {
                                Some(if generation == signal.generation {
                                    now + COLLECT_INTERVAL.as_secs() as i64
                                } else {
                                    now
                                })
                            } else {
                                None
                            };
                        }
                        drop(
                            condvar
                                .wait_timeout_while(signal, COLLECT_INTERVAL, |s| {
                                    !s.stopped && s.enabled && s.generation == generation
                                })
                                .unwrap_or_else(|e| e.into_inner()),
                        );
                    }
                })?,
        );
        Ok(())
    }

    pub fn stop(&self) {
        let (lock, condvar) = &*self.wake;
        let mut signal = lock.lock().unwrap_or_else(|e| e.into_inner());
        signal.stopped = true;
        if let Some(cancel) = &signal.active_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        condvar.notify_all();
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .next_run_at = None;
    }
}

impl Drop for CollectionState {
    fn drop(&mut self) {
        self.stop();
        // Filesystem operations are cooperative; never block UI shutdown on I/O.
        self.worker.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(state: &CollectionState) {
        state.worker.lock().unwrap().take().unwrap().join().unwrap();
    }

    #[test]
    fn startup_disabled_and_stop_never_touch_sources() {
        let state = CollectionState::default();
        assert!(!state.inner.lock().unwrap().enabled);
        state
            .start_with(|_| panic!("disabled worker must not collect"))
            .unwrap();
        state.stop();
        join(&state);
        assert!(state.inner.lock().unwrap().last_run_at.is_none());
    }

    #[test]
    fn enable_wakes_worker_and_disable_cancels_active_pass() {
        let state = CollectionState::default();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        state
            .start_with(move |_| {
                started_tx.send(()).unwrap();
                finish_rx.recv().unwrap();
                Ok(memory::CollectReport {
                    agents: vec![],
                    total: 0,
                    auto_injected: vec![],
                    errors: vec![],
                })
            })
            .unwrap();
        state.update_enabled(true);
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let cancel = state.wake.0.lock().unwrap().active_cancel.clone().unwrap();
        state.update_enabled(false);
        assert!(cancel.load(Ordering::Relaxed));
        assert!(!state.inner.lock().unwrap().enabled);
        assert!(state.inner.lock().unwrap().next_run_at.is_none());
        finish_tx.send(()).unwrap();
        state.stop();
        join(&state);
    }
}
