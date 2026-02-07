use crate::hashing::derive_event_hash;
use crate::EvidenceEvent;
use serde_json::Value;
use sr_common::{ErrorItem, SR_EVD_001};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Append an evidence event to the event stream and return the new hash.
pub fn append_event(
    path: &Path,
    last_hash: &str,
    run_id: &str,
    stage: &str,
    event_type: &str,
    payload: Value,
) -> Result<(EvidenceEvent, String), ErrorItem> {
    let timestamp = unix_timestamp_string();
    let mut event = EvidenceEvent {
        timestamp,
        run_id: run_id.to_string(),
        stage: stage.to_string(),
        event_type: event_type.to_string(),
        payload,
        hash_prev: last_hash.to_string(),
        hash_self: String::new(),
    };
    event.hash_self = derive_event_hash(&event);

    let serialized = serde_json::to_string(&event).map_err(|err| {
        ErrorItem::new(
            SR_EVD_001,
            "events.serialize",
            format!("failed to serialize event: {err}"),
        )
    })?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            ErrorItem::new(
                SR_EVD_001,
                "events.open",
                format!("failed to open event stream file: {err}"),
            )
        })?;

    writeln!(file, "{serialized}").map_err(|err| {
        ErrorItem::new(
            SR_EVD_001,
            "events.write",
            format!("failed to append event stream file: {err}"),
        )
    })?;

    let hash_self = event.hash_self.clone();
    Ok((event, hash_self))
}

fn unix_timestamp_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix:{}.{:09}", now.as_secs(), now.subsec_nanos())
}
