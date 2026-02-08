use sr_common::SR_POL_101;
use sr_policy::{validate_policy_with_allowlist, Audit, Cpu, Memory, Metadata, Mount, Network, NetworkMode, PolicySpec, Resources, Runtime};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut dir = std::env::temp_dir();
    dir.push(format!("safe-run-{label}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_allowlist(path: &Path, host_prefixes: &[&Path]) -> PathBuf {
    let allowlist_path = path.join("allowlist.yaml");
    let host_list = host_prefixes
        .iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    let content = format!(
        "schemaVersion: safe-run.mount-allowlist/v1\nhostAllowPrefixes:\n{host_list}\nguestAllowPrefixes:\n  - /data\n"
    );
    fs::write(&allowlist_path, content).expect("write allowlist");
    allowlist_path
}

fn base_policy_with_mount(source: &str) -> PolicySpec {
    PolicySpec {
        api_version: "policy.safe-run.dev/v1alpha1".to_string(),
        metadata: Metadata {
            name: "demo".to_string(),
        },
        runtime: Runtime {
            command: "/bin/echo".to_string(),
            args: vec!["ok".to_string()],
        },
        resources: Resources {
            cpu: Cpu {
                max: "100000 100000".to_string(),
            },
            memory: Memory {
                max: "256Mi".to_string(),
            },
        },
        network: Network {
            mode: NetworkMode::None,
        },
        mounts: vec![Mount {
            source: source.to_string(),
            target: "/data/input".to_string(),
            read_only: true,
        }],
        audit: Audit {
            level: "basic".to_string(),
        },
    }
}

#[test]
fn allowlisted_path_passes() {
    let base = unique_temp_dir("allowlist-pass");
    let allowed = base.join("allowed");
    fs::create_dir_all(&allowed).expect("create allowed dir");
    let allowlist_path = write_allowlist(&base, &[allowed.as_path()]);

    let policy = base_policy_with_mount(allowed.to_string_lossy().as_ref());
    let result = validate_policy_with_allowlist(policy, Some(allowlist_path.to_string_lossy().as_ref()));
    assert!(result.valid);
    assert!(result.errors.is_empty());
}

#[test]
fn symlink_escape_is_rejected() {
    let base = unique_temp_dir("allowlist-symlink");
    let allowed = base.join("allowed");
    let outside = base.join("outside");
    fs::create_dir_all(&allowed).expect("create allowed dir");
    fs::create_dir_all(&outside).expect("create outside dir");

    let link = allowed.join("link");
    std::os::unix::fs::symlink(&outside, &link).expect("create symlink");

    let allowlist_path = write_allowlist(&base, &[allowed.as_path()]);

    let policy = base_policy_with_mount(link.to_string_lossy().as_ref());
    let result = validate_policy_with_allowlist(policy, Some(allowlist_path.to_string_lossy().as_ref()));
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_101));
}

#[test]
fn path_traversal_outside_allowlist_is_rejected() {
    let base = unique_temp_dir("allowlist-traversal");
    let allowed = base.join("allowed");
    let outside = base.join("outside");
    fs::create_dir_all(&allowed).expect("create allowed dir");
    fs::create_dir_all(&outside).expect("create outside dir");

    let allowlist_path = write_allowlist(&base, &[allowed.as_path()]);

    let traversal = allowed.join("..").join("outside");
    let policy = base_policy_with_mount(traversal.to_string_lossy().as_ref());
    let result = validate_policy_with_allowlist(policy, Some(allowlist_path.to_string_lossy().as_ref()));
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == SR_POL_101));
}
