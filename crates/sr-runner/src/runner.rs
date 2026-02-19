use crate::cleanup::cleanup_run;
use crate::constants::{
    EVENT_MOUNT_APPLIED, EVENT_MOUNT_REJECTED, EVENT_MOUNT_VALIDATED, EVENT_NETWORK_PLAN_GENERATED,
    EVENT_NETWORK_RULE_APPLIED, EVENT_NETWORK_RULE_CLEANUP_FAILED, EVENT_NETWORK_RULE_HIT,
    EVENT_NETWORK_RULE_RELEASED, EVENT_RUN_FAILED, EVENT_RUN_PREPARED, EVENT_VM_STARTED,
    STAGE_CLEANUP, STAGE_LAUNCH, STAGE_MOUNT, STAGE_PREPARE,
};
use crate::event::write_event;
use crate::model::{
    LaunchPlan, MonitorResult, PreparedRun, RunState, RunnerControlRequest, RunnerControlResponse,
    RunnerRuntime,
};
use crate::monitor::monitor_run;
use crate::mount_executor::{
    MountEventHooks, MountExecutor, SystemMountApplier, SystemMountRollbacker,
};
use crate::network_lifecycle::{NetworkLifecycle, SystemNetworkLifecycle};
use crate::prepare::prepare_run;
use serde_json::json;
use sr_common::{ErrorItem, SR_RUN_001, SR_RUN_002, SR_RUN_101, SR_RUN_201, SR_RUN_202};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Runner {
    runtime: RunnerRuntime,
    mount_executor: MountExecutor,
    network_lifecycle: Box<dyn NetworkLifecycle>,
}

impl Default for Runner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner {
    /// Create a runner with default Firecracker/jailer executables.
    pub fn new() -> Self {
        Self {
            runtime: RunnerRuntime::default(),
            mount_executor: MountExecutor::new(SystemMountApplier, SystemMountRollbacker),
            network_lifecycle: Box::new(SystemNetworkLifecycle::default()),
        }
    }

    /// Create a runner with explicitly provided runtime binaries.
    pub fn with_runtime(runtime: RunnerRuntime) -> Self {
        Self {
            runtime,
            mount_executor: MountExecutor::new(SystemMountApplier, SystemMountRollbacker),
            network_lifecycle: Box::new(SystemNetworkLifecycle::default()),
        }
    }

    /// Create a runner with a custom mount executor (used for tests).
    pub fn with_mount_executor(runtime: RunnerRuntime, mount_executor: MountExecutor) -> Self {
        Self {
            runtime,
            mount_executor,
            network_lifecycle: Box::new(SystemNetworkLifecycle::default()),
        }
    }

    /// Create a runner with custom mount and network lifecycle adapters (used for tests).
    pub fn with_mount_and_network_lifecycle<N: NetworkLifecycle + 'static>(
        runtime: RunnerRuntime,
        mount_executor: MountExecutor,
        network_lifecycle: N,
    ) -> Self {
        Self {
            runtime,
            mount_executor,
            network_lifecycle: Box::new(network_lifecycle),
        }
    }

    /// Create a runner with a custom network lifecycle adapter (used for tests).
    pub fn with_network_lifecycle<N: NetworkLifecycle + 'static>(
        runtime: RunnerRuntime,
        network_lifecycle: N,
    ) -> Self {
        Self {
            runtime,
            mount_executor: MountExecutor::new(SystemMountApplier, SystemMountRollbacker),
            network_lifecycle: Box::new(network_lifecycle),
        }
    }

    pub fn runtime(&self) -> &RunnerRuntime {
        &self.runtime
    }

    /// Prepare a run by creating the workdir and runtime artifacts.
    pub fn prepare(&self, request: RunnerControlRequest) -> Result<PreparedRun, ErrorItem> {
        prepare_run(&self.runtime, request)
    }

    /// Launch a prepared run, emitting events and returning the initial response.
    pub fn launch(&self, prepared: &mut PreparedRun) -> Result<RunnerControlResponse, ErrorItem> {
        ensure_prepared_state(prepared)?;

        write_event(
            prepared,
            STAGE_PREPARE,
            EVENT_RUN_PREPARED,
            json!({
                "workdir": prepared.runtime_context.workdir,
                "timeoutSec": prepared.runtime_context.timeout_sec
            }),
        )?;

        if let Err(err) = ensure_launch_binaries(&prepared.launch_plan) {
            self.run_cleanup_on_failure(
                prepared,
                "launch.preflight",
                err.code.clone(),
                err.message.clone(),
            );
            return Err(err);
        }

        let mount_plan = prepared.mount_plan.clone();
        let mut hooks = MountEventWriter { prepared };
        if let Err(err) = self
            .mount_executor
            .apply_plan_with_hooks(&mount_plan, &mut hooks)
        {
            self.run_cleanup_on_failure(
                prepared,
                "mount.apply",
                err.code.clone(),
                err.message.clone(),
            );
            return Err(err);
        }

        if let Err(err) = self.apply_network_if_needed(prepared) {
            self.run_cleanup_on_failure(
                prepared,
                "launch.network.apply",
                err.code.clone(),
                err.message.clone(),
            );
            return Err(err);
        }

        let vm_pid = match self.spawn(&prepared.launch_plan.jailer) {
            Ok(pid) => pid,
            Err(err) => {
                self.run_cleanup_on_failure(
                    prepared,
                    "launch.spawn",
                    SR_RUN_002.to_string(),
                    format!("failed to launch jailer/firecracker: {err}"),
                );
                return Err(ErrorItem::new(
                    SR_RUN_002,
                    "launch",
                    format!("failed to launch jailer/firecracker: {err}"),
                ));
            }
        };

        if let Err(err) = persist_vm_pid(prepared, vm_pid) {
            self.run_cleanup_on_failure(
                prepared,
                "launch.vmPid",
                err.code.clone(),
                err.message.clone(),
            );
            return Err(err);
        }

        prepared.state = RunState::Running;
        if let Err(err) = write_event(
            prepared,
            STAGE_LAUNCH,
            EVENT_VM_STARTED,
            json!({
                "pid": vm_pid,
                "launcher": prepared.launch_plan.jailer.program
            }),
        ) {
            self.run_cleanup_on_failure(
                prepared,
                "launch.vmStarted",
                err.code.clone(),
                err.message.clone(),
            );
            return Err(err);
        }

        Ok(RunnerControlResponse {
            run_id: prepared.run_id.clone(),
            state: prepared.state,
            artifacts: prepared.artifacts.clone(),
            event_stream: prepared.event_stream.clone(),
        })
    }

    /// Monitor a running run until completion or timeout.
    pub fn monitor(&self, prepared: &mut PreparedRun) -> Result<MonitorResult, ErrorItem> {
        monitor_run(prepared)
    }

    /// Clean transient resources and emit cleanup evidence.
    pub fn cleanup(&self, prepared: &mut PreparedRun) -> Result<(), ErrorItem> {
        let network_hits_error = self.collect_network_hits_if_applied(prepared).err();
        let network_release_error = self.release_network_if_applied(prepared).err();
        let cleanup_error = cleanup_run(prepared).err();

        if let Some(err) = network_hits_error {
            prepared.state = RunState::Failed;
            let mut message = err.message.clone();
            if let Some(release_err) = network_release_error {
                message = format!(
                    "{message}; network release also failed: {}",
                    release_err.message
                );
            }
            if let Some(local_cleanup_err) = cleanup_error {
                message = format!(
                    "{message}; local cleanup also failed: {}",
                    local_cleanup_err.message
                );
            }
            let _ = write_event(
                prepared,
                STAGE_CLEANUP,
                EVENT_RUN_FAILED,
                json!({
                    "reason": "cleanup.network.hit",
                    "errorCode": err.code,
                    "message": message
                }),
            );
            return Err(ErrorItem::new(err.code, err.path, message));
        }

        if let Some(err) = network_release_error {
            prepared.state = RunState::Failed;
            let mut message = err.message.clone();
            if let Some(local_cleanup_err) = cleanup_error {
                message = format!(
                    "{message}; local cleanup also failed: {}",
                    local_cleanup_err.message
                );
            }
            let _ = write_event(
                prepared,
                STAGE_CLEANUP,
                EVENT_RUN_FAILED,
                json!({
                    "reason": "cleanup.network.release",
                    "errorCode": err.code,
                    "message": message
                }),
            );
            return Err(ErrorItem::new(err.code, err.path, message));
        }

        if let Some(err) = cleanup_error {
            prepared.state = RunState::Failed;
            let error_code = err.code.clone();
            let message = err.message.clone();
            let _ = write_event(
                prepared,
                STAGE_CLEANUP,
                EVENT_RUN_FAILED,
                json!({
                    "reason": "cleanup.failure",
                    "errorCode": error_code,
                    "message": message
                }),
            );
            return Err(err);
        }

        Ok(())
    }

    pub(crate) fn spawn(&self, command: &crate::model::CommandSpec) -> std::io::Result<u32> {
        let child = Command::new(&command.program).args(&command.args).spawn()?;
        let pid = child.id();
        Ok(pid)
    }

    pub(crate) fn run_cleanup_on_failure(
        &self,
        prepared: &mut PreparedRun,
        reason: &str,
        error_code: String,
        message: String,
    ) {
        let mut final_message = message;
        if let Err(network_err) = self.release_network_if_applied(prepared) {
            final_message = format!(
                "{final_message}; network release failed during failure cleanup: {}",
                network_err.message
            );
        }
        prepared.state = RunState::Failed;
        let _ = fs::write(prepared.cleanup_marker_path(), "cleanup invoked");
        let _ = write_event(
            prepared,
            STAGE_CLEANUP,
            EVENT_RUN_FAILED,
            json!({
                "reason": reason,
                "errorCode": error_code,
                "message": final_message
            }),
        );
    }

    fn apply_network_if_needed(&self, prepared: &mut PreparedRun) -> Result<(), ErrorItem> {
        let Some(network_plan) = prepared.network_plan.clone() else {
            return Ok(());
        };

        write_network_event_if_enabled(
            prepared,
            STAGE_LAUNCH,
            EVENT_NETWORK_PLAN_GENERATED,
            json!({
                "mode": "allowlist",
                "tap": network_plan.tap.name,
                "table": network_plan.nft.table,
                "chains": network_plan.nft.chains,
                "rulesTotal": network_plan.nft.rules.len()
            }),
        )?;

        let applied = self
            .network_lifecycle
            .apply(&prepared.run_id, &network_plan)
            .map_err(|err| run_network_apply_error(err.path, err.message))?;
        for rule in &applied.rules {
            write_network_event_if_enabled(
                prepared,
                STAGE_LAUNCH,
                EVENT_NETWORK_RULE_APPLIED,
                json!({
                    "tap": applied.tap_name,
                    "table": applied.table,
                    "chain": rule.chain,
                    "protocol": rule.protocol,
                    "target": rule.target,
                    "port": rule.port
                }),
            )?;
        }
        prepared.applied_network = Some(applied);
        Ok(())
    }

    fn collect_network_hits_if_applied(&self, prepared: &mut PreparedRun) -> Result<(), ErrorItem> {
        let Some(applied) = prepared.applied_network.as_ref() else {
            return Ok(());
        };

        let hits = self
            .network_lifecycle
            .sample_rule_hits(applied)
            .map_err(|err| run_network_apply_error(err.path, err.message))?;
        let tap_name = applied.tap_name.clone();
        let table = applied.table.clone();
        for hit in hits {
            if hit.allowed_hits == 0 && hit.blocked_hits == 0 {
                continue;
            }
            write_network_event_if_enabled(
                prepared,
                STAGE_CLEANUP,
                EVENT_NETWORK_RULE_HIT,
                json!({
                    "tap": tap_name,
                    "table": table,
                    "chain": hit.chain,
                    "protocol": hit.protocol,
                    "target": hit.target,
                    "port": hit.port,
                    "allowedHits": hit.allowed_hits,
                    "blockedHits": hit.blocked_hits
                }),
            )?;
        }
        Ok(())
    }

    fn release_network_if_applied(&self, prepared: &mut PreparedRun) -> Result<(), ErrorItem> {
        let Some(applied) = prepared.applied_network.take() else {
            return Ok(());
        };

        if let Err(err) = self.network_lifecycle.release(&applied) {
            let message = format!("failed to release network resources: {}", err.message);
            let _ = write_network_event_if_enabled(
                prepared,
                STAGE_CLEANUP,
                EVENT_NETWORK_RULE_CLEANUP_FAILED,
                json!({
                    "tap": applied.tap_name,
                    "table": applied.table,
                    "chains": applied.chains,
                    "rulesTotal": applied.rules.len(),
                    "errorCode": SR_RUN_202,
                    "message": message
                }),
            );
            return Err(run_network_release_error(err.path, message));
        }

        write_network_event_if_enabled(
            prepared,
            STAGE_CLEANUP,
            EVENT_NETWORK_RULE_RELEASED,
            json!({
                "tap": applied.tap_name,
                "table": applied.table,
                "chains": applied.chains,
                "rulesTotal": applied.rules.len()
            }),
        )?;
        Ok(())
    }
}

fn mount_event_enabled(prepared: &PreparedRun, event_type: &str) -> bool {
    prepared.evidence_plan.enabled
        && prepared
            .evidence_plan
            .events
            .iter()
            .any(|event| event == event_type)
}

fn network_event_enabled(prepared: &PreparedRun, event_type: &str) -> bool {
    prepared.evidence_plan.enabled
        && prepared
            .evidence_plan
            .events
            .iter()
            .any(|event| event == event_type)
}

fn write_network_event_if_enabled(
    prepared: &mut PreparedRun,
    stage: &str,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), ErrorItem> {
    if !network_event_enabled(prepared, event_type) {
        return Ok(());
    }
    write_event(prepared, stage, event_type, payload)
}

fn write_mount_event_if_enabled(
    prepared: &mut PreparedRun,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), ErrorItem> {
    if !mount_event_enabled(prepared, event_type) {
        return Ok(());
    }
    write_event(prepared, STAGE_MOUNT, event_type, payload)
}

struct MountEventWriter<'a> {
    prepared: &'a mut PreparedRun,
}

impl<'a> MountEventHooks for MountEventWriter<'a> {
    fn on_validated(&mut self, entry: &sr_compiler::MountPlanEntry) -> Result<(), ErrorItem> {
        write_mount_event_if_enabled(
            self.prepared,
            EVENT_MOUNT_VALIDATED,
            json!({
                "source": entry.source.as_str(),
                "target": entry.target.as_str(),
                "read_only": entry.read_only
            }),
        )
    }

    fn on_applied(&mut self, entry: &sr_compiler::MountPlanEntry) -> Result<(), ErrorItem> {
        write_mount_event_if_enabled(
            self.prepared,
            EVENT_MOUNT_APPLIED,
            json!({
                "source": entry.source.as_str(),
                "target": entry.target.as_str(),
                "read_only": entry.read_only
            }),
        )
    }

    fn on_rejected(
        &mut self,
        entry: &sr_compiler::MountPlanEntry,
        message: &str,
    ) -> Result<(), ErrorItem> {
        write_mount_event_if_enabled(
            self.prepared,
            EVENT_MOUNT_REJECTED,
            json!({
                "source": entry.source.as_str(),
                "target": entry.target.as_str(),
                "read_only": entry.read_only,
                "errorCode": SR_RUN_101,
                "message": message
            }),
        )
    }
}

fn ensure_prepared_state(prepared: &PreparedRun) -> Result<(), ErrorItem> {
    if prepared.state != RunState::Prepared {
        return Err(ErrorItem::new(
            SR_RUN_001,
            "state",
            "runner launch requires prepared state",
        ));
    }
    Ok(())
}

fn persist_vm_pid(prepared: &mut PreparedRun, vm_pid: u32) -> Result<(), ErrorItem> {
    fs::write(prepared.vm_pid_path(), vm_pid.to_string()).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "launch.vmPid",
            format!("failed to write vm pid artifact: {err}"),
        )
    })
}

fn ensure_launch_binaries(launch_plan: &LaunchPlan) -> Result<(), ErrorItem> {
    ensure_runtime_binary(
        &launch_plan.jailer.program,
        "jailer",
        "launch.preflight.jailer",
    )?;
    ensure_runtime_binary(
        &launch_plan.firecracker.program,
        "firecracker",
        "launch.preflight.firecracker",
    )?;
    Ok(())
}

fn ensure_runtime_binary(binary: &str, label: &str, error_path: &str) -> Result<(), ErrorItem> {
    let binary = binary.trim();
    if binary.is_empty() {
        return Err(preflight_error(
            error_path,
            format!("required {label} executable is empty"),
        ));
    }

    let resolved = resolve_binary_path(binary).ok_or_else(|| {
        preflight_error(
            error_path,
            format!(
                "required {label} executable '{binary}' was not found; install it or add it to PATH"
            ),
        )
    })?;

    let metadata = fs::metadata(&resolved).map_err(|err| {
        preflight_error(
            error_path,
            format!(
                "failed to inspect {label} executable '{binary}' at '{}': {err}",
                resolved.display()
            ),
        )
    })?;

    if !metadata.is_file() {
        return Err(preflight_error(
            error_path,
            format!(
                "{label} executable '{binary}' resolved to '{}' but it is not a file",
                resolved.display()
            ),
        ));
    }

    if !is_executable(&metadata) {
        return Err(preflight_error(
            error_path,
            format!(
                "{label} executable '{binary}' resolved to '{}' but is not executable",
                resolved.display()
            ),
        ));
    }

    Ok(())
}

fn preflight_error(path: &str, message: String) -> ErrorItem {
    ErrorItem::new(SR_RUN_002, path, message)
}

fn run_network_apply_error(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_RUN_201, path, message)
}

fn run_network_release_error(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_RUN_202, path, message)
}

fn resolve_binary_path(binary: &str) -> Option<PathBuf> {
    let binary_path = Path::new(binary);
    if binary_path.is_absolute() || binary.contains(std::path::MAIN_SEPARATOR) {
        return binary_path.exists().then(|| binary_path.to_path_buf());
    }

    let path_env = env::var_os("PATH")?;
    for dir in env::split_paths(&path_env) {
        let candidate = dir.join(binary);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}
