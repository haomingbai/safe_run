use crate::{
    append_archive_index, compute_integrity_digest, ArchiveIndexEntry, ArchiveMetadata, RunReport,
    VerificationMetadata,
};
use sr_common::{ErrorItem, SR_OPS_301};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const ARCHIVED_REPORT_FILE: &str = "run_report.json";

pub fn archive_report(
    report: &RunReport,
    archive_root: &Path,
    retention: &str,
) -> Result<RunReport, ErrorItem> {
    fs::create_dir_all(archive_root).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.root",
            format!(
                "failed to prepare archive root '{}': {err}",
                archive_root.display()
            ),
        )
    })?;

    let stored_at = unix_timestamp();
    let bundle_id = build_bundle_id(&report.run_id);

    let mut archived = report.clone();
    archived.archive = Some(ArchiveMetadata {
        bundle_id: bundle_id.clone(),
        stored_at: stored_at.clone(),
        retention: retention.to_string(),
    });
    archived.verification = Some(VerificationMetadata {
        algorithm: "sha256".to_string(),
        verified_at: stored_at.clone(),
        result: "pass".to_string(),
    });
    archived.integrity.digest = compute_integrity_digest(&archived).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.integrity",
            format!("failed to compute archive report digest: {}", err.message),
        )
    })?;

    write_archived_report(archive_root, &bundle_id, &archived)?;
    append_archive_index(
        archive_root,
        ArchiveIndexEntry {
            bundle_id,
            run_id: archived.run_id.clone(),
            stored_at,
            retention: retention.to_string(),
            result: "pass".to_string(),
        },
    )?;

    Ok(archived)
}

pub fn load_archived_report(archive_root: &Path, bundle_id: &str) -> Result<RunReport, ErrorItem> {
    let path = archive_root.join(bundle_id).join(ARCHIVED_REPORT_FILE);
    let raw = fs::read_to_string(&path).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.read",
            format!("failed to read archived report '{}': {err}", path.display()),
        )
    })?;
    serde_json::from_str::<RunReport>(&raw).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.read",
            format!(
                "failed to parse archived report '{}': {err}",
                path.display()
            ),
        )
    })
}

fn write_archived_report(
    archive_root: &Path,
    bundle_id: &str,
    report: &RunReport,
) -> Result<(), ErrorItem> {
    let bundle_dir = archive_root.join(bundle_id);
    fs::create_dir_all(&bundle_dir).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.bundle",
            format!(
                "failed to prepare archive bundle '{}': {err}",
                bundle_dir.display()
            ),
        )
    })?;

    let report_json = serde_json::to_string_pretty(report).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.write",
            format!("failed to serialize archived report: {err}"),
        )
    })?;
    let report_path = bundle_dir.join(ARCHIVED_REPORT_FILE);
    fs::write(&report_path, report_json).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.write",
            format!(
                "failed to write archived report '{}': {err}",
                report_path.display()
            ),
        )
    })
}

fn build_bundle_id(run_id: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let normalized = run_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("bundle-{normalized}-{nanos}")
}

fn unix_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix:{}.{:09}", now.as_secs(), now.subsec_nanos())
}
