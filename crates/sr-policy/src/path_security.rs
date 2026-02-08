use serde::Deserialize;
use sr_common::{ErrorItem, SR_POL_101};
use std::path::{Path, PathBuf};

const DEFAULT_HOST_ALLOW_PREFIXES: [&str; 1] = ["/var/lib/safe-run"];
const DEFAULT_GUEST_ALLOW_PREFIXES: [&str; 1] = ["/data"];
const ALLOWLIST_SCHEMA_VERSION: &str = "safe-run.mount-allowlist/v1";

#[derive(Debug, Clone)]
pub struct MountAllowlist {
    pub host_allow_prefixes: Vec<PathBuf>,
    #[allow(dead_code)]
    pub guest_allow_prefixes: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct MountAllowlistConfig {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    #[serde(rename = "hostAllowPrefixes")]
    host_allow_prefixes: Vec<String>,
    #[serde(rename = "guestAllowPrefixes")]
    guest_allow_prefixes: Vec<String>,
}

impl MountAllowlist {
    pub fn default_allowlist() -> Self {
        Self {
            host_allow_prefixes: DEFAULT_HOST_ALLOW_PREFIXES
                .iter()
                .map(PathBuf::from)
                .collect(),
            guest_allow_prefixes: DEFAULT_GUEST_ALLOW_PREFIXES
                .iter()
                .map(PathBuf::from)
                .collect(),
        }
    }

    pub fn from_file(path: &Path) -> Result<Self, ErrorItem> {
        let raw = std::fs::read_to_string(path).map_err(|err| {
            ErrorItem::new(
                SR_POL_101,
                "mountAllowlist",
                format!("failed to read allowlist file '{}': {err}", path.display()),
            )
        })?;
        let config: MountAllowlistConfig = serde_yaml::from_str(&raw).map_err(|err| {
            ErrorItem::new(
                SR_POL_101,
                "mountAllowlist",
                format!("failed to parse allowlist file '{}': {err}", path.display()),
            )
        })?;
        if config.schema_version != ALLOWLIST_SCHEMA_VERSION {
            return Err(ErrorItem::new(
                SR_POL_101,
                "mountAllowlist.schemaVersion",
                format!(
                    "allowlist schemaVersion must be '{}'",
                    ALLOWLIST_SCHEMA_VERSION
                ),
            ));
        }
        let host_allow_prefixes = parse_prefixes(
            &config.host_allow_prefixes,
            "mountAllowlist.hostAllowPrefixes",
        )?;
        let guest_allow_prefixes = parse_prefixes(
            &config.guest_allow_prefixes,
            "mountAllowlist.guestAllowPrefixes",
        )?;
        Ok(Self {
            host_allow_prefixes,
            guest_allow_prefixes,
        })
    }
}

pub struct PathSecurityEngine {
    allowlist: MountAllowlist,
}

impl PathSecurityEngine {
    pub fn from_sources(explicit_path: Option<&str>) -> Result<Self, ErrorItem> {
        let allowlist = if let Some(path) = explicit_path {
            MountAllowlist::from_file(Path::new(path))?
        } else if let Ok(path) = std::env::var("SAFE_RUN_MOUNT_ALLOWLIST") {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                MountAllowlist::default_allowlist()
            } else {
                MountAllowlist::from_file(Path::new(trimmed))?
            }
        } else {
            MountAllowlist::default_allowlist()
        };
        Ok(Self { allowlist })
    }

    pub fn validate_source_path(&self, source: &str, idx: usize) -> Result<(), ErrorItem> {
        let source_path = Path::new(source);
        let canonical = match std::fs::canonicalize(source_path) {
            Ok(path) => path,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                normalize_path_lexically(source_path)
            }
            Err(err) => {
                return Err(ErrorItem::new(
                    SR_POL_101,
                    format!("mounts[{idx}].source"),
                    format!(
                        "failed to canonicalize mount source '{}': {err}",
                        source_path.display()
                    ),
                ));
            }
        };
        if !self
            .allowlist
            .host_allow_prefixes
            .iter()
            .any(|prefix| canonical.starts_with(prefix))
        {
            return Err(ErrorItem::new(
                SR_POL_101,
                format!("mounts[{idx}].source"),
                format!(
                    "mount source '{}' is outside host allowlist",
                    canonical.display()
                ),
            ));
        }
        Ok(())
    }
}

fn parse_prefixes(prefixes: &[String], path_label: &str) -> Result<Vec<PathBuf>, ErrorItem> {
    let mut parsed = Vec::new();
    for prefix in prefixes {
        let trimmed = prefix.trim();
        if trimmed.is_empty() {
            return Err(ErrorItem::new(
                SR_POL_101,
                path_label,
                "allowlist prefix cannot be empty",
            ));
        }
        let path = PathBuf::from(trimmed);
        if !path.is_absolute() {
            return Err(ErrorItem::new(
                SR_POL_101,
                path_label,
                format!("allowlist prefix must be absolute: {trimmed}"),
            ));
        }
        parsed.push(path);
    }
    Ok(parsed)
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(Path::new("/")),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}
