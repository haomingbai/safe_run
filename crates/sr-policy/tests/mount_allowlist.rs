use sr_common::SR_POL_101;
use sr_policy::{load_policy_from_path, validate_policy_with_allowlist};
use std::path::PathBuf;

fn repo_file(path: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../");
    p.push(path);
    p.to_string_lossy().to_string()
}

#[test]
fn allowlist_policy_passes_with_valid_allowlist() {
    let policy = load_policy_from_path(&repo_file(
        "tests/mount_allowlist/policy_allowlist_pass.yaml",
    ))
    .expect("load allowlist pass policy");
    let allowlist_path = repo_file("tests/mount_allowlist/allowlist-valid.yaml");

    let result = validate_policy_with_allowlist(policy, Some(&allowlist_path));

    assert!(result.valid);
    assert!(result.errors.is_empty());
}

#[test]
fn allowlist_policy_rejects_outside_prefix() {
    let policy = load_policy_from_path(&repo_file(
        "tests/mount_allowlist/policy_allowlist_fail.yaml",
    ))
    .expect("load allowlist fail policy");
    let allowlist_path = repo_file("tests/mount_allowlist/allowlist-valid.yaml");

    let result = validate_policy_with_allowlist(policy, Some(&allowlist_path));

    assert!(!result.valid);
    assert!(result.errors.iter().any(|err| err.code == SR_POL_101));
}
