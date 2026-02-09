use crate::constants::{
    CLEANUP_MARKER_FILE, DEFAULT_CGROUP_PATH, DEFAULT_SAMPLE_INTERVAL_MS,
    FIRECRACKER_API_SOCKET_FILE, FIRECRACKER_CONFIG_FILE, RUNTIME_CONTEXT_FILE, VM_PID_FILE,
};
use serde::{Deserialize, Serialize};
use sr_compiler::{CompileBundle, EvidencePlan, MountPlan};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerControlRequest {
    #[serde(rename = "compileBundle")]
    pub compile_bundle: CompileBundle,
    #[serde(rename = "runtimeContext")]
    pub runtime_context: RuntimeContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContext {
    pub workdir: String,
    #[serde(rename = "timeoutSec")]
    pub timeout_sec: u64,
    #[serde(
        rename = "sampleIntervalMs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub sample_interval_ms: Option<u64>,
    #[serde(
        rename = "cgroupPath",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cgroup_path: Option<String>,
}

impl RuntimeContext {
    pub fn effective_sample_interval(&self) -> Duration {
        let interval_ms = self
            .sample_interval_ms
            .unwrap_or(DEFAULT_SAMPLE_INTERVAL_MS);
        Duration::from_millis(interval_ms)
    }

    pub fn effective_cgroup_path(&self) -> String {
        self.cgroup_path
            .clone()
            .unwrap_or_else(|| DEFAULT_CGROUP_PATH.to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Prepared,
    Running,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunArtifacts {
    pub log: String,
    pub report: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerControlResponse {
    #[serde(rename = "runId")]
    pub run_id: String,
    pub state: RunState,
    pub artifacts: RunArtifacts,
    #[serde(rename = "eventStream")]
    pub event_stream: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchPlan {
    pub jailer: CommandSpec,
    pub firecracker: CommandSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerRuntime {
    #[serde(rename = "jailerBin")]
    pub jailer_bin: String,
    #[serde(rename = "firecrackerBin")]
    pub firecracker_bin: String,
}

impl Default for RunnerRuntime {
    fn default() -> Self {
        Self {
            jailer_bin: "jailer".to_string(),
            firecracker_bin: "firecracker".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedRun {
    pub run_id: String,
    pub state: RunState,
    pub runtime_context: RuntimeContext,
    pub artifacts: RunArtifacts,
    pub event_stream: Vec<String>,
    pub launch_plan: LaunchPlan,
    pub mount_plan: MountPlan,
    pub evidence_plan: EvidencePlan,
    pub(crate) workdir_path: PathBuf,
    pub(crate) artifacts_dir_path: PathBuf,
    pub(crate) last_event_hash: String,
}

impl PreparedRun {
    pub fn workdir(&self) -> &Path {
        &self.workdir_path
    }

    pub fn artifacts_dir(&self) -> &Path {
        &self.artifacts_dir_path
    }

    pub fn event_log_path(&self) -> PathBuf {
        self.artifacts_dir_path.join(&self.artifacts.log)
    }

    pub fn cleanup_marker_path(&self) -> PathBuf {
        self.artifacts_dir_path.join(CLEANUP_MARKER_FILE)
    }

    pub fn firecracker_config_path(&self) -> PathBuf {
        self.workdir_path.join(FIRECRACKER_CONFIG_FILE)
    }

    pub fn runtime_context_path(&self) -> PathBuf {
        self.workdir_path.join(RUNTIME_CONTEXT_FILE)
    }

    pub fn vm_pid_path(&self) -> PathBuf {
        self.artifacts_dir_path.join(VM_PID_FILE)
    }

    pub fn api_socket_path(&self) -> PathBuf {
        self.artifacts_dir_path.join(FIRECRACKER_API_SOCKET_FILE)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorResult {
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
    #[serde(rename = "timedOut")]
    pub timed_out: bool,
    #[serde(rename = "sampleCount")]
    pub sample_count: u64,
}
