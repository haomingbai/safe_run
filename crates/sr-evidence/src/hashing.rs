use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use crate::EvidenceEvent;
use serde_json::Value;

/// Compute sha256 digest of bytes and return with prefix.
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Compute sha256 digest of UTF-8 string and return with prefix.
pub fn sha256_string(data: &str) -> String {
    sha256_bytes(data.as_bytes())
}

/// Normalize a JSON value with sorted object keys and no whitespace.
pub fn normalize_json_string(value: &Value) -> String {
    let canonical = canonicalize_value(value);
    serde_json::to_string(&canonical).unwrap_or_default()
}

/// Compute sha256 digest from a normalized JSON value.
pub fn sha256_json_value(value: &Value) -> String {
    let normalized = normalize_json_string(value);
    sha256_string(&normalized)
}

/// Compute sha256 digest of a file on disk and return with prefix.
pub fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    Ok(sha256_bytes(&bytes))
}

/// Derive a stable hash for an evidence event from its semantic fields.
pub fn derive_event_hash(event: &EvidenceEvent) -> String {
    let payload_json = serde_json::to_string(&event.payload).unwrap_or_default();
    let material = format!(
        "{}|{}|{}|{}|{}|{}",
        event.hash_prev, event.timestamp, event.run_id, event.stage, event.event_type, payload_json
    );
    sha256_string(&material)
}

fn canonicalize_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut new_map = serde_json::Map::new();
            for key in keys {
                if let Some(child) = map.get(key) {
                    new_map.insert(key.clone(), canonicalize_value(child));
                }
            }
            Value::Object(new_map)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_value).collect()),
        _ => value.clone(),
    }
}
