use crate::constants::{
    ARTIFACTS_DIR, EVENTS_FILE, EVENT_COMPILE, GENESIS_HASH, REPORT_FILE, STAGE_COMPILE,
};
use crate::event::write_event;
use crate::launch::assemble_launch_plan;
use crate::model::{
    PreparedRun, RunArtifacts, RunState, RunnerControlRequest, RunnerRuntime, RuntimeContext,
};
use crate::utils::{derive_run_id, write_json_file};
use serde_json::json;
use sr_common::{ErrorItem, SR_RUN_001, SR_RUN_002};
use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

/// Prepare runtime directories and initial artifacts for a run.
pub(crate) fn prepare_run(
    runtime: &RunnerRuntime,
    request: RunnerControlRequest,
) -> Result<PreparedRun, ErrorItem> {
    validate_runtime_context(&request.runtime_context)?;

    let mut compile_bundle = request.compile_bundle;
    let workdir_path = PathBuf::from(request.runtime_context.workdir.trim());
    let artifacts_dir_path = create_workdir(&workdir_path)?;

    materialize_firecracker_artifacts(&workdir_path, &mut compile_bundle)?;
    write_firecracker_config(&workdir_path, &compile_bundle)?;
    write_runtime_context(&workdir_path, &request.runtime_context)?;
    initialize_event_stream(&artifacts_dir_path)?;

    let run_id = derive_run_id(&workdir_path);
    let launch_plan = assemble_launch_plan(&run_id, &workdir_path, &compile_bundle, runtime);

    let mut prepared = PreparedRun {
        run_id,
        state: RunState::Prepared,
        runtime_context: request.runtime_context,
        artifacts: RunArtifacts {
            log: EVENTS_FILE.to_string(),
            report: REPORT_FILE.to_string(),
        },
        event_stream: vec![EVENTS_FILE.to_string()],
        launch_plan,
        mount_plan: compile_bundle.mount_plan.clone(),
        evidence_plan: compile_bundle.evidence_plan.clone(),
        workdir_path,
        artifacts_dir_path,
        last_event_hash: GENESIS_HASH.to_string(),
    };

    write_compile_event_if_enabled(&mut prepared, &compile_bundle)?;

    Ok(prepared)
}

fn validate_runtime_context(runtime_context: &RuntimeContext) -> Result<(), ErrorItem> {
    if runtime_context.timeout_sec == 0 {
        return Err(ErrorItem::new(
            SR_RUN_001,
            "runtimeContext.timeoutSec",
            "timeoutSec must be greater than 0",
        ));
    }

    let workdir_raw = runtime_context.workdir.trim();
    if workdir_raw.is_empty() {
        return Err(ErrorItem::new(
            SR_RUN_001,
            "runtimeContext.workdir",
            "workdir cannot be empty",
        ));
    }

    if let Some(interval_ms) = runtime_context.sample_interval_ms {
        if interval_ms == 0 {
            return Err(ErrorItem::new(
                SR_RUN_001,
                "runtimeContext.sampleIntervalMs",
                "sampleIntervalMs must be greater than 0",
            ));
        }
    }

    if let Some(cgroup_path) = runtime_context.cgroup_path.as_deref() {
        if cgroup_path.trim().is_empty() {
            return Err(ErrorItem::new(
                SR_RUN_001,
                "runtimeContext.cgroupPath",
                "cgroupPath cannot be empty when provided",
            ));
        }
    }
    Ok(())
}

fn create_workdir(workdir_path: &Path) -> Result<PathBuf, ErrorItem> {
    let artifacts_dir_path = workdir_path.join(ARTIFACTS_DIR);
    fs::create_dir_all(&artifacts_dir_path).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "prepare.workdir",
            format!("failed to create run working directories: {err}"),
        )
    })?;
    Ok(artifacts_dir_path)
}

fn write_firecracker_config(
    workdir_path: &Path,
    compile_bundle: &sr_compiler::CompileBundle,
) -> Result<(), ErrorItem> {
    write_json_file(
        &workdir_path.join(crate::constants::FIRECRACKER_CONFIG_FILE),
        &compile_bundle.firecracker_config,
        "prepare.firecrackerConfig",
    )
}

fn write_runtime_context(
    workdir_path: &Path,
    runtime_context: &RuntimeContext,
) -> Result<(), ErrorItem> {
    write_json_file(
        &workdir_path.join(crate::constants::RUNTIME_CONTEXT_FILE),
        &json!({
            "workdir": runtime_context.workdir,
            "timeoutSec": runtime_context.timeout_sec,
            "sampleIntervalMs": runtime_context.sample_interval_ms,
            "cgroupPath": runtime_context.cgroup_path
        }),
        "prepare.runtimeContext",
    )
}

fn initialize_event_stream(artifacts_dir_path: &Path) -> Result<(), ErrorItem> {
    let event_log_path = artifacts_dir_path.join(EVENTS_FILE);
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&event_log_path)
        .map_err(|err| {
            ErrorItem::new(
                SR_RUN_001,
                "prepare.events",
                format!("failed to initialize event stream file: {err}"),
            )
        })?;
    Ok(())
}

fn materialize_firecracker_artifacts(
    workdir_path: &Path,
    compile_bundle: &mut sr_compiler::CompileBundle,
) -> Result<(), ErrorItem> {
    let mut config = compile_bundle.firecracker_config.clone();
    let kernel_raw = json_string_at(
        &config,
        "/boot-source/kernel_image_path",
        "prepare.artifacts.kernel",
    )?;
    let (rootfs_pointer, rootfs_raw) = rootfs_pointer_and_value(&config)?;

    let kernel_target = materialize_artifact_path(workdir_path, &kernel_raw, "kernel")?;
    let rootfs_target = materialize_artifact_path(workdir_path, &rootfs_raw, "rootfs")?;

    set_json_string(
        &mut config,
        "/boot-source/kernel_image_path",
        path_for_config(workdir_path, &kernel_target),
        "prepare.artifacts.kernel",
    )?;
    set_json_string(
        &mut config,
        rootfs_pointer,
        path_for_config(workdir_path, &rootfs_target),
        "prepare.artifacts.rootfs",
    )?;

    compile_bundle.firecracker_config = config;
    Ok(())
}

fn rootfs_pointer_and_value(
    config: &serde_json::Value,
) -> Result<(&'static str, String), ErrorItem> {
    if let Ok(raw) = json_string_at(config, "/rootfs/path", "prepare.artifacts.rootfs") {
        return Ok(("/rootfs/path", raw));
    }
    let raw = json_string_at(config, "/drives/0/path", "prepare.artifacts.rootfs")?;
    Ok(("/drives/0/path", raw))
}

fn materialize_artifact_path(
    workdir_path: &Path,
    raw: &str,
    label: &str,
) -> Result<PathBuf, ErrorItem> {
    let source = resolve_artifact_source(workdir_path, raw, label)?;
    let target = target_artifact_path(workdir_path, raw, &source, label)?;
    copy_artifact_if_needed(&source, &target, label)?;
    Ok(target)
}

fn resolve_artifact_source(
    workdir_path: &Path,
    raw: &str,
    label: &str,
) -> Result<PathBuf, ErrorItem> {
    let raw_path = Path::new(raw);
    if raw_path.is_absolute() {
        return Ok(raw_path.to_path_buf());
    }

    let cwd = env::current_dir().map_err(|err| {
        ErrorItem::new(
            SR_RUN_002,
            format!("prepare.artifacts.{label}"),
            format!("failed to resolve current dir: {err}"),
        )
    })?;
    let candidate = cwd.join(raw_path);
    if candidate.exists() {
        return Ok(candidate);
    }

    let workdir_candidate = workdir_path.join(raw_path);
    if workdir_candidate.exists() {
        return Ok(workdir_candidate);
    }

    Err(ErrorItem::new(
        SR_RUN_002,
        format!("prepare.artifacts.{label}"),
        format!(
            "artifact path '{raw}' not found relative to '{}' or '{}'",
            cwd.display(),
            workdir_path.display()
        ),
    ))
}

fn target_artifact_path(
    workdir_path: &Path,
    raw: &str,
    source: &Path,
    label: &str,
) -> Result<PathBuf, ErrorItem> {
    if raw.starts_with('/') {
        let filename = source.file_name().ok_or_else(|| {
            ErrorItem::new(
                SR_RUN_002,
                format!("prepare.artifacts.{label}"),
                format!("artifact path '{}' has no filename", source.display()),
            )
        })?;
        return Ok(workdir_path.join(ARTIFACTS_DIR).join(filename));
    }
    Ok(workdir_path.join(raw))
}

fn copy_artifact_if_needed(source: &Path, target: &Path, label: &str) -> Result<(), ErrorItem> {
    let metadata = fs::metadata(source).map_err(|err| {
        ErrorItem::new(
            SR_RUN_002,
            format!("prepare.artifacts.{label}"),
            format!("failed to inspect artifact '{}': {err}", source.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(ErrorItem::new(
            SR_RUN_002,
            format!("prepare.artifacts.{label}"),
            format!("artifact path '{}' is not a file", source.display()),
        ));
    }
    if source == target {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            ErrorItem::new(
                SR_RUN_002,
                format!("prepare.artifacts.{label}"),
                format!(
                    "failed to create artifact directory '{}': {err}",
                    parent.display()
                ),
            )
        })?;
    }
    fs::copy(source, target).map_err(|err| {
        ErrorItem::new(
            SR_RUN_002,
            format!("prepare.artifacts.{label}"),
            format!(
                "failed to copy artifact from '{}' to '{}': {err}",
                source.display(),
                target.display()
            ),
        )
    })?;
    Ok(())
}

fn path_for_config(workdir_path: &Path, path: &Path) -> String {
    path.strip_prefix(workdir_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn json_string_at(
    value: &serde_json::Value,
    pointer: &str,
    error_path: &str,
) -> Result<String, ErrorItem> {
    value
        .pointer(pointer)
        .and_then(|item| item.as_str())
        .map(|raw| raw.to_string())
        .ok_or_else(|| {
            ErrorItem::new(
                SR_RUN_002,
                error_path,
                format!("missing required field {pointer}"),
            )
        })
}

fn set_json_string(
    value: &mut serde_json::Value,
    pointer: &str,
    new_value: String,
    error_path: &str,
) -> Result<(), ErrorItem> {
    value
        .pointer_mut(pointer)
        .and_then(|item| {
            *item = serde_json::Value::String(new_value);
            Some(())
        })
        .ok_or_else(|| {
            ErrorItem::new(
                SR_RUN_002,
                error_path,
                format!("missing required field {pointer}"),
            )
        })
}

fn write_compile_event_if_enabled(
    prepared: &mut PreparedRun,
    compile_bundle: &sr_compiler::CompileBundle,
) -> Result<(), ErrorItem> {
    if !compile_bundle.evidence_plan.enabled {
        return Ok(());
    }
    if !compile_bundle
        .evidence_plan
        .events
        .iter()
        .any(|event| event == EVENT_COMPILE)
    {
        return Ok(());
    }
    write_event(
        prepared,
        STAGE_COMPILE,
        EVENT_COMPILE,
        json!({"status": "ok"}),
    )
}
