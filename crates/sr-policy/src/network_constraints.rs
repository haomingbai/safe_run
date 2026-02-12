use std::net::Ipv4Addr;

use crate::{Network, NetworkEgressRule, NetworkMode};
use sr_common::{ErrorItem, SR_POL_201};

/// Validate M3 allowlist network constraints and return field-oriented errors.
pub fn validate_network_constraints(network: &Network) -> Vec<ErrorItem> {
    if network.mode == NetworkMode::None {
        if !network.egress.is_empty() {
            return vec![pol201(
                "network.egress",
                "network.egress must be empty when network.mode=none",
            )];
        }
        return Vec::new();
    }
    if network.egress.is_empty() {
        return vec![pol201(
            "network.egress",
            "network.egress must contain at least one rule when network.mode=allowlist",
        )];
    }
    let mut errors = Vec::new();
    for (idx, rule) in network.egress.iter().enumerate() {
        validate_rule(rule, idx, &mut errors);
    }
    errors
}

fn validate_rule(rule: &NetworkEgressRule, idx: usize, errors: &mut Vec<ErrorItem>) {
    if !is_allowed_protocol(rule.protocol.as_deref()) {
        errors.push(pol201(
            field_path(idx, "protocol"),
            "protocol must be tcp or udp",
        ));
    }
    if !is_allowed_port(rule.port) {
        errors.push(pol201(
            field_path(idx, "port"),
            "port must be within 1..=65535",
        ));
    }
    validate_target(rule, idx, errors);
}

fn validate_target(rule: &NetworkEgressRule, idx: usize, errors: &mut Vec<ErrorItem>) {
    let has_host = has_non_empty(rule.host.as_deref());
    let has_cidr = has_non_empty(rule.cidr.as_deref());
    if has_host == has_cidr {
        let message = "exactly one of host or cidr must be set";
        errors.push(pol201(field_path(idx, "host"), message));
        errors.push(pol201(field_path(idx, "cidr"), message));
        return;
    }
    if has_cidr && !is_valid_ipv4_cidr(rule.cidr.as_deref().unwrap_or_default()) {
        errors.push(pol201(
            field_path(idx, "cidr"),
            "cidr must be an IPv4 CIDR, for example 1.2.3.4/32",
        ));
    }
}

fn is_allowed_protocol(protocol: Option<&str>) -> bool {
    matches!(normalized(protocol), Some("tcp") | Some("udp"))
}

fn is_allowed_port(port: Option<u32>) -> bool {
    matches!(port, Some(value) if (1..=65535).contains(&value))
}

fn has_non_empty(value: Option<&str>) -> bool {
    normalized(value).is_some()
}

fn normalized(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|raw| !raw.is_empty())
}

fn is_valid_ipv4_cidr(cidr: &str) -> bool {
    let trimmed = cidr.trim();
    let Some((ip_raw, prefix_raw)) = trimmed.split_once('/') else {
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

fn pol201(path: impl Into<String>, message: impl Into<String>) -> ErrorItem {
    ErrorItem::new(SR_POL_201, path, message)
}
