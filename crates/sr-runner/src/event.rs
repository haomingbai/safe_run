use crate::model::PreparedRun;
use serde_json::Value;
use sr_common::ErrorItem;
use sr_evidence::append_event;

/// Append an evidence event to the run's event stream and advance the hash chain.
pub(crate) fn write_event(
    prepared: &mut PreparedRun,
    stage: &str,
    event_type: &str,
    payload: Value,
) -> Result<(), ErrorItem> {
    let (_, hash_self) = append_event(
        prepared.event_log_path().as_path(),
        &prepared.last_event_hash,
        &prepared.run_id,
        stage,
        event_type,
        payload,
    )?;
    prepared.last_event_hash = hash_self;
    Ok(())
}
