use sr_common::{ErrorItem, SR_POL_101, SR_POL_102};
use std::path::{Path, PathBuf};

const SENSITIVE_HOST_PREFIXES: [&str; 3] = ["/proc", "/sys", "/dev"];
const GUEST_DENY_PREFIXES: [&str; 12] = [
    "/", "/proc", "/sys", "/dev", "/run", "/boot", "/etc", "/bin", "/sbin", "/lib", "/lib64",
    "/usr",
];

#[derive(Debug, Clone)]
pub struct MountConstraints {
    guest_allow_prefixes: Vec<PathBuf>,
}

impl MountConstraints {
    /// Build target-path constraint validator from guest allowlist prefixes.
    pub fn new(guest_allow_prefixes: Vec<PathBuf>) -> Self {
        Self {
            guest_allow_prefixes,
        }
    }

    /// Reject sensitive host path sources even when allowlisted.
    /// Error mapping: `SR-POL-101` with `mounts[i].source`.
    pub fn validate_source_sensitive(&self, canonical: &Path, idx: usize) -> Result<(), ErrorItem> {
        if is_sensitive_host_path(canonical) {
            return Err(policy_error(
                SR_POL_101,
                format!("mounts[{idx}].source"),
                format!(
                    "mount source '{}' is within a sensitive host path",
                    canonical.display()
                ),
            ));
        }
        Ok(())
    }

    /// Validate mount target namespace and forbidden guest path denylist.
    /// Error mapping: `SR-POL-102` with `mounts[i].target`.
    pub fn validate_target_path(&self, target: &str, idx: usize) -> Result<(), ErrorItem> {
        let target_path = Path::new(target);
        if !self
            .guest_allow_prefixes
            .iter()
            .any(|prefix| target_path.starts_with(prefix))
        {
            return Err(policy_error(
                SR_POL_102,
                format!("mounts[{idx}].target"),
                format!(
                    "mount target '{}' is outside guest allowlist",
                    target_path.display()
                ),
            ));
        }

        if let Some(deny_prefix) = guest_deny_hit(target_path) {
            return Err(policy_error(
                SR_POL_102,
                format!("mounts[{idx}].target"),
                format!(
                    "mount target '{}' is within forbidden guest path '{deny_prefix}'",
                    target_path.display()
                ),
            ));
        }

        Ok(())
    }
}

fn is_sensitive_host_path(path: &Path) -> bool {
    SENSITIVE_HOST_PREFIXES
        .iter()
        .any(|prefix| is_prefix_or_exact(path, Path::new(prefix)))
}

fn guest_deny_hit(target: &Path) -> Option<&'static str> {
    for prefix in GUEST_DENY_PREFIXES {
        let prefix_path = Path::new(prefix);
        if prefix == "/" {
            if target == prefix_path {
                return Some(prefix);
            }
            continue;
        }
        if is_prefix_or_exact(target, prefix_path) {
            return Some(prefix);
        }
    }
    None
}

fn is_prefix_or_exact(path: &Path, prefix: &Path) -> bool {
    path == prefix || path.starts_with(prefix)
}

fn policy_error(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ErrorItem {
    ErrorItem::new(code, path, message)
}
