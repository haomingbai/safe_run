use crate::constants::{ARTIFACTS_DIR, EVENTS_FILE, GENESIS_HASH, REPORT_FILE};
use crate::launch::assemble_launch_plan;
use crate::model::{
    PreparedRun, RunArtifacts, RunState, RunnerControlRequest, RunnerRuntime, RuntimeContext,
};
use crate::utils::{derive_run_id, write_json_file};
use serde_json::json;
use sr_common::{ErrorItem, SR_RUN_001};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

/// Prepare runtime directories and initial artifacts for a run.
pub(crate) fn prepare_run(
    runtime: &RunnerRuntime,
    request: RunnerControlRequest,
) -> Result<PreparedRun, ErrorItem> {
    validate_runtime_context(&request.runtime_context)?;

    let compile_bundle = request.compile_bundle;
    let workdir_path = PathBuf::from(request.runtime_context.workdir.trim());
    let artifacts_dir_path = create_workdir(&workdir_path)?;

    write_firecracker_config(&workdir_path, &compile_bundle)?;
    write_runtime_context(&workdir_path, &request.runtime_context)?;
    initialize_event_stream(&artifacts_dir_path)?;

    let run_id = derive_run_id(&workdir_path);
    let launch_plan = assemble_launch_plan(&run_id, &workdir_path, &compile_bundle, runtime);

    Ok(PreparedRun {
        run_id,
        state: RunState::Prepared,
        runtime_context: request.runtime_context,
        artifacts: RunArtifacts {
            log: EVENTS_FILE.to_string(),
            report: REPORT_FILE.to_string(),
        },
        event_stream: vec![EVENTS_FILE.to_string()],
        launch_plan,
        workdir_path,
        artifacts_dir_path,
        last_event_hash: GENESIS_HASH.to_string(),
    })
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
