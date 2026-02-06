use sr_compiler::compile_dry_run;
use sr_policy::{load_policy_from_path, validate_policy};
use std::path::PathBuf;

fn repo_file(path: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../");
    p.push(path);
    p.to_string_lossy().to_string()
}

#[test]
fn compile_output_matches_snapshot() {
    let policy = load_policy_from_path(&repo_file("tests/compile_snapshot/minimal_policy.yaml"))
        .expect("load policy for compile");
    let validated = validate_policy(policy);
    assert!(validated.valid, "validation failed: {:?}", validated.errors);

    let bundle = compile_dry_run(
        &validated
            .normalized_policy
            .expect("normalized policy on valid validation"),
    )
    .expect("compile dry run should succeed");

    let actual = serde_json::to_string_pretty(&bundle).expect("serialize compile bundle");
    let expected = std::fs::read_to_string(repo_file("tests/compile_snapshot/expected_bundle.json"))
        .expect("read expected bundle snapshot");

    assert_eq!(actual.trim(), expected.trim());
}

#[test]
fn compile_output_is_deterministic() {
    let policy = load_policy_from_path(&repo_file("tests/compile_snapshot/minimal_policy.yaml"))
        .expect("load policy for deterministic compile");
    let validated = validate_policy(policy);
    assert!(validated.valid, "validation failed: {:?}", validated.errors);

    let normalized = validated
        .normalized_policy
        .expect("normalized policy on valid validation");
    let bundle_first = compile_dry_run(&normalized).expect("first compile should succeed");
    let bundle_second = compile_dry_run(&normalized).expect("second compile should succeed");

    let first_json = serde_json::to_string_pretty(&bundle_first).expect("serialize first bundle");
    let second_json =
        serde_json::to_string_pretty(&bundle_second).expect("serialize second bundle");
    assert_eq!(first_json, second_json);
}
