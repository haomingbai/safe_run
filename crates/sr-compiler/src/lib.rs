use serde::{Deserialize, Serialize};
use serde_json::json;
use sr_common::{ErrorItem, SR_CMP_001, SR_CMP_002, SR_CMP_201};
use sr_evidence::{
    EVENT_NETWORK_PLAN_GENERATED, EVENT_NETWORK_RULE_APPLIED, EVENT_NETWORK_RULE_CLEANUP_FAILED,
    EVENT_NETWORK_RULE_HIT, EVENT_NETWORK_RULE_RELEASED, REQUIRED_EVIDENCE_EVENTS,
};
use sr_policy::{NetworkMode, PolicySpec};

mod mount_plan;
mod network_plan;
use mount_plan::MountPlanBuilder;
pub use mount_plan::{MountPlan, MountPlanEntry};
use network_plan::NetworkPlanBuilder;
pub use network_plan::{NetworkPlan, NftPlan, NftRule, TapPlan};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileBundle {
    #[serde(rename = "firecrackerConfig")]
    pub firecracker_config: serde_json::Value,
    #[serde(rename = "jailerPlan")]
    pub jailer_plan: Plan,
    #[serde(rename = "cgroupPlan")]
    pub cgroup_plan: Plan,
    #[serde(rename = "mountPlan")]
    pub mount_plan: MountPlan,
    #[serde(rename = "networkPlan")]
    pub network_plan: Option<NetworkPlan>,
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

const ALLOWLIST_NETWORK_EVENTS: [&str; 5] = [
    EVENT_NETWORK_PLAN_GENERATED,
    EVENT_NETWORK_RULE_APPLIED,
    EVENT_NETWORK_RULE_HIT,
    EVENT_NETWORK_RULE_RELEASED,
    EVENT_NETWORK_RULE_CLEANUP_FAILED,
];

/// Compile a validated `PolicySpec` into a deterministic `CompileBundle`.
/// Boundary: M3 allows `network.mode=allowlist`; `networkPlan` must be null for none and non-null for allowlist.
/// Error mapping: `SR-CMP-001` for template mapping failures, `SR-CMP-002` for invalid request/output, `SR-CMP-201` for network plan failures.
pub fn compile_dry_run(policy: &PolicySpec) -> Result<CompileBundle, ErrorItem> {
    if policy.runtime.command.trim().is_empty() {
        return Err(cmp_error(
            "runtime.command",
            "runtime.command is empty after normalization",
        ));
    }

    let mem_size_mib = memory_to_mib(&policy.resources.memory.max).ok_or_else(|| {
        cmp_template_error(
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
        "drives": [],
        "rootfs": {
            "path": "artifacts/rootfs.ext4",
            "readOnly": true
        }
    });

    let mount_plan = MountPlanBuilder::build(&policy.mounts);
    let network_plan = NetworkPlanBuilder::build(&policy.network)?;

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
        mount_plan,
        network_plan,
        evidence_plan: EvidencePlan {
            enabled: true,
            events: required_evidence_events(&policy.network.mode),
        },
    };
    ensure_bundle_complete(&bundle, &policy.network.mode)?;
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

fn ensure_bundle_complete(
    bundle: &CompileBundle,
    network_mode: &NetworkMode,
) -> Result<(), ErrorItem> {
    if bundle.firecracker_config.get("machine-config").is_none() {
        return Err(cmp_error(
            "firecrackerConfig.machine-config",
            "compile output is missing machine-config",
        ));
    }

    if bundle.firecracker_config.get("boot-source").is_none() {
        return Err(cmp_error(
            "firecrackerConfig.boot-source",
            "compile output is missing boot-source",
        ));
    }

    let drives_empty = bundle
        .firecracker_config
        .get("drives")
        .and_then(|drives| drives.as_array())
        .map(|drives| drives.is_empty())
        .unwrap_or(true);
    let rootfs_missing = bundle.firecracker_config.get("rootfs").is_none();
    if drives_empty && rootfs_missing {
        return Err(cmp_error(
            "firecrackerConfig.rootfs",
            "compile output is missing rootfs description",
        ));
    }

    if bundle.jailer_plan.ops.is_empty() {
        return Err(cmp_error(
            "jailerPlan.ops",
            "compile output is missing jailer operations",
        ));
    }

    if bundle.cgroup_plan.ops.is_empty() {
        return Err(cmp_error(
            "cgroupPlan.ops",
            "compile output is missing cgroup operations",
        ));
    }

    if !bundle.mount_plan.enabled {
        return Err(cmp_error(
            "mountPlan.enabled",
            "compile output mountPlan must be enabled",
        ));
    }

    match network_mode {
        NetworkMode::None => {
            if bundle.network_plan.is_some() {
                return Err(cmp_error(
                    "networkPlan",
                    "compile output must keep networkPlan as null when network.mode=none",
                ));
            }
        }
        NetworkMode::Allowlist => {
            let plan = bundle.network_plan.as_ref().ok_or_else(|| {
                cmp_network_error(
                    "networkPlan",
                    "compile output must provide networkPlan when network.mode=allowlist",
                )
            })?;
            if !plan.nft.chains.iter().any(|chain| chain == "forward") {
                return Err(cmp_network_error(
                    "networkPlan.nft.chains",
                    "networkPlan must include nft forward chain",
                ));
            }
        }
    }

    if bundle.evidence_plan.events.is_empty() {
        return Err(cmp_error(
            "evidencePlan.events",
            "compile output is missing evidence events",
        ));
    }

    let required = required_evidence_events(network_mode);
    for event in required {
        if !bundle
            .evidence_plan
            .events
            .iter()
            .any(|item| item == &event)
        {
            return Err(cmp_error(
                "evidencePlan.events",
                format!("compile output is missing evidence event: {event}"),
            ));
        }
    }

    Ok(())
}

fn required_evidence_events(network_mode: &NetworkMode) -> Vec<String> {
    let mut events = REQUIRED_EVIDENCE_EVENTS
        .iter()
        .map(|event| event.to_string())
        .collect::<Vec<String>>();
    if matches!(network_mode, NetworkMode::Allowlist) {
        events.extend(
            ALLOWLIST_NETWORK_EVENTS
                .iter()
                .map(|event| event.to_string()),
        );
    }
    events
}

fn cmp_template_error(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_CMP_001, path, message)
}

fn cmp_error(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_CMP_002, path, message)
}

fn cmp_network_error(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_CMP_201, path, message)
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
                egress: vec![],
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
    fn compile_allowlist_network_generates_plan() {
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
                mode: NetworkMode::Allowlist,
                egress: vec![sr_policy::NetworkEgressRule {
                    protocol: Some("tcp".to_string()),
                    host: Some("api.example.com".to_string()),
                    cidr: None,
                    port: Some(443),
                }],
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        };

        let bundle = compile_dry_run(&policy).expect("allowlist should compile in M3");
        let network_plan = bundle
            .network_plan
            .expect("allowlist compile output should include network plan");
        assert_eq!(network_plan.tap.name, "sr-tap-<runId>");
        assert_eq!(network_plan.nft.table, "safe_run");
        assert_eq!(network_plan.nft.chains, vec!["forward".to_string()]);
        assert_eq!(network_plan.nft.rules.len(), 1);
        assert!(bundle
            .evidence_plan
            .events
            .iter()
            .any(|event| event == EVENT_NETWORK_RULE_HIT));
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
                egress: vec![],
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        };

        let err = compile_dry_run(&policy).expect_err("invalid memory must fail");
        assert_eq!(err.code, SR_CMP_001);
    }

    #[test]
    fn compile_empty_command_returns_cmp_002() {
        let policy = PolicySpec {
            api_version: "policy.safe-run.dev/v1alpha1".to_string(),
            metadata: Metadata {
                name: "demo".to_string(),
            },
            runtime: Runtime {
                command: "".to_string(),
                args: vec![],
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
                egress: vec![],
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        };

        let err = compile_dry_run(&policy).expect_err("empty command must fail");
        assert_eq!(err.code, SR_CMP_002);
    }

    #[test]
    fn ensure_bundle_complete_detects_missing_machine_config() {
        let mut bundle = CompileBundle {
            firecracker_config: json!({
                "boot-source": {
                    "kernel_image_path": "artifacts/vmlinux",
                    "boot_args": "console=ttyS0"
                }
            }),
            jailer_plan: Plan {
                enabled: true,
                ops: vec!["prepare_jailer_context".to_string()],
            },
            cgroup_plan: Plan {
                enabled: true,
                ops: vec!["set_cpu_max=100000 100000".to_string()],
            },
            mount_plan: MountPlan {
                enabled: true,
                mounts: vec![],
            },
            network_plan: None,
            evidence_plan: EvidencePlan {
                enabled: true,
                events: required_evidence_events(&NetworkMode::None),
            },
        };

        let err = ensure_bundle_complete(&bundle, &NetworkMode::None)
            .expect_err("missing machine-config must fail");
        assert_eq!(err.code, SR_CMP_002);

        bundle.firecracker_config = json!({
            "machine-config": {"vcpu_count": 1, "mem_size_mib": 128, "smt": false}
        });
        let err = ensure_bundle_complete(&bundle, &NetworkMode::None)
            .expect_err("missing boot-source must fail");
        assert_eq!(err.code, SR_CMP_002);
    }

    #[test]
    fn ensure_bundle_complete_detects_missing_evidence_event() {
        let bundle = CompileBundle {
            firecracker_config: json!({
                "machine-config": {
                    "vcpu_count": 1,
                    "mem_size_mib": 128,
                    "smt": false
                }
            }),
            jailer_plan: Plan {
                enabled: true,
                ops: vec!["prepare_jailer_context".to_string()],
            },
            cgroup_plan: Plan {
                enabled: true,
                ops: vec!["set_cpu_max=100000 100000".to_string()],
            },
            mount_plan: MountPlan {
                enabled: true,
                mounts: vec![],
            },
            network_plan: None,
            evidence_plan: EvidencePlan {
                enabled: true,
                events: vec![sr_evidence::EVENT_RUN_PREPARED.to_string()],
            },
        };

        let err = ensure_bundle_complete(&bundle, &NetworkMode::None)
            .expect_err("missing evidence events must fail");
        assert_eq!(err.code, SR_CMP_002);
    }

    #[test]
    fn ensure_bundle_complete_requires_network_plan_for_allowlist() {
        let bundle = CompileBundle {
            firecracker_config: json!({
                "machine-config": {
                    "vcpu_count": 1,
                    "mem_size_mib": 128,
                    "smt": false
                },
                "boot-source": {
                    "kernel_image_path": "artifacts/vmlinux",
                    "boot_args": "console=ttyS0"
                },
                "rootfs": {
                    "path": "artifacts/rootfs.ext4",
                    "readOnly": true
                },
                "drives": []
            }),
            jailer_plan: Plan {
                enabled: true,
                ops: vec!["prepare_jailer_context".to_string()],
            },
            cgroup_plan: Plan {
                enabled: true,
                ops: vec!["set_cpu_max=100000 100000".to_string()],
            },
            mount_plan: MountPlan {
                enabled: true,
                mounts: vec![],
            },
            network_plan: None,
            evidence_plan: EvidencePlan {
                enabled: true,
                events: required_evidence_events(&NetworkMode::Allowlist),
            },
        };

        let err = ensure_bundle_complete(&bundle, &NetworkMode::Allowlist)
            .expect_err("allowlist compile output must include network plan");
        assert_eq!(err.code, SR_CMP_201);
        assert_eq!(err.path, "networkPlan");
    }
}
