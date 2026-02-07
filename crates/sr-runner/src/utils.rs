use sr_common::{ErrorItem, SR_RUN_001};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Persist a JSON payload to disk with standard SR-RUN-001 error mapping.
pub(crate) fn write_json_file(
    path: &Path,
    value: &serde_json::Value,
    error_path: &str,
) -> Result<(), ErrorItem> {
    let content = serde_json::to_string_pretty(value).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            error_path,
            format!("failed to serialize json payload: {err}"),
        )
    })?;
    fs::write(path, content).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            error_path,
            format!("failed to write json file: {err}"),
        )
    })
}

/// Derive a stable run identifier from the workdir name or a timestamp fallback.
pub(crate) fn derive_run_id(workdir: &Path) -> String {
    if let Some(dir_name) = workdir.file_name().and_then(|name| name.to_str()) {
        let candidate = dir_name.trim();
        if !candidate.is_empty() {
            return candidate.to_string();
        }
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("sr-{}", now.as_secs())
}
