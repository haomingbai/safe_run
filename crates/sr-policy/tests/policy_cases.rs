use sr_common::{SR_POL_001, SR_POL_002, SR_POL_003, SR_POL_103};
use sr_policy::{load_policy_from_path, validate_policy};
use std::path::PathBuf;

fn repo_file(path: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../");
    p.push(path);
    p.to_string_lossy().to_string()
}

#[test]
fn valid_case_passes() {
    let policy = load_policy_from_path(&repo_file("tests/policy_valid_cases/minimal.yaml"))
        .expect("load valid policy");
    let result = validate_policy(policy);
    assert!(result.valid);
    assert!(result.errors.is_empty());
}

#[test]
fn invalid_case_rejects_allowlist() {
    let policy = load_policy_from_path(&repo_file(
        "tests/policy_invalid_cases/network_allowlist.yaml",
    ))
    .expect("load invalid policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_003));
}

#[test]
fn missing_required_field_returns_sr_pol_001() {
    let err = load_policy_from_path(&repo_file(
        "tests/policy_invalid_cases/missing_runtime.yaml",
    ))
    .expect_err("missing runtime should fail on parse");
    assert_eq!(err.code, SR_POL_001);
}

#[test]
fn invalid_cpu_format_returns_sr_pol_002() {
    let policy = load_policy_from_path(&repo_file(
        "tests/policy_invalid_cases/invalid_cpu_format.yaml",
    ))
    .expect("load invalid cpu policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_002));
}

#[test]
fn invalid_memory_format_returns_sr_pol_002() {
    let policy = load_policy_from_path(&repo_file(
        "tests/policy_invalid_cases/invalid_memory_format.yaml",
    ))
    .expect("load invalid memory policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_002));
}

#[test]
fn missing_runtime_args_returns_sr_pol_001() {
    let err = load_policy_from_path(&repo_file(
        "tests/policy_invalid_cases/missing_runtime_args.yaml",
    ))
    .expect_err("missing runtime.args should fail on parse");
    assert_eq!(err.code, SR_POL_001);
}

#[test]
fn missing_mounts_returns_sr_pol_001() {
    let err = load_policy_from_path(&repo_file("tests/policy_invalid_cases/missing_mounts.yaml"))
        .expect_err("missing mounts should fail on parse");
    assert_eq!(err.code, SR_POL_001);
}

#[test]
fn mount_alias_fields_are_normalized() {
    let policy = load_policy_from_path(&repo_file("tests/policy_valid_cases/mount_alias.yaml"))
        .expect("load alias policy");
    let result = validate_policy(policy);
    assert!(result.valid);
    let normalized = result.normalized_policy.expect("normalized policy");
    assert_eq!(normalized.mounts.len(), 1);
    let mount = &normalized.mounts[0];
    assert_eq!(mount.source, "/var/lib/safe-run/input");
    assert_eq!(mount.target, "/data/input");
    assert!(mount.read_only);
}

#[test]
fn invalid_mount_source_empty_returns_sr_pol_002() {
    let policy = load_policy_from_path(&repo_file(
        "tests/policy_invalid_cases/invalid_mount_source_empty.yaml",
    ))
    .expect("load invalid mount source policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_002));
}

#[test]
fn invalid_mount_target_not_absolute_returns_sr_pol_002() {
    let policy = load_policy_from_path(&repo_file(
        "tests/policy_invalid_cases/invalid_mount_target_not_absolute.yaml",
    ))
    .expect("load invalid mount target policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_002));
}

#[test]
fn invalid_mount_read_only_false_returns_sr_pol_103() {
    let policy = load_policy_from_path(&repo_file(
        "tests/policy_invalid_cases/invalid_mount_read_only_false.yaml",
    ))
    .expect("load invalid read_only policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    let err = result
        .errors
        .iter()
        .find(|e| e.code == SR_POL_103)
        .expect("expected SR-POL-103");
    assert_eq!(err.path, "mounts[0].read_only");
}
