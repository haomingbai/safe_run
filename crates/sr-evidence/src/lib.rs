use serde::{Deserialize, Serialize};

pub const RUN_REPORT_SCHEMA_VERSION: &str = "safe-run.report/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceEvent {
    pub timestamp: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    pub stage: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: serde_json::Value,
    #[serde(rename = "hashPrev")]
    pub hash_prev: String,
    #[serde(rename = "hashSelf")]
    pub hash_self: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "finishedAt")]
    pub finished_at: String,
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
    pub artifacts: ReportArtifacts,
    #[serde(rename = "policySummary")]
    pub policy_summary: PolicySummary,
    #[serde(rename = "resourceUsage")]
    pub resource_usage: ResourceUsage,
    pub events: Vec<EvidenceEvent>,
    pub integrity: Integrity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportArtifacts {
    #[serde(rename = "kernelHash")]
    pub kernel_hash: String,
    #[serde(rename = "rootfsHash")]
    pub rootfs_hash: String,
    #[serde(rename = "policyHash")]
    pub policy_hash: String,
    #[serde(rename = "commandHash")]
    pub command_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySummary {
    pub network: String,
    pub mounts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceUsage {
    pub cpu: String,
    pub memory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Integrity {
    pub digest: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evidence_event_serializes_hash_chain_fields() {
        let event = EvidenceEvent {
            timestamp: "2026-02-06T10:00:00Z".to_string(),
            run_id: "sr-20260206-001".to_string(),
            stage: "launch".to_string(),
            event_type: "vm.started".to_string(),
            payload: json!({"pid": 1234}),
            hash_prev: "sha256:0000000000000000".to_string(),
            hash_self: "sha256:1111111111111111".to_string(),
        };

        let value = serde_json::to_value(event).expect("serialize evidence event");
        assert_eq!(value["runId"], "sr-20260206-001");
        assert_eq!(value["type"], "vm.started");
        assert_eq!(value["hashPrev"], "sha256:0000000000000000");
        assert_eq!(value["hashSelf"], "sha256:1111111111111111");
    }

    #[test]
    fn run_report_serializes_m1_subset_fields() {
        let report = RunReport {
            schema_version: RUN_REPORT_SCHEMA_VERSION.to_string(),
            run_id: "sr-20260206-001".to_string(),
            started_at: "2026-02-06T10:00:00Z".to_string(),
            finished_at: "2026-02-06T10:00:05Z".to_string(),
            exit_code: 0,
            artifacts: ReportArtifacts {
                kernel_hash: "sha256:kernel".to_string(),
                rootfs_hash: "sha256:rootfs".to_string(),
                policy_hash: "sha256:policy".to_string(),
                command_hash: "sha256:command".to_string(),
            },
            policy_summary: PolicySummary {
                network: "none".to_string(),
                mounts: 0,
            },
            resource_usage: ResourceUsage {
                cpu: "10000 100000".to_string(),
                memory: "256Mi".to_string(),
            },
            events: vec![EvidenceEvent {
                timestamp: "2026-02-06T10:00:00Z".to_string(),
                run_id: "sr-20260206-001".to_string(),
                stage: "prepare".to_string(),
                event_type: "run.prepared".to_string(),
                payload: json!({"workdir": "/var/lib/safe-run/runs/sr-20260206-001"}),
                hash_prev: "sha256:0000000000000000".to_string(),
                hash_self: "sha256:1111111111111111".to_string(),
            }],
            integrity: Integrity {
                digest: "sha256:report".to_string(),
            },
        };

        let value = serde_json::to_value(report).expect("serialize run report");
        assert_eq!(value["schemaVersion"], RUN_REPORT_SCHEMA_VERSION);
        assert_eq!(value["runId"], "sr-20260206-001");
        assert_eq!(value["exitCode"], 0);
        assert_eq!(value["artifacts"]["kernelHash"], "sha256:kernel");
        assert_eq!(value["artifacts"]["rootfsHash"], "sha256:rootfs");
        assert_eq!(value["artifacts"]["policyHash"], "sha256:policy");
        assert_eq!(value["artifacts"]["commandHash"], "sha256:command");
        assert_eq!(value["policySummary"]["network"], "none");
        assert_eq!(value["policySummary"]["mounts"], 0);
        assert_eq!(value["resourceUsage"]["cpu"], "10000 100000");
        assert_eq!(value["resourceUsage"]["memory"], "256Mi");
        assert_eq!(value["integrity"]["digest"], "sha256:report");
    }
}
