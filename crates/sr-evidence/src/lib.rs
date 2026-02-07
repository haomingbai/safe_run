mod event_writer;
mod hashing;
mod report_builder;

use serde::{Deserialize, Serialize};

pub use event_writer::append_event;
pub use hashing::{
    derive_event_hash, normalize_json_string, sha256_bytes, sha256_file, sha256_json_value,
    sha256_string,
};
pub use report_builder::{
    build_report, compute_artifact_hashes, compute_artifact_hashes_from_json,
    compute_integrity_digest, ArtifactInputs, ArtifactJsonInputs,
};

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
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

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

    fn temp_event_log_path(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("safe-run-evidence-{label}-{}", Uuid::new_v4()));
        path
    }

    #[test]
    fn event_writer_appends_events_and_advances_hash_chain() {
        let log_path = temp_event_log_path("events");
        let run_id = "sr-20260206-001";
        let (event1, hash1) = append_event(
            &log_path,
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            run_id,
            "prepare",
            "run.prepared",
            json!({"workdir": "/tmp/run"}),
        )
        .expect("write first event");
        let (event2, hash2) = append_event(
            &log_path,
            &hash1,
            run_id,
            "launch",
            "vm.started",
            json!({"pid": 1234}),
        )
        .expect("write second event");

        assert_eq!(event1.hash_self, hash1);
        assert_eq!(event2.hash_prev, hash1);
        assert_eq!(event2.hash_self, hash2);
        assert_eq!(event2.hash_self, derive_event_hash(&event2));

        let raw = fs::read_to_string(&log_path).expect("read event log");
        let lines: Vec<&str> = raw.lines().collect();
        assert_eq!(lines.len(), 2);

        let _ = fs::remove_file(&log_path);
    }

    #[test]
    fn evidence_hashes_files_and_strings() {
        let mut path = std::env::temp_dir();
        path.push(format!("safe-run-evidence-hash-{}", Uuid::new_v4()));
        fs::write(&path, "hello world").expect("write temp file");

        let file_hash = sha256_file(&path).expect("hash file");
        let string_hash = sha256_string("hello world");
        assert_eq!(file_hash, string_hash);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn report_builder_assembles_m1_report() {
        let artifacts = ReportArtifacts {
            kernel_hash: "sha256:kernel".to_string(),
            rootfs_hash: "sha256:rootfs".to_string(),
            policy_hash: "sha256:policy".to_string(),
            command_hash: "sha256:command".to_string(),
        };
        let report = build_report(
            "sr-20260206-001".to_string(),
            "2026-02-06T10:00:00Z".to_string(),
            "2026-02-06T10:00:05Z".to_string(),
            0,
            artifacts,
            PolicySummary {
                network: "none".to_string(),
                mounts: 0,
            },
            ResourceUsage {
                cpu: "10000 100000".to_string(),
                memory: "256Mi".to_string(),
            },
            vec![],
            "sha256:digest".to_string(),
        );

        assert_eq!(report.schema_version, RUN_REPORT_SCHEMA_VERSION);
        assert_eq!(report.run_id, "sr-20260206-001");
        assert_eq!(report.integrity.digest, "sha256:digest");
    }

    #[test]
    fn normalize_json_string_sorts_keys_and_stabilizes_hashes() {
        let value_a = json!({
            "b": 1,
            "a": {"d": 2, "c": 3},
            "arr": [{"y": 2, "x": 1}]
        });
        let value_b = json!({
            "arr": [{"x": 1, "y": 2}],
            "a": {"c": 3, "d": 2},
            "b": 1
        });

        let normalized_a = normalize_json_string(&value_a);
        let normalized_b = normalize_json_string(&value_b);
        assert_eq!(normalized_a, normalized_b);
        assert_eq!(sha256_json_value(&value_a), sha256_json_value(&value_b));
    }

    #[test]
    fn integrity_digest_ignores_existing_digest_value() {
        let artifacts = ReportArtifacts {
            kernel_hash: "sha256:kernel".to_string(),
            rootfs_hash: "sha256:rootfs".to_string(),
            policy_hash: "sha256:policy".to_string(),
            command_hash: "sha256:command".to_string(),
        };
        let base_report = build_report(
            "sr-20260206-001".to_string(),
            "2026-02-06T10:00:00Z".to_string(),
            "2026-02-06T10:00:05Z".to_string(),
            0,
            artifacts.clone(),
            PolicySummary {
                network: "none".to_string(),
                mounts: 0,
            },
            ResourceUsage {
                cpu: "10000 100000".to_string(),
                memory: "256Mi".to_string(),
            },
            vec![],
            "sha256:placeholder-a".to_string(),
        );

        let mut report_b = base_report.clone();
        report_b.integrity.digest = "sha256:placeholder-b".to_string();

        let digest_a = compute_integrity_digest(&base_report).expect("digest a");
        let digest_b = compute_integrity_digest(&report_b).expect("digest b");
        assert_eq!(digest_a, digest_b);
    }
}
