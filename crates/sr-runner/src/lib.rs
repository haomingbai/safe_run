use serde::{Deserialize, Serialize};
use sr_compiler::CompileBundle;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sr_compiler::{EvidencePlan, Plan};

    fn sample_compile_bundle() -> CompileBundle {
        CompileBundle {
            firecracker_config: json!({
                "machine-config": {
                    "vcpu_count": 1,
                    "mem_size_mib": 256,
                    "smt": false
                }
            }),
            jailer_plan: Plan {
                enabled: true,
                ops: vec!["prepare_jailer_context".to_string()],
            },
            cgroup_plan: Plan {
                enabled: true,
                ops: vec!["set_cpu_max=100000 100000".to_string()],
            },
            network_plan: None,
            evidence_plan: EvidencePlan {
                enabled: true,
                events: vec!["run.prepared".to_string()],
            },
        }
    }

    #[test]
    fn runner_control_request_serializes_required_fields() {
        let request = RunnerControlRequest {
            compile_bundle: sample_compile_bundle(),
            runtime_context: RuntimeContext {
                workdir: "/var/lib/safe-run/runs/sr-20260206-001".to_string(),
                timeout_sec: 300,
            },
        };

        let value = serde_json::to_value(request).expect("serialize runner request");
        assert!(value.get("compileBundle").is_some());
        assert_eq!(
            value["runtimeContext"]["workdir"],
            "/var/lib/safe-run/runs/sr-20260206-001"
        );
        assert_eq!(value["runtimeContext"]["timeoutSec"], 300);
    }

    #[test]
    fn runner_control_response_serializes_required_fields() {
        let response = RunnerControlResponse {
            run_id: "sr-20260206-001".to_string(),
            state: RunState::Finished,
            artifacts: RunArtifacts {
                log: "events.jsonl".to_string(),
                report: "run_report.json".to_string(),
            },
            event_stream: vec!["events.jsonl".to_string()],
        };

        let value = serde_json::to_value(response).expect("serialize runner response");
        assert_eq!(value["runId"], "sr-20260206-001");
        assert_eq!(value["state"], "finished");
        assert_eq!(value["artifacts"]["log"], "events.jsonl");
        assert_eq!(value["artifacts"]["report"], "run_report.json");
        assert_eq!(value["eventStream"][0], "events.jsonl");
    }
}
