use nix::mount::{mount, umount2, MntFlags, MsFlags};
use sr_common::{ErrorItem, SR_RUN_101};
use sr_compiler::{MountPlan, MountPlanEntry};
use std::fs;
use std::path::Path;

use crate::rollback::rollback_mounts;

/// Mount operation adapter used by `MountExecutor`.
pub trait MountApplier {
    fn apply(&self, entry: &MountPlanEntry) -> Result<(), String>;
}

/// Rollback adapter used by `MountExecutor` when partial apply fails.
pub trait MountRollbacker {
    fn rollback(&self, entry: &MountPlanEntry) -> Result<(), String>;
}

/// Event callbacks emitted around mount decision/apply/reject steps.
pub trait MountEventHooks {
    fn on_validated(&mut self, entry: &MountPlanEntry) -> Result<(), ErrorItem>;
    fn on_applied(&mut self, entry: &MountPlanEntry) -> Result<(), ErrorItem>;
    fn on_rejected(&mut self, entry: &MountPlanEntry, message: &str) -> Result<(), ErrorItem>;
}

#[allow(dead_code)]
pub struct NoopMountEventHooks;

impl MountEventHooks for NoopMountEventHooks {
    fn on_validated(&mut self, _entry: &MountPlanEntry) -> Result<(), ErrorItem> {
        Ok(())
    }

    fn on_applied(&mut self, _entry: &MountPlanEntry) -> Result<(), ErrorItem> {
        Ok(())
    }

    fn on_rejected(&mut self, _entry: &MountPlanEntry, _message: &str) -> Result<(), ErrorItem> {
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopMountApplier;

impl MountApplier for NoopMountApplier {
    fn apply(&self, _entry: &MountPlanEntry) -> Result<(), String> {
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopMountRollbacker;

impl MountRollbacker for NoopMountRollbacker {
    fn rollback(&self, _entry: &MountPlanEntry) -> Result<(), String> {
        Ok(())
    }
}

pub struct MountExecutor {
    applier: Box<dyn MountApplier>,
    rollbacker: Box<dyn MountRollbacker>,
}

impl MountExecutor {
    /// Create a mount executor from apply/rollback adapters.
    pub fn new<A: MountApplier + 'static, R: MountRollbacker + 'static>(
        applier: A,
        rollbacker: R,
    ) -> Self {
        Self {
            applier: Box::new(applier),
            rollbacker: Box::new(rollbacker),
        }
    }

    pub fn apply_plan(&self, plan: &MountPlan) -> Result<Vec<MountPlanEntry>, ErrorItem> {
        if !plan.enabled {
            return Ok(Vec::new());
        }

        let mut applied = Vec::with_capacity(plan.mounts.len());
        for entry in &plan.mounts {
            if let Err(message) = self.applier.apply(entry) {
                let rollback = rollback_mounts(self.rollbacker.as_ref(), &applied);
                let mut detail = format!(
                    "failed to apply mount {} -> {}: {}",
                    entry.source, entry.target, message
                );
                if let Err(rollback_err) = rollback {
                    detail = format!("{detail}; rollback failed: {}", rollback_err.message);
                }
                return Err(mount_error("mount.apply", detail));
            }
            applied.push(entry.clone());
        }

        Ok(applied)
    }

    pub fn apply_plan_with_hooks(
        &self,
        plan: &MountPlan,
        hooks: &mut dyn MountEventHooks,
    ) -> Result<Vec<MountPlanEntry>, ErrorItem> {
        if !plan.enabled {
            return Ok(Vec::new());
        }

        let mut applied = Vec::with_capacity(plan.mounts.len());
        for entry in &plan.mounts {
            hooks.on_validated(entry)?;
            if let Err(message) = self.applier.apply(entry) {
                hooks.on_rejected(entry, &message)?;
                let rollback = rollback_mounts(self.rollbacker.as_ref(), &applied);
                let mut detail = format!(
                    "failed to apply mount {} -> {}: {}",
                    entry.source, entry.target, message
                );
                if let Err(rollback_err) = rollback {
                    detail = format!("{detail}; rollback failed: {}", rollback_err.message);
                }
                return Err(mount_error("mount.apply", detail));
            }
            applied.push(entry.clone());
            hooks.on_applied(entry)?;
        }

        Ok(applied)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemMountApplier;

impl MountApplier for SystemMountApplier {
    fn apply(&self, entry: &MountPlanEntry) -> Result<(), String> {
        let source = Path::new(&entry.source);
        let target = Path::new(&entry.target);
        if !target.exists() {
            fs::create_dir_all(target).map_err(|err| err.to_string())?;
        }
        mount(
            Some(source),
            target,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .map_err(|err: nix::Error| err.to_string())?;

        if entry.read_only {
            mount(
                Some(source),
                target,
                None::<&str>,
                MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
                None::<&str>,
            )
            .map_err(|err: nix::Error| err.to_string())?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemMountRollbacker;

impl MountRollbacker for SystemMountRollbacker {
    fn rollback(&self, entry: &MountPlanEntry) -> Result<(), String> {
        let target = Path::new(&entry.target);
        umount2(target, MntFlags::MNT_DETACH).map_err(|err: nix::Error| err.to_string())?;
        Ok(())
    }
}

fn mount_error(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_RUN_101, path, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone)]
    struct RecordingApplier {
        calls: Rc<RefCell<Vec<String>>>,
        fail_on: Option<String>,
    }

    impl MountApplier for RecordingApplier {
        fn apply(&self, entry: &MountPlanEntry) -> Result<(), String> {
            self.calls.borrow_mut().push(entry.target.clone());
            if let Some(target) = &self.fail_on {
                if &entry.target == target {
                    return Err("apply failed".to_string());
                }
            }
            Ok(())
        }
    }

    #[derive(Clone)]
    struct RecordingRollbacker {
        calls: Rc<RefCell<Vec<String>>>,
    }

    impl MountRollbacker for RecordingRollbacker {
        fn rollback(&self, entry: &MountPlanEntry) -> Result<(), String> {
            self.calls.borrow_mut().push(entry.target.clone());
            Ok(())
        }
    }

    fn sample_plan() -> MountPlan {
        MountPlan {
            enabled: true,
            mounts: vec![
                MountPlanEntry {
                    source: "/var/lib/safe-run/input".to_string(),
                    target: "/data/input".to_string(),
                    read_only: true,
                },
                MountPlanEntry {
                    source: "/var/lib/safe-run/cache".to_string(),
                    target: "/data/cache".to_string(),
                    read_only: true,
                },
                MountPlanEntry {
                    source: "/var/lib/safe-run/output".to_string(),
                    target: "/data/output".to_string(),
                    read_only: true,
                },
            ],
        }
    }

    #[test]
    fn apply_plan_runs_in_order_without_rollback() {
        let apply_calls = Rc::new(RefCell::new(Vec::new()));
        let rollback_calls = Rc::new(RefCell::new(Vec::new()));
        let executor = MountExecutor::new(
            RecordingApplier {
                calls: apply_calls.clone(),
                fail_on: None,
            },
            RecordingRollbacker {
                calls: rollback_calls.clone(),
            },
        );

        let applied = executor.apply_plan(&sample_plan()).expect("apply plan");
        assert_eq!(applied.len(), 3);
        assert_eq!(
            *apply_calls.borrow(),
            vec!["/data/input", "/data/cache", "/data/output"]
        );
        assert!(rollback_calls.borrow().is_empty());
    }

    #[test]
    fn apply_plan_rolls_back_in_reverse_on_failure() {
        let apply_calls = Rc::new(RefCell::new(Vec::new()));
        let rollback_calls = Rc::new(RefCell::new(Vec::new()));
        let executor = MountExecutor::new(
            RecordingApplier {
                calls: apply_calls.clone(),
                fail_on: Some("/data/output".to_string()),
            },
            RecordingRollbacker {
                calls: rollback_calls.clone(),
            },
        );

        let err = executor
            .apply_plan(&sample_plan())
            .expect_err("apply should fail");
        assert_eq!(err.code, SR_RUN_101);
        assert_eq!(err.path, "mount.apply");
        assert_eq!(
            *apply_calls.borrow(),
            vec!["/data/input", "/data/cache", "/data/output"]
        );
        assert_eq!(*rollback_calls.borrow(), vec!["/data/cache", "/data/input"]);
    }
}
