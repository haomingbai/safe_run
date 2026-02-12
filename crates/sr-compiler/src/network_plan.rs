use serde::{Deserialize, Serialize};
use sr_common::{ErrorItem, SR_CMP_201};
use sr_policy::{Network, NetworkEgressRule, NetworkMode};
use std::net::Ipv4Addr;

const TAP_NAME_TEMPLATE: &str = "sr-tap-<runId>";
const NFT_TABLE_NAME: &str = "safe_run";
const NFT_FORWARD_CHAIN: &str = "forward";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkPlan {
    pub tap: TapPlan,
    pub nft: NftPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TapPlan {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NftPlan {
    pub table: String,
    pub chains: Vec<String>,
    pub rules: Vec<NftRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NftRule {
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cidr: Option<String>,
    pub port: u16,
}

pub struct NetworkPlanBuilder;

impl NetworkPlanBuilder {
    /// Build deterministic M3 `networkPlan` from normalized policy network config.
    /// Error mapping: invalid or unsupported network compile inputs -> `SR-CMP-201`.
    pub fn build(network: &Network) -> Result<Option<NetworkPlan>, ErrorItem> {
        match network.mode {
            NetworkMode::None => {
                if network.egress.is_empty() {
                    return Ok(None);
                }
                Err(cmp201(
                    "network.egress",
                    "network.egress must be empty when network.mode=none",
                ))
            }
            NetworkMode::Allowlist => build_allowlist_plan(network),
        }
    }
}

fn build_allowlist_plan(network: &Network) -> Result<Option<NetworkPlan>, ErrorItem> {
    if network.egress.is_empty() {
        return Err(cmp201(
            "network.egress",
            "network.egress must contain at least one rule when network.mode=allowlist",
        ));
    }

    let mut rules = Vec::with_capacity(network.egress.len());
    for (idx, rule) in network.egress.iter().enumerate() {
        rules.push(build_rule(rule, idx)?);
    }

    Ok(Some(NetworkPlan {
        tap: TapPlan {
            name: TAP_NAME_TEMPLATE.to_string(),
        },
        nft: NftPlan {
            table: NFT_TABLE_NAME.to_string(),
            chains: vec![NFT_FORWARD_CHAIN.to_string()],
            rules,
        },
    }))
}

fn build_rule(rule: &NetworkEgressRule, idx: usize) -> Result<NftRule, ErrorItem> {
    let protocol = normalize(rule.protocol.as_deref())
        .filter(|value| matches!(*value, "tcp" | "udp"))
        .ok_or_else(|| cmp201(field_path(idx, "protocol"), "protocol must be tcp or udp"))?
        .to_string();

    let port = rule
        .port
        .filter(|value| (1..=65535).contains(value))
        .ok_or_else(|| cmp201(field_path(idx, "port"), "port must be within 1..=65535"))?
        as u16;

    let host = normalize(rule.host.as_deref()).map(ToString::to_string);
    let cidr = normalize(rule.cidr.as_deref()).map(ToString::to_string);
    if host.is_some() == cidr.is_some() {
        return Err(cmp201(
            field_path(idx, "host"),
            "exactly one of host or cidr must be set",
        ));
    }
    if let Some(raw) = cidr.as_deref() {
        if !is_valid_ipv4_cidr(raw) {
            return Err(cmp201(
                field_path(idx, "cidr"),
                "cidr must be an IPv4 CIDR, for example 1.2.3.4/32",
            ));
        }
    }

    Ok(NftRule {
        protocol,
        host,
        cidr,
        port,
    })
}

fn normalize(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|item| !item.is_empty())
}

fn is_valid_ipv4_cidr(cidr: &str) -> bool {
    let Some((ip_raw, prefix_raw)) = cidr.split_once('/') else {
        return false;
    };
    let Ok(prefix) = prefix_raw.parse::<u8>() else {
        return false;
    };
    prefix <= 32 && ip_raw.parse::<Ipv4Addr>().is_ok()
}

fn field_path(idx: usize, field: &str) -> String {
    format!("network.egress[{idx}].{field}")
}

fn cmp201(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_CMP_201, path, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_policy::NetworkEgressRule;

    fn sample_rule() -> NetworkEgressRule {
        NetworkEgressRule {
            protocol: Some("tcp".to_string()),
            host: Some("api.example.com".to_string()),
            cidr: None,
            port: Some(443),
        }
    }

    #[test]
    fn none_mode_generates_null_network_plan() {
        let network = Network {
            mode: NetworkMode::None,
            egress: vec![],
        };
        let plan = NetworkPlanBuilder::build(&network).expect("build none network plan");
        assert!(plan.is_none());
    }

    #[test]
    fn allowlist_mode_generates_forward_chain_plan() {
        let network = Network {
            mode: NetworkMode::Allowlist,
            egress: vec![sample_rule()],
        };
        let plan = NetworkPlanBuilder::build(&network)
            .expect("build allowlist network plan")
            .expect("allowlist should be non-null");
        assert_eq!(plan.tap.name, TAP_NAME_TEMPLATE);
        assert_eq!(plan.nft.table, NFT_TABLE_NAME);
        assert_eq!(plan.nft.chains, vec![NFT_FORWARD_CHAIN.to_string()]);
        assert_eq!(plan.nft.rules.len(), 1);
    }

    #[test]
    fn invalid_rule_returns_sr_cmp_201() {
        let network = Network {
            mode: NetworkMode::Allowlist,
            egress: vec![NetworkEgressRule {
                protocol: Some("icmp".to_string()),
                host: Some("api.example.com".to_string()),
                cidr: None,
                port: Some(443),
            }],
        };
        let err = NetworkPlanBuilder::build(&network)
            .expect_err("invalid protocol should fail compile build");
        assert_eq!(err.code, SR_CMP_201);
        assert_eq!(err.path, "network.egress[0].protocol");
    }
}
