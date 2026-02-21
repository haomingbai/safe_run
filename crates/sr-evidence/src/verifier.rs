use crate::{compute_integrity_digest, derive_event_hash, RunReport, RUN_REPORT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use sr_common::{ErrorItem, SR_EVD_002, SR_EVD_301, SR_EVD_302, SR_EVD_303};
use std::fs;
use std::path::Path;

const GENESIS_HASH: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyCheck {
    pub name: String,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub valid: bool,
    pub checks: Vec<VerifyCheck>,
    pub errors: Vec<ErrorItem>,
}

pub fn verify_report_file(path: &Path) -> Result<VerifyResult, ErrorItem> {
    let raw = fs::read_to_string(path).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "report.verify",
            format!("failed to read run report '{}': {err}", path.display()),
        )
    })?;
    let report: RunReport = serde_json::from_str(&raw).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "report.verify",
            format!("failed to parse run report '{}': {err}", path.display()),
        )
    })?;
    Ok(verify_report(&report))
}

pub fn verify_report(report: &RunReport) -> VerifyResult {
    let mut checks = vec![
        VerifyCheck {
            name: "schema".to_string(),
            ok: true,
        },
        VerifyCheck {
            name: "artifact_hash".to_string(),
            ok: true,
        },
        VerifyCheck {
            name: "event_chain".to_string(),
            ok: true,
        },
    ];
    let mut errors = Vec::new();

    if report.schema_version != RUN_REPORT_SCHEMA_VERSION {
        set_check_status(&mut checks, "schema", false);
        errors.push(ErrorItem::new(
            SR_EVD_301,
            "schemaVersion",
            format!(
                "schemaVersion must be '{}'",
                RUN_REPORT_SCHEMA_VERSION
            ),
        ));
    }

    if !event_chain_ok(report) {
        set_check_status(&mut checks, "event_chain", false);
        errors.push(ErrorItem::new(
            SR_EVD_303,
            "events",
            "event hash chain verification failed",
        ));
    }

    if !artifact_hash_ok(report) {
        set_check_status(&mut checks, "artifact_hash", false);
        errors.push(ErrorItem::new(
            SR_EVD_302,
            "artifacts",
            "artifact hash verification failed",
        ));
    }

    VerifyResult {
        valid: errors.is_empty(),
        checks,
        errors,
    }
}

fn set_check_status(checks: &mut [VerifyCheck], name: &str, ok: bool) {
    if let Some(check) = checks.iter_mut().find(|check| check.name == name) {
        check.ok = ok;
    }
}

fn event_chain_ok(report: &RunReport) -> bool {
    let mut expected_prev = GENESIS_HASH.to_string();
    for event in &report.events {
        if event.hash_prev != expected_prev {
            return false;
        }
        if derive_event_hash(event) != event.hash_self {
            return false;
        }
        expected_prev = event.hash_self.clone();
    }
    true
}

fn artifact_hash_ok(report: &RunReport) -> bool {
    if !is_sha256_hash(&report.artifacts.kernel_hash)
        || !is_sha256_hash(&report.artifacts.rootfs_hash)
        || !is_sha256_hash(&report.artifacts.policy_hash)
        || !is_sha256_hash(&report.artifacts.command_hash)
    {
        return false;
    }

    if !is_sha256_hash(&report.integrity.digest) {
        return false;
    }

    match compute_integrity_digest(report) {
        Ok(recomputed) => recomputed == report.integrity.digest,
        Err(_) => false,
    }
}

fn is_sha256_hash(value: &str) -> bool {
    if !value.starts_with("sha256:") {
        return false;
    }
    let payload = &value[7..];
    payload.len() == 64 && payload.chars().all(|ch| ch.is_ascii_hexdigit())
}
