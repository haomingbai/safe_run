use crate::cleanup::cleanup_run;
use crate::event::write_event;
use crate::model::{
    MonitorResult, PreparedRun, RunState, RunnerControlRequest, RunnerControlResponse,
    RunnerRuntime,
};
use crate::monitor::monitor_run;
use crate::prepare::prepare_run;
use serde_json::json;
use sr_common::{ErrorItem, SR_RUN_001, SR_RUN_002};
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct Runner {
    runtime: RunnerRuntime,
}

impl Runner {
    /// Create a runner with default Firecracker/jailer executables.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a runner with explicitly provided runtime binaries.
    pub fn with_runtime(runtime: RunnerRuntime) -> Self {
        Self { runtime }
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
            "prepare",
            "run.prepared",
            json!({
                "workdir": prepared.runtime_context.workdir,
                "timeoutSec": prepared.runtime_context.timeout_sec
            }),
        )?;

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
            "launch",
            "vm.started",
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
        match cleanup_run(prepared) {
            Ok(()) => Ok(()),
            Err(err) => {
                prepared.state = RunState::Failed;
                let error_code = err.code.clone();
                let message = err.message.clone();
                let _ = write_event(
                    prepared,
                    "cleanup",
                    "run.failed",
                    json!({
                        "reason": "cleanup.failure",
                        "errorCode": error_code,
                        "message": message
                    }),
                );
                Err(err)
            }
        }
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
        prepared.state = RunState::Failed;
        let _ = fs::write(prepared.cleanup_marker_path(), "cleanup invoked");
        let _ = write_event(
            prepared,
            "cleanup",
            "run.failed",
            json!({
                "reason": reason,
                "errorCode": error_code,
                "message": message
            }),
        );
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
