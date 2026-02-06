use sr_common::{SR_POL_001, SR_POL_002, SR_POL_003};
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
    let policy = load_policy_from_path(&repo_file("tests/policy_invalid_cases/network_allowlist.yaml"))
        .expect("load invalid policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_003));
}

#[test]
fn missing_required_field_returns_sr_pol_001() {
    let err = load_policy_from_path(&repo_file("tests/policy_invalid_cases/missing_runtime.yaml"))
        .expect_err("missing runtime should fail on parse");
    assert_eq!(err.code, SR_POL_001);
}

#[test]
fn invalid_cpu_format_returns_sr_pol_002() {
    let policy = load_policy_from_path(&repo_file("tests/policy_invalid_cases/invalid_cpu_format.yaml"))
        .expect("load invalid cpu policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_002));
}

#[test]
fn invalid_memory_format_returns_sr_pol_002() {
    let policy =
        load_policy_from_path(&repo_file("tests/policy_invalid_cases/invalid_memory_format.yaml"))
            .expect("load invalid memory policy");
    let result = validate_policy(policy);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_002));
}

#[test]
fn missing_runtime_args_returns_sr_pol_001() {
    let err = load_policy_from_path(&repo_file("tests/policy_invalid_cases/missing_runtime_args.yaml"))
        .expect_err("missing runtime.args should fail on parse");
    assert_eq!(err.code, SR_POL_001);
}

#[test]
fn missing_mounts_returns_sr_pol_001() {
    let err = load_policy_from_path(&repo_file("tests/policy_invalid_cases/missing_mounts.yaml"))
        .expect_err("missing mounts should fail on parse");
    assert_eq!(err.code, SR_POL_001);
}
