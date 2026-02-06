use serde::{Deserialize, Serialize};
use serde_json::json;
use sr_common::{ErrorItem, SR_CMP_001, SR_CMP_002};
use sr_policy::PolicySpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileBundle {
    #[serde(rename = "firecrackerConfig")]
    pub firecracker_config: serde_json::Value,
    #[serde(rename = "jailerPlan")]
    pub jailer_plan: Plan,
    #[serde(rename = "cgroupPlan")]
    pub cgroup_plan: Plan,
    #[serde(rename = "networkPlan")]
    pub network_plan: Option<serde_json::Value>,
    #[serde(rename = "evidencePlan")]
    pub evidence_plan: EvidencePlan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub enabled: bool,
    pub ops: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidencePlan {
    pub enabled: bool,
    pub events: Vec<String>,
}

pub fn compile_dry_run(policy: &PolicySpec) -> Result<CompileBundle, ErrorItem> {
    if policy.runtime.command.trim().is_empty() {
        return Err(ErrorItem::new(
            SR_CMP_002,
            "runtime.command",
            "runtime.command is empty after normalization",
        ));
    }

    let mem_size_mib = memory_to_mib(&policy.resources.memory.max).ok_or_else(|| {
        ErrorItem::new(
            SR_CMP_001,
            "resources.memory.max",
            "compile template cannot map memory.max to MiB",
        )
    })?;

    let firecracker_config = json!({
        "machine-config": {
            "vcpu_count": 1,
            "mem_size_mib": mem_size_mib,
            "smt": false
        },
        "boot-source": {
            "kernel_image_path": "artifacts/vmlinux",
            "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
        },
        "drives": []
    });

    let bundle = CompileBundle {
        firecracker_config,
        jailer_plan: Plan {
            enabled: true,
            ops: vec!["prepare_jailer_context".to_string()],
        },
        cgroup_plan: Plan {
            enabled: true,
            ops: vec![
                format!("set_cpu_max={}", policy.resources.cpu.max),
                format!("set_memory_max={}", policy.resources.memory.max),
            ],
        },
        network_plan: None,
        evidence_plan: EvidencePlan {
            enabled: true,
            events: vec!["compile".to_string()],
        },
    };
    ensure_bundle_complete(&bundle)?;
    Ok(bundle)
}

fn memory_to_mib(memory: &str) -> Option<u64> {
    if let Some(raw) = memory.strip_suffix("Mi") {
        return raw.parse::<u64>().ok();
    }
    if let Some(raw) = memory.strip_suffix("Gi") {
        return raw.parse::<u64>().ok().map(|n| n * 1024);
    }
    if let Some(raw) = memory.strip_suffix("Ki") {
        return raw.parse::<u64>().ok().map(|n| n / 1024);
    }
    None
}

fn ensure_bundle_complete(bundle: &CompileBundle) -> Result<(), ErrorItem> {
    if bundle
        .firecracker_config
        .get("machine-config")
        .is_none()
    {
        return Err(ErrorItem::new(
            SR_CMP_002,
            "firecrackerConfig.machine-config",
            "compile output is missing machine-config",
        ));
    }

    if bundle.jailer_plan.ops.is_empty() {
        return Err(ErrorItem::new(
            SR_CMP_002,
            "jailerPlan.ops",
            "compile output is missing jailer operations",
        ));
    }

    if bundle.cgroup_plan.ops.is_empty() {
        return Err(ErrorItem::new(
            SR_CMP_002,
            "cgroupPlan.ops",
            "compile output is missing cgroup operations",
        ));
    }

    if bundle.network_plan.is_some() {
        return Err(ErrorItem::new(
            SR_CMP_002,
            "networkPlan",
            "M0 compile output must keep networkPlan as null",
        ));
    }

    if bundle.evidence_plan.events.is_empty() {
        return Err(ErrorItem::new(
            SR_CMP_002,
            "evidencePlan.events",
            "compile output is missing evidence events",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_policy::{Audit, Cpu, Memory, Metadata, Network, NetworkMode, Resources, Runtime};

    #[test]
    fn compile_bundle_network_is_none() {
        let policy = PolicySpec {
            api_version: "policy.safe-run.dev/v1alpha1".to_string(),
            metadata: Metadata {
                name: "demo".to_string(),
            },
            runtime: Runtime {
                command: "/bin/echo".to_string(),
                args: vec!["hello".to_string()],
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
                mode: NetworkMode::None,
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        };

        let bundle = compile_dry_run(&policy).expect("compile bundle");
        assert!(bundle.network_plan.is_none());
    }

    #[test]
    fn compile_invalid_memory_format_returns_cmp_001() {
        let policy = PolicySpec {
            api_version: "policy.safe-run.dev/v1alpha1".to_string(),
            metadata: Metadata {
                name: "demo".to_string(),
            },
            runtime: Runtime {
                command: "/bin/echo".to_string(),
                args: vec!["hello".to_string()],
            },
            resources: Resources {
                cpu: Cpu {
                    max: "100000 100000".to_string(),
                },
                memory: Memory {
                    max: "256MB".to_string(),
                },
            },
            network: Network {
                mode: NetworkMode::None,
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        };

        let err = compile_dry_run(&policy).expect_err("invalid memory must fail");
        assert_eq!(err.code, SR_CMP_001);
    }
}
