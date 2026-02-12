use sr_common::SR_POL_201;
use sr_policy::{parse_policy, validate_policy};

fn parse_and_validate(input: &str) -> sr_policy::ValidationResult {
    let policy = parse_policy(input).expect("policy should parse");
    validate_policy(policy)
}

fn assert_has_error(result: &sr_policy::ValidationResult, path: &str) {
    assert!(
        result
            .errors
            .iter()
            .any(|err| err.code == SR_POL_201 && err.path == path),
        "expected SR-POL-201 at path {path}, got {:?}",
        result.errors
    );
}

#[test]
fn allowlist_without_egress_is_rejected() {
    let result = parse_and_validate(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: allowlist\nmounts: []\naudit:\n  level: basic\n",
    );
    assert!(!result.valid);
    assert_has_error(&result, "network.egress");
}

#[test]
fn none_mode_with_egress_is_rejected() {
    let result = parse_and_validate(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: none\n  egress:\n    - protocol: tcp\n      host: api.example.com\n      port: 443\nmounts: []\naudit:\n  level: basic\n",
    );
    assert!(!result.valid);
    assert_has_error(&result, "network.egress");
}

#[test]
fn allowlist_with_empty_egress_is_rejected() {
    let result = parse_and_validate(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: allowlist\n  egress: []\nmounts: []\naudit:\n  level: basic\n",
    );
    assert!(!result.valid);
    assert_has_error(&result, "network.egress");
}

#[test]
fn allowlist_invalid_protocol_is_rejected() {
    let result = parse_and_validate(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: allowlist\n  egress:\n    - protocol: icmp\n      host: api.example.com\n      port: 443\nmounts: []\naudit:\n  level: basic\n",
    );
    assert!(!result.valid);
    assert_has_error(&result, "network.egress[0].protocol");
}

#[test]
fn allowlist_invalid_port_is_rejected() {
    let result = parse_and_validate(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: allowlist\n  egress:\n    - protocol: tcp\n      host: api.example.com\n      port: 0\nmounts: []\naudit:\n  level: basic\n",
    );
    assert!(!result.valid);
    assert_has_error(&result, "network.egress[0].port");

    let result = parse_and_validate(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: allowlist\n  egress:\n    - protocol: tcp\n      host: api.example.com\n      port: 65536\nmounts: []\naudit:\n  level: basic\n",
    );
    assert!(!result.valid);
    assert_has_error(&result, "network.egress[0].port");
}

#[test]
fn allowlist_requires_exactly_one_host_or_cidr() {
    let both_missing = parse_and_validate(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: allowlist\n  egress:\n    - protocol: tcp\n      port: 443\nmounts: []\naudit:\n  level: basic\n",
    );
    assert!(!both_missing.valid);
    assert!(
        both_missing.errors.iter().any(|err| {
            err.code == SR_POL_201
                && (err.path == "network.egress[0].host" || err.path == "network.egress[0].cidr")
        }),
        "expected host/cidr error, got {:?}",
        both_missing.errors
    );

    let both_present = parse_and_validate(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: allowlist\n  egress:\n    - protocol: udp\n      host: api.example.com\n      cidr: 1.1.1.1/32\n      port: 53\nmounts: []\naudit:\n  level: basic\n",
    );
    assert!(!both_present.valid);
    assert!(
        both_present.errors.iter().any(|err| {
            err.code == SR_POL_201
                && (err.path == "network.egress[0].host" || err.path == "network.egress[0].cidr")
        }),
        "expected host/cidr error, got {:?}",
        both_present.errors
    );
}
