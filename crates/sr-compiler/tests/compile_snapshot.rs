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
    let expected =
        std::fs::read_to_string(repo_file("tests/compile_snapshot/expected_bundle.json"))
            .expect("read expected bundle snapshot");

    assert_eq!(actual.trim(), expected.trim());
}

fn assert_json_subset(actual: &serde_json::Value, expected_subset: &serde_json::Value) {
    match (actual, expected_subset) {
        (serde_json::Value::Object(actual_map), serde_json::Value::Object(expected_map)) => {
            for (key, expected_value) in expected_map {
                let actual_value = actual_map
                    .get(key)
                    .unwrap_or_else(|| panic!("missing key: {key}"));
                assert_json_subset(actual_value, expected_value);
            }
        }
        (serde_json::Value::Array(actual_list), serde_json::Value::Array(expected_list)) => {
            assert!(
                actual_list.len() >= expected_list.len(),
                "array length mismatch"
            );
            for (idx, expected_item) in expected_list.iter().enumerate() {
                let actual_item = &actual_list[idx];
                assert_json_subset(actual_item, expected_item);
            }
        }
        _ => {
            assert_eq!(actual, expected_subset);
        }
    }
}

#[test]
fn compile_output_is_additive_over_m0_snapshot() {
    let policy = load_policy_from_path(&repo_file("tests/compile_snapshot/minimal_policy.yaml"))
        .expect("load policy for additive snapshot");
    let validated = validate_policy(policy);
    assert!(validated.valid, "validation failed: {:?}", validated.errors);

    let bundle = compile_dry_run(
        &validated
            .normalized_policy
            .expect("normalized policy on valid validation"),
    )
    .expect("compile should succeed for additive snapshot");

    let actual = serde_json::to_value(&bundle).expect("serialize compile bundle to value");
    let expected_subset = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(repo_file("tests/compile_snapshot/expected_bundle_m0.json"))
            .expect("read m0 expected bundle snapshot"),
    )
    .expect("parse m0 snapshot");

    assert_json_subset(&actual, &expected_subset);
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

#[test]
fn compile_allowlist_output_matches_m3_snapshot() {
    let policy = load_policy_from_path(&repo_file(
        "tests/compile_snapshot/m3_allowlist_policy.yaml",
    ))
    .expect("load allowlist policy for compile");
    let validated = validate_policy(policy);
    assert!(validated.valid, "validation failed: {:?}", validated.errors);

    let bundle = compile_dry_run(
        &validated
            .normalized_policy
            .expect("normalized policy on valid validation"),
    )
    .expect("compile dry run should succeed");

    let actual = serde_json::to_string_pretty(&bundle).expect("serialize compile bundle");
    let expected = std::fs::read_to_string(repo_file(
        "tests/compile_snapshot/expected_bundle_m3_allowlist.json",
    ))
    .expect("read expected m3 allowlist bundle snapshot");

    assert_eq!(actual.trim(), expected.trim());
}

#[test]
fn compile_allowlist_output_is_deterministic() {
    let policy = load_policy_from_path(&repo_file(
        "tests/compile_snapshot/m3_allowlist_policy.yaml",
    ))
    .expect("load allowlist policy for deterministic compile");
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
