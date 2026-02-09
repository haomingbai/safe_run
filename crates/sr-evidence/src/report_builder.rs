use crate::hashing::{sha256_file, sha256_json_value};
use crate::{
    EvidenceEvent, Integrity, MountAudit, PolicySummary, ReportArtifacts, ResourceUsage, RunReport,
    EVENT_MOUNT_APPLIED, EVENT_MOUNT_REJECTED, EVENT_MOUNT_VALIDATED, EVENT_RESOURCE_SAMPLED,
    RUN_REPORT_SCHEMA_VERSION,
};
use serde_json::Value;
use sr_common::{ErrorItem, SR_EVD_002};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Assemble the M0-M2 run report with a precomputed integrity digest.
pub fn build_report(
    run_id: String,
    started_at: String,
    finished_at: String,
    exit_code: i32,
    artifacts: ReportArtifacts,
    policy_summary: PolicySummary,
    resource_usage: ResourceUsage,
    events: Vec<EvidenceEvent>,
    mount_audit: MountAudit,
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
        mount_audit,
        integrity: Integrity {
            digest: integrity_digest,
        },
    }
}

/// Derive `(started_at, finished_at)` from event stream boundaries.
/// Falls back to `unix:<sec>.<nsec>` when events are empty.
pub fn event_time_range(events: &[EvidenceEvent]) -> (String, String) {
    if events.is_empty() {
        let fallback = unix_timestamp();
        return (fallback.clone(), fallback);
    }
    let started_at = events
        .first()
        .map(|event| event.timestamp.clone())
        .unwrap_or_else(unix_timestamp);
    let finished_at = events
        .last()
        .map(|event| event.timestamp.clone())
        .unwrap_or_else(unix_timestamp);
    (started_at, finished_at)
}

/// Extract resource summary from latest `resource.sampled` event.
/// Returns zero-valued defaults when event stream has no resource samples.
pub fn resource_usage_from_events(events: &[EvidenceEvent]) -> ResourceUsage {
    for event in events.iter().rev() {
        if event.event_type != EVENT_RESOURCE_SAMPLED {
            continue;
        }
        let cpu = event
            .payload
            .get("cpuUsageUsec")
            .and_then(|value| value.as_u64())
            .map(|value| format!("cpuUsageUsec={value}"))
            .unwrap_or_else(|| "cpuUsageUsec=0".to_string());
        let memory = event
            .payload
            .get("memoryCurrentBytes")
            .and_then(|value| value.as_u64())
            .map(|value| format!("memoryCurrentBytes={value}"))
            .unwrap_or_else(|| "memoryCurrentBytes=0".to_string());
        return ResourceUsage { cpu, memory };
    }
    ResourceUsage {
        cpu: "cpuUsageUsec=0".to_string(),
        memory: "memoryCurrentBytes=0".to_string(),
    }
}

pub fn mount_audit_from_events(events: &[EvidenceEvent]) -> MountAudit {
    let mut requested = 0usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut applied = 0usize;
    let mut reasons: Vec<String> = Vec::new();

    for event in events {
        match event.event_type.as_str() {
            EVENT_MOUNT_VALIDATED => {
                requested += 1;
                accepted += 1;
            }
            EVENT_MOUNT_REJECTED => {
                requested += 1;
                rejected += 1;
                collect_reasons(&event.payload, &mut reasons);
            }
            EVENT_MOUNT_APPLIED => {
                applied += 1;
            }
            _ => {}
        }
    }

    if accepted == 0 && applied > 0 {
        accepted = applied;
    }
    if requested == 0 {
        requested = accepted + rejected + applied;
    }

    MountAudit {
        requested,
        accepted,
        rejected,
        reasons,
    }
}

fn unix_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix:{}.{:09}", now.as_secs(), now.subsec_nanos())
}

fn collect_reasons(payload: &Value, reasons: &mut Vec<String>) {
    if let Some(array) = payload.get("reasons").and_then(|value| value.as_array()) {
        for item in array.iter().filter_map(|value| value.as_str()) {
            push_unique(reasons, item.to_string());
        }
    }

    if let Some(value) = payload.get("reason").and_then(|value| value.as_str()) {
        push_unique(reasons, value.to_string());
        return;
    }

    if let Some(value) = payload.get("errorCode").and_then(|value| value.as_str()) {
        push_unique(reasons, value.to_string());
        return;
    }

    if let Some(value) = payload.get("message").and_then(|value| value.as_str()) {
        push_unique(reasons, value.to_string());
    }
}

fn push_unique(reasons: &mut Vec<String>, value: String) {
    if !reasons.iter().any(|existing| existing == &value) {
        reasons.push(value);
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
