use crate::constants::{EVENT_RUN_CLEANED, STAGE_CLEANUP};
use crate::event::write_event;
use crate::model::{PreparedRun, RunState};
use serde_json::json;
use sr_common::{ErrorItem, SR_RUN_001};
use std::fs;
use std::path::Path;

/// Release temporary runner resources and emit `run.cleaned`.
/// The cleanup keeps evidence artifacts and report inputs while removing transient runtime files.
pub(crate) fn cleanup_run(prepared: &mut PreparedRun) -> Result<(), ErrorItem> {
    remove_file_if_exists(
        prepared.runtime_context_path().as_path(),
        "cleanup.runtimeContext",
    )?;
    remove_file_if_exists(prepared.vm_pid_path().as_path(), "cleanup.vmPid")?;
    remove_file_if_exists(prepared.api_socket_path().as_path(), "cleanup.apiSocket")?;
    fs::write(prepared.cleanup_marker_path(), "cleanup completed").map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "cleanup.marker",
            format!("failed to write cleanup marker: {err}"),
        )
    })?;
    write_event(
        prepared,
        STAGE_CLEANUP,
        EVENT_RUN_CLEANED,
        json!({
            "state": state_label(prepared.state),
            "cleanupMarker": prepared.cleanup_marker_path()
        }),
    )
}

fn remove_file_if_exists(path: &Path, error_path: &str) -> Result<(), ErrorItem> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            error_path,
            format!("failed to inspect cleanup target: {err}"),
        )
    })?;
    if metadata.is_dir() {
        return Err(ErrorItem::new(
            SR_RUN_001,
            error_path,
            "cleanup target is a directory, expected a file",
        ));
    }
    fs::remove_file(path).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            error_path,
            format!("failed to remove cleanup target file: {err}"),
        )
    })
}

fn state_label(state: RunState) -> &'static str {
    match state {
        RunState::Prepared => "prepared",
        RunState::Running => "running",
        RunState::Finished => "finished",
        RunState::Failed => "failed",
    }
}
