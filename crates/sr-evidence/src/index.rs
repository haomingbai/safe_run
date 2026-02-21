use serde::{Deserialize, Serialize};
use sr_common::{ErrorItem, SR_OPS_301};
use std::fs;
use std::path::Path;

const ARCHIVE_INDEX_FILE: &str = "index.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ArchiveIndex {
    pub entries: Vec<ArchiveIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveIndexEntry {
    #[serde(rename = "bundleId")]
    pub bundle_id: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "storedAt")]
    pub stored_at: String,
    pub retention: String,
    pub result: String,
}

pub fn load_archive_index(archive_root: &Path) -> Result<ArchiveIndex, ErrorItem> {
    let index_path = archive_root.join(ARCHIVE_INDEX_FILE);
    if !index_path.exists() {
        return Ok(ArchiveIndex::default());
    }

    let raw = fs::read_to_string(&index_path).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.index",
            format!("failed to read archive index '{}': {err}", index_path.display()),
        )
    })?;
    serde_json::from_str::<ArchiveIndex>(&raw).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.index",
            format!(
                "failed to parse archive index '{}': {err}",
                index_path.display()
            ),
        )
    })
}

pub fn append_archive_index(
    archive_root: &Path,
    entry: ArchiveIndexEntry,
) -> Result<(), ErrorItem> {
    let mut index = load_archive_index(archive_root)?;
    index.entries.push(entry);

    let index_path = archive_root.join(ARCHIVE_INDEX_FILE);
    let content = serde_json::to_string_pretty(&index).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.index",
            format!("failed to serialize archive index: {err}"),
        )
    })?;
    fs::write(&index_path, content).map_err(|err| {
        ErrorItem::new(
            SR_OPS_301,
            "archive.index",
            format!("failed to write archive index '{}': {err}", index_path.display()),
        )
    })
}
