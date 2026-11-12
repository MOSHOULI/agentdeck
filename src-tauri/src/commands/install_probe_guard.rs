//! Process-local Codex list-probe suppression, separate from inspection references.
//! std-only so synchronization tests can run without a desktop SDK.
use std::sync::Mutex;

#[cfg(any(target_os = "linux", windows))]
pub const SUPPRESSED: &str = "Codex 安装保护未通过检查，已暂停此条目的主动探测。刷新或切换页面不会为此条目执行 Codex 或 npm；重启不代表安全恢复。未清理残留。";
#[cfg(not(any(target_os = "linux", windows)))]
pub const SUPPRESSED: &str = "当前平台未提供经过验证的 Codex 持久安装保护；此桌面条目的主动探测与安装已暂停，未执行 npm 或 Codex。";
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

    // Explicit metadata-only recovery may inspect persistent evidence while
    // process-local suppression is armed. Poison/contention still fail closed.
    pub fn recovery<T>(
        &self,
        action: impl FnOnce() -> Result<T, String>,
        release: bool,
    ) -> Result<T, String> {
        let mut blocked = self
            .blocked
            .try_lock()
            .map_err(|_| SUPPRESSED.to_string())?;
        let result = action();
        if release && result.is_ok() {
            *blocked = false;
        }
        result
    }

    // Arm eagerly so panic/uncertain unwind fails closed. Only a returned error
    // before the core's typed mutation-start callback restores the clear state.
    // Prior suppression refuses installation; only verified explicit recovery can release it.
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
    fn recovery_inspection_and_failure_cannot_clear_but_verified_success_can() {
        let gate = ProbeGuard::new();
        assert!(
            gate.install::<()>("codex", |start| {
                start();
                Err("failed".into())
            })
            .is_err()
        );
        gate.recovery(|| Ok(()), false).unwrap();
        assert!(!gate.inspect("codex", || true, || false));
        assert!(gate.recovery::<()>(|| Err("stale".into()), true).is_err());
        assert!(!gate.inspect("codex", || true, || false));
        gate.recovery(|| Ok(()), true).unwrap();
        assert!(gate.inspect("codex", || true, || false));
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
