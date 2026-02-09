use sr_common::SR_POL_101;
use sr_policy::{load_policy_from_path, parse_policy, validate_policy_with_allowlist};
use std::fs;
use std::path::PathBuf;

fn repo_file(path: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../");
    p.push(path);
    p.to_string_lossy().to_string()
}

#[test]
fn dotdot_escape_is_rejected() {
    let policy = load_policy_from_path(&repo_file(
        "tests/mount_escape_cases/policy_escape_dotdot.yaml",
    ))
    .expect("load dotdot escape policy");
    let allowlist_path = repo_file("tests/mount_allowlist/allowlist-valid.yaml");

    let result = validate_policy_with_allowlist(policy, Some(&allowlist_path));

    assert!(!result.valid);
    assert!(result.errors.iter().any(|err| err.code == SR_POL_101));
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let tmp_dir = std::env::temp_dir().join(format!("safe-run-escape-{}", std::process::id()));
    let allowed = tmp_dir.join("allowed");
    let outside = tmp_dir.join("outside");
    let link = allowed.join("link");

    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&allowed).expect("create allowed dir");
    fs::create_dir_all(&outside).expect("create outside dir");
    fs::write(outside.join("data.txt"), b"payload").expect("write outside file");
    symlink(&outside, &link).expect("create symlink");

    let allowlist_path = tmp_dir.join("allowlist.yaml");
    fs::write(
        &allowlist_path,
        format!(
            "schemaVersion: safe-run.mount-allowlist/v1\nhostAllowPrefixes:\n  - {}\nguestAllowPrefixes:\n  - /data\n",
            allowed.to_string_lossy()
        ),
    )
    .expect("write allowlist");

    let policy_yaml = format!(
        "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: symlink-escape\nruntime:\n  command: /bin/echo\n  args: [\"ok\"]\nresources:\n  cpu:\n    max: \"100000 100000\"\n  memory:\n    max: 256Mi\nnetwork:\n  mode: none\nmounts:\n  - source: {}\n    target: /data/input\n    read_only: true\naudit:\n  level: basic\n",
        link.to_string_lossy()
    );
    let policy = parse_policy(&policy_yaml).expect("parse policy");
    let allowlist_path_str = allowlist_path.to_string_lossy().to_string();

    let result = validate_policy_with_allowlist(policy, Some(&allowlist_path_str));

    assert!(!result.valid);
    assert!(result.errors.iter().any(|err| err.code == SR_POL_101));

    let _ = fs::remove_dir_all(&tmp_dir);
}
