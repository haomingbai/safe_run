use sr_common::{ErrorItem, SR_RUN_101};
use sr_compiler::MountPlanEntry;

use crate::mount_executor::MountRollbacker;

/// Roll back applied mounts in reverse order.
/// Error mapping: rollback failures -> `SR-RUN-101` with `mount.rollback` path.
pub fn rollback_mounts(
    rollbacker: &dyn MountRollbacker,
    applied: &[MountPlanEntry],
) -> Result<(), ErrorItem> {
    for entry in applied.iter().rev() {
        if let Err(message) = rollbacker.rollback(entry) {
            return Err(run_mount_error(
                "mount.rollback",
                format!(
                    "failed to rollback mount {} -> {}: {}",
                    entry.source, entry.target, message
                ),
            ));
        }
    }
    Ok(())
}

fn run_mount_error(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_RUN_101, path, message)
}
