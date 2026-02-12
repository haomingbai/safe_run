#![allow(dead_code)]

use serde_json::json;
use sr_compiler::{compile_dry_run, CompileBundle};
use sr_evidence::{
    build_report, compute_artifact_hashes_from_json, compute_integrity_digest, event_time_range,
    mount_audit_from_events, resource_usage_from_events, ArtifactJsonInputs, EvidenceEvent,
    PolicySummary, RunReport,
};
use sr_policy::{
    validate_policy, Audit, Cpu, Memory, Metadata, Network, NetworkMode, PolicySpec, Resources,
    Runtime,
};
use sr_runner::{CommandSpec, MonitorResult, PreparedRun, Runner, RunnerRuntime, RuntimeContext};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const GENESIS_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

pub fn new_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "safe-run-vibe-{label}-{}-{nanos}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp run directory");
    path
}

pub fn remove_temp_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

pub fn write_mock_vm_artifacts(workdir: &Path) {
    let artifacts_dir = workdir.join("artifacts");
    fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");
    fs::write(artifacts_dir.join("vmlinux"), b"kernel-image").expect("write mock kernel");
    fs::write(artifacts_dir.join("rootfs.ext4"), b"rootfs-image").expect("write mock rootfs");
}

pub fn write_mock_cgroup_files(cgroup_dir: &Path, cpu_usage_usec: u64, memory_current: u64) {
    fs::create_dir_all(cgroup_dir).expect("create cgroup dir");
    fs::write(
        cgroup_dir.join("cpu.stat"),
        format!("usage_usec {cpu_usage_usec}\nuser_usec 12\nsystem_usec 8\n"),
    )
    .expect("write cpu.stat");
    fs::write(
        cgroup_dir.join("memory.current"),
        memory_current.to_string(),
    )
    .expect("write memory.current");
}

pub fn compile_bundle_from_policy() -> (PolicySpec, CompileBundle) {
    let validation = validate_policy(sample_policy());
    assert!(
        validation.valid,
        "policy validation failed: {:?}",
        validation.errors
    );
    let policy = validation
        .normalized_policy
        .expect("normalized policy should exist for valid policy");
    let bundle = compile_dry_run(&policy).expect("compile should succeed");
    (policy, bundle)
}

pub fn runtime_context(
    workdir: &Path,
    cgroup_path: Option<&Path>,
    timeout_sec: u64,
    sample_interval_ms: u64,
) -> RuntimeContext {
    RuntimeContext {
        workdir: workdir.to_string_lossy().to_string(),
        timeout_sec,
        sample_interval_ms: Some(sample_interval_ms),
        cgroup_path: cgroup_path.map(|path| path.to_string_lossy().to_string()),
    }
}

pub fn runner_with_mock_runtime() -> Runner {
    Runner::with_runtime(RunnerRuntime {
        jailer_bin: "/bin/true".to_string(),
        firecracker_bin: "/bin/true".to_string(),
    })
}

pub fn override_launch_command(prepared: &mut PreparedRun, shell_command: &str) {
    prepared.launch_plan.jailer = CommandSpec {
        program: "/bin/sh".to_string(),
        args: vec!["-c".to_string(), shell_command.to_string()],
    };
}

pub fn parse_event_stream(path: &Path) -> Vec<EvidenceEvent> {
    let raw = fs::read_to_string(path).expect("read events file");
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<EvidenceEvent>(line).expect("parse event"))
        .collect()
}

pub fn build_report_from_events(
    workdir: &Path,
    run_id: &str,
    monitor_result: &MonitorResult,
    events: &[EvidenceEvent],
    policy: &PolicySpec,
    compile_bundle: &CompileBundle,
) -> RunReport {
    let artifacts = report_artifacts(workdir, policy, compile_bundle);
    let (started_at, finished_at) = event_time_range(events);
    let resource_usage = resource_usage_from_events(events);
    let mount_audit = mount_audit_from_events(events);
    let mut report = build_report(
        run_id.to_string(),
        started_at,
        finished_at,
        monitor_result.exit_code,
        artifacts,
        PolicySummary {
            network: "none".to_string(),
            mounts: policy.mounts.len(),
        },
        resource_usage,
        events.to_vec(),
        mount_audit,
        String::new(),
    );
    report.integrity.digest = compute_integrity_digest(&report).expect("compute report digest");
    report
}

pub fn write_report(path: &Path, report: &RunReport) {
    let content = serde_json::to_string_pretty(report).expect("serialize report");
    fs::write(path, content).expect("write run report");
}

fn sample_policy() -> PolicySpec {
    PolicySpec {
        api_version: "policy.safe-run.dev/v1alpha1".to_string(),
        metadata: Metadata {
            name: "integration-smoke".to_string(),
        },
        runtime: Runtime {
            command: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
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
            egress: vec![],
        },
        mounts: vec![],
        audit: Audit {
            level: "basic".to_string(),
        },
    }
}

fn report_artifacts(
    workdir: &Path,
    policy: &PolicySpec,
    compile_bundle: &CompileBundle,
) -> sr_evidence::ReportArtifacts {
    let policy_json = serde_json::to_value(policy).expect("serialize policy to json");
    let command_json = json!({
        "command": policy.runtime.command,
        "args": policy.runtime.args
    });
    let (kernel_path, rootfs_path) = artifact_paths(workdir, &compile_bundle.firecracker_config);
    compute_artifact_hashes_from_json(ArtifactJsonInputs {
        kernel_path: &kernel_path,
        rootfs_path: &rootfs_path,
        policy_json: &policy_json,
        command_json: &command_json,
    })
    .expect("compute report artifacts")
}

fn artifact_paths(workdir: &Path, firecracker_config: &serde_json::Value) -> (PathBuf, PathBuf) {
    let kernel_raw = json_path(firecracker_config, "/boot-source/kernel_image_path")
        .expect("kernel path must exist");
    let rootfs_raw = json_path(firecracker_config, "/rootfs/path")
        .or_else(|| json_path(firecracker_config, "/drives/0/path"))
        .expect("rootfs path must exist");
    (
        resolve_path(workdir, &kernel_raw),
        resolve_path(workdir, &rootfs_raw),
    )
}

fn json_path(value: &serde_json::Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(|item| item.as_str())
        .map(|raw| raw.to_string())
}

fn resolve_path(workdir: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workdir.join(path)
    }
}
