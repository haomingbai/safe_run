use sr_common::{SR_EVD_301, SR_EVD_302, SR_EVD_303};
use sr_evidence::{
    compute_integrity_digest, derive_event_hash, verify_report, EvidenceEvent, Integrity, MountAudit,
    NetworkAudit, PolicySummary, ReportArtifacts, ResourceUsage, RunReport,
    RUN_REPORT_SCHEMA_VERSION, STAGE_PREPARE,
};
use serde_json::json;

const GENESIS_HASH: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn report_verify_compat_valid_report_single_event_passes() {
    let report = valid_report_with_events(1);
    let result = verify_report(&report);
    assert!(result.valid);
    assert_eq!(result.checks.len(), 3);
    assert!(result.checks.iter().any(|check| check.name == "schema" && check.ok));
    assert!(
        result
            .checks
            .iter()
            .any(|check| check.name == "artifact_hash" && check.ok)
    );
    assert!(
        result
            .checks
            .iter()
            .any(|check| check.name == "event_chain" && check.ok)
    );
}

#[test]
fn report_verify_compat_valid_report_multi_events_passes() {
    let report = valid_report_with_events(3);
    let result = verify_report(&report);
    assert!(result.valid);
    assert!(result.errors.is_empty());
}

#[test]
fn report_verify_compat_schema_mismatch_returns_301_for_wrong_version() {
    let mut report = valid_report_with_events(1);
    report.schema_version = "safe-run.report/v2".to_string();
    let result = verify_report(&report);
    assert!(!result.valid);
    assert_eq!(result.errors[0].code, SR_EVD_301);
}

#[test]
fn report_verify_compat_schema_mismatch_returns_301_for_empty_schema() {
    let mut report = valid_report_with_events(1);
    report.schema_version = String::new();
    let result = verify_report(&report);
    assert!(!result.valid);
    assert_eq!(result.errors[0].code, SR_EVD_301);
}

#[test]
fn report_verify_compat_artifact_hash_mismatch_returns_302_for_digest_mismatch() {
    let mut report = valid_report_with_events(1);
    report.artifacts.policy_hash = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    let result = verify_report(&report);
    assert!(!result.valid);
    assert_eq!(result.errors[0].code, SR_EVD_302);
}

#[test]
fn report_verify_compat_artifact_hash_mismatch_returns_302_for_invalid_hash_format() {
    let mut report = valid_report_with_events(1);
    report.artifacts.command_hash = "sha256:not_hex".to_string();
    report.integrity.digest = compute_integrity_digest(&report).expect("recompute digest");
    let result = verify_report(&report);
    assert!(!result.valid);
    assert_eq!(result.errors[0].code, SR_EVD_302);
}

#[test]
fn report_verify_compat_event_chain_break_returns_303_for_hash_prev_mismatch() {
    let mut report = valid_report_with_events(2);
    report.events[0].hash_prev = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    let result = verify_report(&report);
    assert!(!result.valid);
    assert_eq!(result.errors[0].code, SR_EVD_303);
}

#[test]
fn report_verify_compat_event_chain_break_returns_303_for_hash_self_mismatch() {
    let mut report = valid_report_with_events(2);
    report.events[1].hash_self = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string();
    let result = verify_report(&report);
    assert!(!result.valid);
    assert_eq!(result.errors[0].code, SR_EVD_303);
}

fn valid_report_with_events(count: usize) -> RunReport {
    let mut events = Vec::new();
    let mut prev = GENESIS_HASH.to_string();

    for i in 0..count {
        let mut event = EvidenceEvent {
            timestamp: format!("2026-02-21T10:00:0{}Z", i),
            run_id: "sr-report-verify-test".to_string(),
            stage: STAGE_PREPARE.to_string(),
            event_type: "run.prepared".to_string(),
            payload: json!({"seq": i}),
            hash_prev: prev.clone(),
            hash_self: String::new(),
        };
        event.hash_self = derive_event_hash(&event);
        prev = event.hash_self.clone();
        events.push(event);
    }

    let mut report = RunReport {
        schema_version: RUN_REPORT_SCHEMA_VERSION.to_string(),
        run_id: "sr-report-verify-test".to_string(),
        started_at: "2026-02-21T10:00:00Z".to_string(),
        finished_at: "2026-02-21T10:00:05Z".to_string(),
        exit_code: 0,
        artifacts: ReportArtifacts {
            kernel_hash: "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            rootfs_hash: "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            policy_hash: "sha256:3333333333333333333333333333333333333333333333333333333333333333".to_string(),
            command_hash: "sha256:4444444444444444444444444444444444444444444444444444444444444444".to_string(),
        },
        policy_summary: PolicySummary {
            network: "none".to_string(),
            mounts: 0,
        },
        resource_usage: ResourceUsage {
            cpu: "cpuUsageUsec=0".to_string(),
            memory: "memoryCurrentBytes=0".to_string(),
        },
        events,
        mount_audit: MountAudit::default(),
        network_audit: NetworkAudit::default(),
        archive: None,
        verification: None,
        integrity: Integrity {
            digest: String::new(),
        },
    };
    report.integrity.digest = compute_integrity_digest(&report).expect("compute digest");
    report
}
