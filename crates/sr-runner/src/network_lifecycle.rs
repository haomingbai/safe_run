use sr_compiler::{NetworkPlan, NftRule};
use std::collections::BTreeSet;
use std::net::{IpAddr, ToSocketAddrs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedNetwork {
    pub tap_name: String,
    pub table: String,
    pub chains: Vec<String>,
    pub rules: Vec<AppliedNetworkRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedNetworkRule {
    pub chain: String,
    pub protocol: String,
    pub target: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkRuleHit {
    pub chain: String,
    pub protocol: String,
    pub target: String,
    pub port: u16,
    pub allowed_hits: u64,
    pub blocked_hits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkLifecycleError {
    pub path: String,
    pub message: String,
}

impl NetworkLifecycleError {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Network lifecycle adapter for runner apply/release hooks.
pub trait NetworkLifecycle {
    fn apply(
        &self,
        run_id: &str,
        plan: &NetworkPlan,
    ) -> Result<AppliedNetwork, NetworkLifecycleError>;
    fn sample_rule_hits(
        &self,
        _applied: &AppliedNetwork,
    ) -> Result<Vec<NetworkRuleHit>, NetworkLifecycleError> {
        Ok(Vec::new())
    }
    fn release(&self, applied: &AppliedNetwork) -> Result<(), NetworkLifecycleError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemNetworkLifecycle;

impl NetworkLifecycle for SystemNetworkLifecycle {
    fn apply(
        &self,
        run_id: &str,
        plan: &NetworkPlan,
    ) -> Result<AppliedNetwork, NetworkLifecycleError> {
        if plan.nft.chains.is_empty() {
            return Err(NetworkLifecycleError::new(
                "launch.network.apply",
                "network plan must include at least one nft chain",
            ));
        }

        let tap_name = plan.tap.name.replace("<runId>", run_id);
        let mut applied_rules = Vec::new();
        for rule in &plan.nft.rules {
            let targets = resolve_rule_targets(rule)?;
            for chain in &plan.nft.chains {
                for target in &targets {
                    applied_rules.push(AppliedNetworkRule {
                        chain: chain.clone(),
                        protocol: rule.protocol.clone(),
                        target: target.clone(),
                        port: rule.port,
                    });
                }
            }
        }

        Ok(AppliedNetwork {
            tap_name,
            table: plan.nft.table.clone(),
            chains: plan.nft.chains.clone(),
            rules: applied_rules,
        })
    }

    fn release(&self, _applied: &AppliedNetwork) -> Result<(), NetworkLifecycleError> {
        Ok(())
    }
}

fn resolve_rule_targets(rule: &NftRule) -> Result<Vec<String>, NetworkLifecycleError> {
    if let Some(cidr) = rule.cidr.as_ref() {
        return Ok(vec![cidr.clone()]);
    }

    if let Some(host) = rule.host.as_deref() {
        let resolved = resolve_host_ipv4(host, rule.port)?;
        if resolved.is_empty() {
            return Err(NetworkLifecycleError::new(
                "launch.network.dns",
                format!("host '{host}' resolved to no IPv4 addresses"),
            ));
        }
        return Ok(resolved);
    }

    Err(NetworkLifecycleError::new(
        "launch.network.apply",
        "network rule is missing host/cidr target",
    ))
}

fn resolve_host_ipv4(host: &str, port: u16) -> Result<Vec<String>, NetworkLifecycleError> {
    let addrs = (host, port).to_socket_addrs().map_err(|err| {
        NetworkLifecycleError::new(
            "launch.network.dns",
            format!("failed to resolve host '{host}': {err}"),
        )
    })?;
    let mut ipv4 = BTreeSet::new();
    for addr in addrs {
        if let IpAddr::V4(v4) = addr.ip() {
            ipv4.insert(format!("{v4}/32"));
        }
    }
    Ok(ipv4.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_compiler::{NetworkPlan, NftPlan, NftRule, TapPlan};

    fn sample_plan() -> NetworkPlan {
        NetworkPlan {
            tap: TapPlan {
                name: "sr-tap-<runId>".to_string(),
            },
            nft: NftPlan {
                table: "safe_run".to_string(),
                chains: vec!["forward".to_string()],
                rules: vec![NftRule {
                    protocol: "tcp".to_string(),
                    host: None,
                    cidr: Some("1.1.1.1/32".to_string()),
                    port: 443,
                }],
            },
        }
    }

    #[test]
    fn apply_replaces_tap_name_with_run_id() {
        let lifecycle = SystemNetworkLifecycle;
        let applied = lifecycle
            .apply("sr-20260210-001", &sample_plan())
            .expect("apply network plan");
        assert_eq!(applied.tap_name, "sr-tap-sr-20260210-001");
    }

    #[test]
    fn apply_expands_rule_targets_by_chain() {
        let lifecycle = SystemNetworkLifecycle;
        let applied = lifecycle
            .apply("sr-20260210-001", &sample_plan())
            .expect("apply network plan");
        assert_eq!(applied.rules.len(), 1);
        assert_eq!(applied.rules[0].chain, "forward");
        assert_eq!(applied.rules[0].target, "1.1.1.1/32");
    }
}
