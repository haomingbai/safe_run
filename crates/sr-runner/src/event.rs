use crate::model::PreparedRun;
use serde_json::Value;
use sr_common::{ErrorItem, SR_RUN_001};
use sr_evidence::EvidenceEvent;
use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

/// Append an evidence event to the run's event stream and advance the hash chain.
pub(crate) fn write_event(
    prepared: &mut PreparedRun,
    stage: &str,
    event_type: &str,
    payload: Value,
) -> Result<(), ErrorItem> {
    let timestamp = unix_timestamp_string();
    let hash_prev = prepared.last_event_hash.clone();
    let mut event = EvidenceEvent {
        timestamp,
        run_id: prepared.run_id.clone(),
        stage: stage.to_string(),
        event_type: event_type.to_string(),
        payload,
        hash_prev,
        hash_self: String::new(),
    };
    event.hash_self = derive_event_hash(&event);

    let serialized = serde_json::to_string(&event).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "events.serialize",
            format!("failed to serialize event: {err}"),
        )
    })?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(prepared.event_log_path())
        .map_err(|err| {
            ErrorItem::new(
                SR_RUN_001,
                "events.open",
                format!("failed to open event stream file: {err}"),
            )
        })?;
    writeln!(file, "{serialized}").map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "events.write",
            format!("failed to append event stream file: {err}"),
        )
    })?;

    prepared.last_event_hash = event.hash_self;
    Ok(())
}

/// Compute deterministic hash material for the evidence chain.
fn derive_event_hash(event: &EvidenceEvent) -> String {
    let mut hasher = DefaultHasher::new();
    event.hash_prev.hash(&mut hasher);
    event.timestamp.hash(&mut hasher);
    event.run_id.hash(&mut hasher);
    event.stage.hash(&mut hasher);
    event.event_type.hash(&mut hasher);
    event.payload.to_string().hash(&mut hasher);
    format!("sha256:{:016x}", hasher.finish())
}

fn unix_timestamp_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix:{}.{:09}", now.as_secs(), now.subsec_nanos())
}
