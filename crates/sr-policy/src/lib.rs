use regex::Regex;
use serde::{Deserialize, Serialize};
use sr_common::{ErrorItem, SR_POL_001, SR_POL_002, SR_POL_103};

mod mount_constraints;
mod network_constraints;
mod path_security;
use mount_constraints::MountConstraints;
use network_constraints::validate_network_constraints;
use path_security::PathSecurityEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySpec {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub metadata: Metadata,
    pub runtime: Runtime,
    pub resources: Resources,
    pub network: Network,
    pub mounts: Vec<Mount>,
    pub audit: Audit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runtime {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resources {
    pub cpu: Cpu,
    pub memory: Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cpu {
    pub max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub mode: NetworkMode,
    #[serde(default)]
    pub egress: Vec<NetworkEgressRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    None,
    Allowlist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEgressRule {
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub cidr: Option<String>,
    #[serde(default)]
    pub port: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    #[serde(alias = "hostPath")]
    pub source: String,
    #[serde(alias = "guestPath")]
    pub target: String,
    #[serde(default, alias = "readOnly")]
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audit {
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ErrorItem>,
    pub warnings: Vec<String>,
    #[serde(rename = "normalizedPolicy")]
    pub normalized_policy: Option<PolicySpec>,
}

/// Parse policy YAML/JSON into `PolicySpec`.
/// Error mapping: missing required fields -> `SR-POL-001`, invalid structure -> `SR-POL-002`.
pub fn parse_policy(input: &str) -> Result<PolicySpec, ErrorItem> {
    serde_yaml::from_str::<PolicySpec>(input).map_err(|e| {
        let message = format!("failed to parse policy: {e}");
        let code = if message.contains("missing field") {
            SR_POL_001
        } else {
            SR_POL_002
        };
        pol_error(code, "policy", message)
    })
}

/// Load and parse policy file from disk.
/// Error mapping: file read failures -> `SR-POL-001`, parse failures follow `parse_policy`.
pub fn load_policy_from_path(path: &str) -> Result<PolicySpec, ErrorItem> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        pol_error(
            SR_POL_001,
            "policy",
            format!("failed to read policy file: {e}"),
        )
    })?;
    parse_policy(&text)
}

/// Validate policy with default allowlist source resolution.
pub fn validate_policy(policy: PolicySpec) -> ValidationResult {
    validate_policy_with_allowlist(policy, None)
}

/// Validate policy semantics and emit normalized policy on success.
/// Boundary: M3 allows `network.mode=allowlist`; mounts still pass allowlist + constraint checks.
/// Error mapping: `SR-POL-001/002/101/102/103/201` with field-oriented `path`.
pub fn validate_policy_with_allowlist(
    mut policy: PolicySpec,
    allowlist_path: Option<&str>,
) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let allowlist_engine = match PathSecurityEngine::from_sources(allowlist_path) {
        Ok(engine) => Some(engine),
        Err(err) => {
            errors.push(err);
            None
        }
    };
    let mount_constraints = allowlist_engine
        .as_ref()
        .map(|engine| MountConstraints::new(engine.guest_allow_prefixes().to_vec()));

    if policy.api_version != "policy.safe-run.dev/v1alpha1" {
        errors.push(pol_error(
            SR_POL_002,
            "apiVersion",
            "apiVersion must be policy.safe-run.dev/v1alpha1",
        ));
    }

    if policy.metadata.name.trim().is_empty() {
        errors.push(pol_error(
            SR_POL_001,
            "metadata.name",
            "metadata.name is required",
        ));
    }

    if policy.runtime.command.trim().is_empty() {
        errors.push(pol_error(
            SR_POL_001,
            "runtime.command",
            "runtime.command is required",
        ));
    }

    let cpu_re = Regex::new(r"^(max|[0-9]+)\s+(max|[0-9]+)$").expect("regex");
    if !cpu_re.is_match(policy.resources.cpu.max.trim()) {
        errors.push(pol_error(
            SR_POL_002,
            "resources.cpu.max",
            "cpu.max must be '<quota> <period>'",
        ));
    }

    let mem_re = Regex::new(r"^[0-9]+(Ki|Mi|Gi)$").expect("regex");
    if !mem_re.is_match(policy.resources.memory.max.trim()) {
        errors.push(pol_error(
            SR_POL_002,
            "resources.memory.max",
            "memory.max must be like 256Mi",
        ));
    }

    errors.extend(validate_network_constraints(&policy.network));

    for (idx, mount) in policy.mounts.iter().enumerate() {
        let mut source_valid = true;
        let mut target_valid = true;
        let mut source_canonical = None;
        if mount.source.trim().is_empty() {
            errors.push(pol_error(
                SR_POL_002,
                mount_field_path(idx, "source"),
                "mount source cannot be empty",
            ));
            source_valid = false;
        }
        if mount.target.trim().is_empty() || !mount.target.starts_with('/') {
            errors.push(pol_error(
                SR_POL_002,
                mount_field_path(idx, "target"),
                "mount target must be an absolute path",
            ));
            target_valid = false;
        }
        if !mount.read_only {
            errors.push(pol_error(
                SR_POL_103,
                mount_field_path(idx, "read_only"),
                "mounts must be read-only in M2",
            ));
        }
        if let Some(engine) = allowlist_engine.as_ref() {
            if source_valid {
                match engine.validate_source_path(&mount.source, idx) {
                    Ok(canonical) => {
                        source_canonical = Some(canonical);
                    }
                    Err(err) => {
                        errors.push(err);
                    }
                }
            }
        }
        if let Some(constraints) = mount_constraints.as_ref() {
            if let Some(canonical) = source_canonical.as_ref() {
                if let Err(err) = constraints.validate_source_sensitive(canonical, idx) {
                    errors.push(err);
                }
            }
            if target_valid {
                if let Err(err) = constraints.validate_target_path(&mount.target, idx) {
                    errors.push(err);
                }
            }
        }
    }

    if policy.audit.level.trim().is_empty() {
        errors.push(pol_error(
            SR_POL_001,
            "audit.level",
            "audit.level is required",
        ));
    }

    if errors.is_empty() {
        policy.runtime.args.retain(|arg| !arg.trim().is_empty());
        warnings.push("default deny policy is active".to_string());
        ValidationResult {
            valid: true,
            errors,
            warnings,
            normalized_policy: Some(policy),
        }
    } else {
        ValidationResult {
            valid: false,
            errors,
            warnings,
            normalized_policy: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_common::SR_POL_201;

    #[test]
    fn validate_allowlist_without_egress_is_rejected() {
        let policy = PolicySpec {
            api_version: "policy.safe-run.dev/v1alpha1".to_string(),
            metadata: Metadata {
                name: "demo".to_string(),
            },
            runtime: Runtime {
                command: "/bin/echo".to_string(),
                args: vec!["ok".to_string()],
            },
            resources: Resources {
                cpu: Cpu {
                    max: "100000 100000".to_string(),
                },
                memory: Memory {
                    max: "256Mi".to_string(),
                },
            },
            network: Network {
                mode: NetworkMode::Allowlist,
                egress: vec![],
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        };

        let result = validate_policy(policy);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|err| err.code == SR_POL_201));
        assert!(result.errors.iter().any(|err| err.path == "network.egress"));
    }
}

fn pol_error(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ErrorItem {
    ErrorItem::new(code, path, message)
}

fn mount_field_path(idx: usize, field: &str) -> String {
    format!("mounts[{idx}].{field}")
}
