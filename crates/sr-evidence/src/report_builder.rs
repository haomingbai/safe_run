use crate::hashing::{sha256_file, sha256_json_value};
use crate::{
    EvidenceEvent, Integrity, PolicySummary, ReportArtifacts, ResourceUsage, RunReport,
    RUN_REPORT_SCHEMA_VERSION,
};
use serde_json::Value;
use sr_common::{ErrorItem, SR_EVD_002};
use std::path::Path;

/// Inputs required to compute the artifacts hash bundle.
pub struct ArtifactInputs<'a> {
    pub kernel_path: &'a Path,
    pub rootfs_path: &'a Path,
    pub policy_bytes: &'a [u8],
    pub command_material: &'a str,
}

/// Inputs required to compute artifacts hashes using JSON normalization.
pub struct ArtifactJsonInputs<'a> {
    pub kernel_path: &'a Path,
    pub rootfs_path: &'a Path,
    pub policy_json: &'a Value,
    pub command_json: &'a Value,
}

/// Compute kernel/rootfs/policy/command hashes for a run report.
/// The policy and command materials must be valid JSON strings and will be normalized.
pub fn compute_artifact_hashes(inputs: ArtifactInputs<'_>) -> Result<ReportArtifacts, ErrorItem> {
    let kernel_hash = sha256_file(inputs.kernel_path).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "artifacts.kernel",
            format!("failed to hash kernel file: {err}"),
        )
    })?;
    let rootfs_hash = sha256_file(inputs.rootfs_path).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "artifacts.rootfs",
            format!("failed to hash rootfs file: {err}"),
        )
    })?;
    let policy_value: Value = serde_json::from_slice(inputs.policy_bytes).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "artifacts.policy",
            format!("failed to parse policy JSON for hashing: {err}"),
        )
    })?;
    let command_value: Value = serde_json::from_str(inputs.command_material).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "artifacts.command",
            format!("failed to parse command JSON for hashing: {err}"),
        )
    })?;
    let policy_hash = sha256_json_value(&policy_value);
    let command_hash = sha256_json_value(&command_value);

    Ok(ReportArtifacts {
        kernel_hash,
        rootfs_hash,
        policy_hash,
        command_hash,
    })
}

/// Compute kernel/rootfs/policy/command hashes with JSON normalization.
pub fn compute_artifact_hashes_from_json(
    inputs: ArtifactJsonInputs<'_>,
) -> Result<ReportArtifacts, ErrorItem> {
    let kernel_hash = sha256_file(inputs.kernel_path).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "artifacts.kernel",
            format!("failed to hash kernel file: {err}"),
        )
    })?;
    let rootfs_hash = sha256_file(inputs.rootfs_path).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "artifacts.rootfs",
            format!("failed to hash rootfs file: {err}"),
        )
    })?;
    let policy_hash = sha256_json_value(inputs.policy_json);
    let command_hash = sha256_json_value(inputs.command_json);

    Ok(ReportArtifacts {
        kernel_hash,
        rootfs_hash,
        policy_hash,
        command_hash,
    })
}

/// Assemble the M1 run report with a precomputed integrity digest.
pub fn build_report(
    run_id: String,
    started_at: String,
    finished_at: String,
    exit_code: i32,
    artifacts: ReportArtifacts,
    policy_summary: PolicySummary,
    resource_usage: ResourceUsage,
    events: Vec<EvidenceEvent>,
    integrity_digest: String,
) -> RunReport {
    RunReport {
        schema_version: RUN_REPORT_SCHEMA_VERSION.to_string(),
        run_id,
        started_at,
        finished_at,
        exit_code,
        artifacts,
        policy_summary,
        resource_usage,
        events,
        integrity: Integrity {
            digest: integrity_digest,
        },
    }
}

/// Compute integrity digest from a normalized JSON form of the report.
/// The `integrity.digest` field is normalized to an empty string before hashing.
pub fn compute_integrity_digest(report: &RunReport) -> Result<String, ErrorItem> {
    let mut value = serde_json::to_value(report).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "integrity.serialize",
            format!("failed to serialize report for digest: {err}"),
        )
    })?;

    if let Some(obj) = value.as_object_mut() {
        if let Some(integrity) = obj.get_mut("integrity") {
            if let Some(integrity_obj) = integrity.as_object_mut() {
                integrity_obj.insert("digest".to_string(), Value::String(String::new()));
            }
        }
    }

    Ok(sha256_json_value(&value))
}
