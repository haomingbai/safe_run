use serde_json::json;
use sr_common::SR_OPS_301;
use sr_evidence::{
    archive_report, compute_integrity_digest, derive_event_hash, load_archive_index,
    load_archived_report, EvidenceEvent, Integrity, MountAudit, NetworkAudit, PolicySummary,
    ReportArtifacts, ResourceUsage, RunReport, RUN_REPORT_SCHEMA_VERSION, STAGE_PREPARE,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const GENESIS_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn archive_write_success_returns_archive_and_verification_metadata() {
    let report = valid_report("sr-archive-ok-1");
    let archive_root = temp_dir("archive-write-success-1");

    let archived = archive_report(&report, &archive_root, "180d").expect("archive report");

    assert!(archived.archive.is_some());
    assert!(archived.verification.is_some());
    let archive = archived.archive.as_ref().expect("archive metadata");
    let verification = archived.verification.as_ref().expect("verification metadata");
    assert!(!archive.bundle_id.is_empty());
    assert!(!archive.stored_at.is_empty());
    assert_eq!(archive.retention, "180d");
    assert_eq!(verification.algorithm, "sha256");
    assert_eq!(verification.result, "pass");
    assert!(!verification.verified_at.is_empty());

    let _ = fs::remove_dir_all(&archive_root);
}

#[test]
fn archive_write_success_persists_report_file() {
    let report = valid_report("sr-archive-ok-2");
    let archive_root = temp_dir("archive-write-success-2");

    let archived = archive_report(&report, &archive_root, "90d").expect("archive report");
    let archive = archived.archive.as_ref().expect("archive metadata");
    let report_path = archive_root.join(&archive.bundle_id).join("run_report.json");

    assert!(report_path.exists());
    let persisted = load_archived_report(&archive_root, &archive.bundle_id).expect("load report");
    assert_eq!(persisted.run_id, report.run_id);
    assert_eq!(persisted.verification.expect("verification").algorithm, "sha256");

    let _ = fs::remove_dir_all(&archive_root);
}

#[test]
fn archive_index_success_contains_written_bundle() {
    let report = valid_report("sr-archive-index-1");
    let archive_root = temp_dir("archive-index-success-1");

    let archived = archive_report(&report, &archive_root, "30d").expect("archive report");
    let archive = archived.archive.as_ref().expect("archive metadata");
    let index = load_archive_index(&archive_root).expect("load index");

    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].bundle_id, archive.bundle_id);
    assert_eq!(index.entries[0].run_id, report.run_id);

    let _ = fs::remove_dir_all(&archive_root);
}

#[test]
fn archive_index_success_appends_multiple_bundles() {
    let first = valid_report("sr-archive-index-2-a");
    let second = valid_report("sr-archive-index-2-b");
    let archive_root = temp_dir("archive-index-success-2");

    archive_report(&first, &archive_root, "30d").expect("archive first");
    archive_report(&second, &archive_root, "30d").expect("archive second");
    let index = load_archive_index(&archive_root).expect("load index");

    assert_eq!(index.entries.len(), 2);
    assert!(index.entries.iter().any(|entry| entry.run_id == first.run_id));
    assert!(index.entries.iter().any(|entry| entry.run_id == second.run_id));

    let _ = fs::remove_dir_all(&archive_root);
}

#[test]
fn archive_write_failure_returns_ops_301_when_archive_root_is_file() {
    let report = valid_report("sr-archive-fail-1");
    let base = temp_dir("archive-fail-root-file");
    let archive_root_file = base.join("archive-root-file");
    fs::write(&archive_root_file, b"not a dir").expect("create blocker file");

    let err = archive_report(&report, &archive_root_file, "7d").expect_err("must fail");
    assert_eq!(err.code, SR_OPS_301);

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn archive_write_failure_returns_ops_301_when_index_path_is_directory() {
    let report = valid_report("sr-archive-fail-2");
    let archive_root = temp_dir("archive-fail-index-dir");
    fs::create_dir_all(archive_root.join("index.json")).expect("create index dir blocker");

    let err = archive_report(&report, &archive_root, "7d").expect_err("must fail");
    assert_eq!(err.code, SR_OPS_301);

    let _ = fs::remove_dir_all(&archive_root);
}

fn valid_report(run_id: &str) -> RunReport {
    let mut event = EvidenceEvent {
        timestamp: "2026-02-21T10:00:00Z".to_string(),
        run_id: run_id.to_string(),
        stage: STAGE_PREPARE.to_string(),
        event_type: "run.prepared".to_string(),
        payload: json!({"workdir": "/tmp/safe-run/test"}),
        hash_prev: GENESIS_HASH.to_string(),
        hash_self: String::new(),
    };
    event.hash_self = derive_event_hash(&event);

    let mut report = RunReport {
        schema_version: RUN_REPORT_SCHEMA_VERSION.to_string(),
        run_id: run_id.to_string(),
        started_at: "2026-02-21T10:00:00Z".to_string(),
        finished_at: "2026-02-21T10:00:01Z".to_string(),
        exit_code: 0,
        artifacts: ReportArtifacts {
            kernel_hash: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            rootfs_hash: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            policy_hash: "sha256:3333333333333333333333333333333333333333333333333333333333333333"
                .to_string(),
            command_hash: "sha256:4444444444444444444444444444444444444444444444444444444444444444"
                .to_string(),
        },
        policy_summary: PolicySummary {
            network: "none".to_string(),
            mounts: 0,
        },
        resource_usage: ResourceUsage {
            cpu: "cpuUsageUsec=0".to_string(),
            memory: "memoryCurrentBytes=0".to_string(),
        },
        events: vec![event],
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

fn temp_dir(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.push(format!("safe-run-vibe-{label}-{nanos}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp dir");
    path
}
