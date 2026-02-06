use regex::Regex;
use serde::{Deserialize, Serialize};
use sr_common::{ErrorItem, SR_POL_001, SR_POL_002, SR_POL_003};

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    None,
    Allowlist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    pub source: String,
    pub target: String,
    #[serde(default)]
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

pub fn parse_policy(input: &str) -> Result<PolicySpec, ErrorItem> {
    serde_yaml::from_str::<PolicySpec>(input).map_err(|e| {
        let message = format!("failed to parse policy: {e}");
        let code = if message.contains("missing field") {
            SR_POL_001
        } else {
            SR_POL_002
        };
        ErrorItem::new(code, "policy", message)
    })
}

pub fn load_policy_from_path(path: &str) -> Result<PolicySpec, ErrorItem> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        ErrorItem::new(
            SR_POL_001,
            "policy",
            format!("failed to read policy file: {e}"),
        )
    })?;
    parse_policy(&text)
}

pub fn validate_policy(mut policy: PolicySpec) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if policy.api_version != "policy.safe-run.dev/v1alpha1" {
        errors.push(ErrorItem::new(
            SR_POL_002,
            "apiVersion",
            "apiVersion must be policy.safe-run.dev/v1alpha1",
        ));
    }

    if policy.metadata.name.trim().is_empty() {
        errors.push(ErrorItem::new(
            SR_POL_001,
            "metadata.name",
            "metadata.name is required",
        ));
    }

    if policy.runtime.command.trim().is_empty() {
        errors.push(ErrorItem::new(
            SR_POL_001,
            "runtime.command",
            "runtime.command is required",
        ));
    }

    let cpu_re = Regex::new(r"^(max|[0-9]+)\s+(max|[0-9]+)$").expect("regex");
    if !cpu_re.is_match(policy.resources.cpu.max.trim()) {
        errors.push(ErrorItem::new(
            SR_POL_002,
            "resources.cpu.max",
            "cpu.max must be '<quota> <period>'",
        ));
    }

    let mem_re = Regex::new(r"^[0-9]+(Ki|Mi|Gi)$").expect("regex");
    if !mem_re.is_match(policy.resources.memory.max.trim()) {
        errors.push(ErrorItem::new(
            SR_POL_002,
            "resources.memory.max",
            "memory.max must be like 256Mi",
        ));
    }

    if policy.network.mode != NetworkMode::None {
        errors.push(ErrorItem::new(
            SR_POL_003,
            "network.mode",
            "M0 only supports network.mode=none",
        ));
    }

    for (idx, mount) in policy.mounts.iter().enumerate() {
        if mount.source.trim().is_empty() {
            errors.push(ErrorItem::new(
                SR_POL_002,
                format!("mounts[{idx}].source"),
                "mount source cannot be empty",
            ));
        }
        if mount.target.trim().is_empty() || !mount.target.starts_with('/') {
            errors.push(ErrorItem::new(
                SR_POL_002,
                format!("mounts[{idx}].target"),
                "mount target must be an absolute path",
            ));
        }
    }

    if policy.audit.level.trim().is_empty() {
        errors.push(ErrorItem::new(
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

    #[test]
    fn validate_allowlist_is_rejected_in_m0() {
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
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        };

        let result = validate_policy(policy);
        assert!(!result.valid);
        assert_eq!(result.errors[0].code, SR_POL_003);
    }
}
