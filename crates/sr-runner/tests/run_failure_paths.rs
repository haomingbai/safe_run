mod common;

use common::{
    compile_bundle_from_policy, new_temp_dir, override_launch_command, parse_event_stream,
    remove_temp_dir, runtime_context, write_mock_cgroup_files, write_mock_vm_artifacts,
};
use serde_json::json;
use sr_common::{SR_RUN_001, SR_RUN_002, SR_RUN_003};
use sr_runner::{RunState, Runner, RunnerControlRequest, RunnerRuntime};
use std::fs;

#[test]
fn launch_failure_returns_sr_run_002() {
    let workdir = new_temp_dir("run-failure-launch");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 10, 512);

    let (_, compile_bundle) = compile_bundle_from_policy();
    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };
    let runner = Runner::with_runtime(RunnerRuntime {
        jailer_bin: "/definitely-not-found/safe-run-jailer".to_string(),
        firecracker_bin: "/bin/true".to_string(),
    });

    let mut prepared = runner.prepare(request).expect("prepare should succeed");
    let err = runner.launch(&mut prepared).expect_err("launch must fail");
    let events = parse_event_stream(&prepared.event_log_path());

    assert_eq!(err.code, SR_RUN_002);
    assert_eq!(prepared.state, RunState::Failed);
    assert!(events.iter().any(|event| event.event_type == "run.failed"));
    remove_temp_dir(&workdir);
}

#[test]
fn prepare_rejects_missing_artifacts() {
    let workdir = new_temp_dir("run-failure-missing-artifacts");
    let runner = Runner::with_runtime(RunnerRuntime {
        jailer_bin: "/bin/true".to_string(),
        firecracker_bin: "/bin/true".to_string(),
    });
    let (_, mut compile_bundle) = compile_bundle_from_policy();
    compile_bundle.firecracker_config = json!({
        "machine-config": {
            "vcpu_count": 1,
            "mem_size_mib": 256,
            "smt": false
        },
        "boot-source": {
            "kernel_image_path": "missing/vmlinux",
            "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
        },
        "rootfs": {
            "path": "missing/rootfs.ext4",
            "readOnly": true
        },
        "drives": []
    });
    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, None, 3, 20),
    };

    let err = runner.prepare(request).expect_err("prepare must fail");
    assert_eq!(err.code, SR_RUN_002);
    assert!(err.path.starts_with("prepare.artifacts."));
    remove_temp_dir(&workdir);
}

#[test]
fn prepare_copies_absolute_artifacts_into_workdir() {
    let workdir = new_temp_dir("run-prepare-copy-artifacts");
    let source_dir = new_temp_dir("run-prepare-source-artifacts");
    let kernel_source = source_dir.join("kernel-src");
    let rootfs_source = source_dir.join("rootfs-src.ext4");
    fs::write(&kernel_source, b"kernel-image").expect("write kernel source");
    fs::write(&rootfs_source, b"rootfs-image").expect("write rootfs source");

    let runner = Runner::with_runtime(RunnerRuntime {
        jailer_bin: "/bin/true".to_string(),
        firecracker_bin: "/bin/true".to_string(),
    });
    let (_, mut compile_bundle) = compile_bundle_from_policy();
    compile_bundle.firecracker_config = json!({
        "machine-config": {
            "vcpu_count": 1,
            "mem_size_mib": 256,
            "smt": false
        },
        "boot-source": {
            "kernel_image_path": kernel_source.to_string_lossy(),
            "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
        },
        "rootfs": {
            "path": rootfs_source.to_string_lossy(),
            "readOnly": true
        },
        "drives": []
    });
    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, None, 3, 20),
    };

    let prepared = runner.prepare(request).expect("prepare should succeed");
    let config_raw = fs::read_to_string(prepared.firecracker_config_path())
        .expect("read firecracker config");
    let config_json: serde_json::Value =
        serde_json::from_str(&config_raw).expect("parse firecracker config");
    let kernel_path = config_json
        .pointer("/boot-source/kernel_image_path")
        .and_then(|value| value.as_str())
        .expect("kernel path exists");
    let rootfs_path = config_json
        .pointer("/rootfs/path")
        .and_then(|value| value.as_str())
        .expect("rootfs path exists");

    assert!(kernel_path.starts_with("artifacts/"));
    assert!(rootfs_path.starts_with("artifacts/"));
    assert!(prepared.workdir().join(kernel_path).is_file());
    assert!(prepared.workdir().join(rootfs_path).is_file());

    remove_temp_dir(&workdir);
    remove_temp_dir(&source_dir);
}

#[test]
fn preflight_failure_on_missing_firecracker_returns_sr_run_002() {
    let workdir = new_temp_dir("run-failure-preflight-firecracker");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 11, 768);

    let (_, compile_bundle) = compile_bundle_from_policy();
    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };
    let runner = Runner::with_runtime(RunnerRuntime {
        jailer_bin: "/bin/true".to_string(),
        firecracker_bin: "/definitely-not-found/safe-run-firecracker".to_string(),
    });

    let mut prepared = runner.prepare(request).expect("prepare should succeed");
    let err = runner.launch(&mut prepared).expect_err("launch must fail");
    let events = parse_event_stream(&prepared.event_log_path());

    assert_eq!(err.code, SR_RUN_002);
    assert_eq!(err.path, "launch.preflight.firecracker");
    assert_eq!(prepared.state, RunState::Failed);
    assert!(events.iter().any(|event| event.event_type == "run.failed"));
    remove_temp_dir(&workdir);
}

#[test]
fn timeout_path_returns_sr_run_003() {
    let workdir = new_temp_dir("run-failure-timeout");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 20, 1024);

    let (_, compile_bundle) = compile_bundle_from_policy();
    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 1, 20),
    };
    let runner = common::runner_with_mock_runtime();

    let mut prepared = runner.prepare(request).expect("prepare should succeed");
    override_launch_command(&mut prepared, "sleep 2");
    runner.launch(&mut prepared).expect("launch should succeed");
    let err = runner
        .monitor(&mut prepared)
        .expect_err("monitor should time out");
    runner
        .cleanup(&mut prepared)
        .expect("cleanup should succeed");
    let events = parse_event_stream(&prepared.event_log_path());

    assert_eq!(err.code, SR_RUN_003);
    assert_eq!(prepared.state, RunState::Failed);
    assert!(events.iter().any(|event| event.event_type == "vm.exited"));
    assert!(events.iter().any(|event| event.event_type == "run.failed"));
    remove_temp_dir(&workdir);
}

#[test]
fn abnormal_exit_is_recorded_with_non_zero_exit_code() {
    let workdir = new_temp_dir("run-failure-abnormal-exit");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 30, 1536);

    let (_, compile_bundle) = compile_bundle_from_policy();
    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };
    let runner = common::runner_with_mock_runtime();

    let mut prepared = runner.prepare(request).expect("prepare should succeed");
    override_launch_command(&mut prepared, "exit 17");
    runner.launch(&mut prepared).expect("launch should succeed");
    let monitor_result = runner
        .monitor(&mut prepared)
        .expect("monitor should complete");
    runner
        .cleanup(&mut prepared)
        .expect("cleanup should succeed");
    let events = parse_event_stream(&prepared.event_log_path());

    assert_eq!(monitor_result.exit_code, 17);
    assert!(!monitor_result.timed_out);
    assert_eq!(prepared.state, RunState::Failed);
    assert!(events.iter().any(|event| event.event_type == "vm.exited"));
    let failed_event = events
        .iter()
        .find(|event| event.event_type == "run.failed")
        .expect("non-zero exit must emit run.failed");
    assert_eq!(
        failed_event
            .payload
            .get("errorCode")
            .and_then(|value| value.as_str()),
        Some(SR_RUN_001)
    );
    remove_temp_dir(&workdir);
}
