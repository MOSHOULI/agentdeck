//! Process-local Codex list-probe suppression, separate from inspection references.
//! std-only so synchronization tests can run without a desktop SDK.
use std::sync::Mutex;

pub const SUPPRESSED: &str = "本进程中 Codex 安装完成状态未确认，已暂停自动版本探测。刷新或切换页面不会为此条目执行 Codex 或 npm；未清理残留。此保护不跨应用重启。";
pub static CODEX: ProbeGuard = ProbeGuard::new();

pub struct ProbeGuard {
    blocked: Mutex<bool>,
}
impl Default for ProbeGuard {
    fn default() -> Self {
        Self::new()
    }
}
impl ProbeGuard {
    pub const fn new() -> Self {
        Self {
            blocked: Mutex::new(false),
        }
    }

    // Keep the gate throughout the active probe, not only the state check.
    // Contention and poisoning return no active evidence and invoke no probe.
    pub fn inspect<T>(
        &self,
        id: &str,
        active: impl FnOnce() -> T,
        passive: impl FnOnce() -> T,
    ) -> T {
        if id != "codex" {
            return active();
        }
        let Ok(blocked) = self.blocked.try_lock() else {
            return passive();
        };
        if *blocked { passive() } else { active() }
    }

    pub fn preview<T>(
        &self,
        id: &str,
        action: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        self.inspect(id, action, || Err(SUPPRESSED.into()))
    }

    // Arm eagerly so panic/uncertain unwind fails closed. Only a returned error
    // before the core's typed mutation-start callback restores the clear state.
    // Prior suppression refuses the action entirely; there is no reset endpoint.
    pub fn install<T>(
        &self,
        id: &str,
        action: impl FnOnce(&mut dyn FnMut()) -> Result<T, String>,
    ) -> Result<T, String> {
        if id != "codex" {
            return action(&mut || {});
        }
        let mut blocked = self
            .blocked
            .try_lock()
            .map_err(|_| SUPPRESSED.to_string())?;
        if *blocked {
            return Err(SUPPRESSED.into());
        }
        *blocked = true;
        let mut started = false;
        let result = action(&mut || started = true);
        if result.is_ok() || !started {
            *blocked = false;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn preflight_rejection_and_success_remain_probeable() {
        let gate = ProbeGuard::new();
        assert!(
            gate.install::<()>("codex", |_| Err("stale revision".into()))
                .is_err()
        );
        assert!(gate.inspect("codex", || true, || false));
        gate.install("codex", |started| {
            started();
            Ok(())
        })
        .unwrap();
        assert!(gate.inspect("codex", || true, || false));
    }

    #[test]
    fn attempted_failure_never_resets_through_preview_or_later_submission() {
        let gate = ProbeGuard::new();
        assert!(
            gate.install::<()>("codex", |started| {
                started();
                Err("uncertain".into())
            })
            .is_err()
        );
        for _ in 0..3 {
            assert!(!gate.inspect("codex", || panic!("active probe"), || false));
            assert!(
                gate.preview::<()>("codex", || panic!("new preview"))
                    .is_err()
            );
            assert!(
                gate.install::<()>("codex", |_| panic!("later install"))
                    .is_err()
            );
        }
        assert!(gate.inspect("other", || true, || false));
        gate.preview("other", || Ok(())).unwrap();
        gate.install("other", |_| Ok(())).unwrap();
    }

    #[test]
    fn active_probe_excludes_install_until_probe_finishes() {
        let gate = Arc::new(ProbeGuard::new());
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let thread = {
            let (gate, entered, release) = (gate.clone(), entered.clone(), release.clone());
            std::thread::spawn(move || {
                gate.inspect(
                    "codex",
                    || {
                        entered.wait();
                        release.wait();
                        true
                    },
                    || false,
                )
            })
        };
        entered.wait();
        assert!(
            gate.install::<()>("codex", |_| panic!("must not install during probe"))
                .is_err()
        );
        assert!(!gate.inspect("codex", || panic!("concurrent probe"), || false));
        release.wait();
        assert!(thread.join().unwrap());
        gate.install("codex", |started| {
            started();
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn install_excludes_probes_and_completed_old_results_are_redacted() {
        let gate = Arc::new(ProbeGuard::new());
        let prior = gate.inspect("codex", || "old installed evidence", || "passive");
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let thread = {
            let (gate, entered, release) = (gate.clone(), entered.clone(), release.clone());
            std::thread::spawn(move || {
                gate.install::<()>("codex", |started| {
                    started();
                    entered.wait();
                    release.wait();
                    Err("wrong root".into())
                })
            })
        };
        entered.wait();
        assert!(!gate.inspect("codex", || panic!("probe during install"), || false));
        release.wait();
        assert!(thread.join().unwrap().is_err());
        assert_eq!(gate.inspect("codex", || prior, || "passive"), "passive");
    }

    #[test]
    fn panic_before_or_after_dispatch_and_poison_fail_closed() {
        for dispatch in [false, true] {
            let gate = ProbeGuard::new();
            assert!(
                std::panic::catch_unwind(|| gate.install::<()>("codex", |started| {
                    if dispatch {
                        started();
                    }
                    panic!("synthetic unwind");
                }))
                .is_err()
            );
            assert!(!gate.inspect("codex", || panic!("poisoned probe"), || false));
            assert!(
                gate.preview::<()>("codex", || panic!("poisoned preview"))
                    .is_err()
            );
            assert!(
                gate.install::<()>("codex", |_| panic!("poisoned install"))
                    .is_err()
            );
        }
    }
}
