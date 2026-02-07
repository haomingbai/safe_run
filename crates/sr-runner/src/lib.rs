mod cleanup;
mod constants;
mod event;
mod launch;
mod model;
mod monitor;
mod prepare;
mod runner;
mod utils;

pub use model::{
    CommandSpec, LaunchPlan, MonitorResult, PreparedRun, RunArtifacts, RunState,
    RunnerControlRequest, RunnerControlResponse, RunnerRuntime, RuntimeContext,
};
pub use runner::Runner;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sr_common::{SR_RUN_001, SR_RUN_002, SR_RUN_003};
    use sr_compiler::{CompileBundle, EvidencePlan, Plan};
    use sr_evidence::EvidenceEvent;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TMP_ID: AtomicU64 = AtomicU64::new(1);

    fn sample_compile_bundle() -> CompileBundle {
        CompileBundle {
            firecracker_config: json!({
                "machine-config": {
                    "vcpu_count": 1,
                    "mem_size_mib": 256,
                    "smt": false
                },
                "boot-source": {
                    "kernel_image_path": "artifacts/vmlinux",
                    "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
                },
                "rootfs": {
                    "path": "artifacts/rootfs.ext4",
                    "readOnly": true
                }
            }),
            jailer_plan: Plan {
                enabled: true,
                ops: vec!["prepare_jailer_context".to_string()],
            },
            cgroup_plan: Plan {
                enabled: true,
                ops: vec!["set_cpu_max=100000 100000".to_string()],
            },
            network_plan: None,
            evidence_plan: EvidencePlan {
                enabled: true,
                events: vec![
                    "run.prepared".to_string(),
                    "vm.started".to_string(),
                    "run.failed".to_string(),
                ],
            },
        }
    }

    fn runner_for_tests() -> Runner {
        Runner::with_runtime(RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        })
    }

    fn new_temp_run_dir(label: &str) -> PathBuf {
        let id = NEXT_TMP_ID.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!("safe-run-vibe-{label}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn sample_request(workdir: &Path) -> RunnerControlRequest {
        RunnerControlRequest {
            compile_bundle: sample_compile_bundle(),
            runtime_context: RuntimeContext {
                workdir: workdir.to_string_lossy().to_string(),
                timeout_sec: 300,
                sample_interval_ms: None,
                cgroup_path: None,
            },
        }
    }

    fn set_mock_cgroup(cgroup_dir: &Path, cpu_usage_usec: u64, memory_current: u64) {
        fs::create_dir_all(cgroup_dir).expect("create mock cgroup dir");
        fs::write(
            cgroup_dir.join("cpu.stat"),
            format!("usage_usec {cpu_usage_usec}\nuser_usec 10\nsystem_usec 20\n"),
        )
        .expect("write cpu.stat");
        fs::write(
            cgroup_dir.join("memory.current"),
            memory_current.to_string(),
        )
        .expect("write memory.current");
    }

    fn override_launch_to_sleep(prepared: &mut PreparedRun, sleep_seconds: &str) {
        prepared.launch_plan.jailer = CommandSpec {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), format!("sleep {sleep_seconds}")],
        };
    }

    #[test]
    fn runner_control_request_serializes_required_fields() {
        let request = RunnerControlRequest {
            compile_bundle: sample_compile_bundle(),
            runtime_context: RuntimeContext {
                workdir: "/var/lib/safe-run/runs/sr-20260206-001".to_string(),
                timeout_sec: 300,
                sample_interval_ms: Some(200),
                cgroup_path: Some("/sys/fs/cgroup/demo".to_string()),
            },
        };

        let value = serde_json::to_value(request).expect("serialize runner request");
        assert!(value.get("compileBundle").is_some());
        assert_eq!(
            value["runtimeContext"]["workdir"],
            "/var/lib/safe-run/runs/sr-20260206-001"
        );
        assert_eq!(value["runtimeContext"]["timeoutSec"], 300);
        assert_eq!(value["runtimeContext"]["sampleIntervalMs"], 200);
        assert_eq!(value["runtimeContext"]["cgroupPath"], "/sys/fs/cgroup/demo");
    }

    #[test]
    fn runner_control_response_serializes_required_fields() {
        let response = RunnerControlResponse {
            run_id: "sr-20260206-001".to_string(),
            state: RunState::Finished,
            artifacts: RunArtifacts {
                log: "events.jsonl".to_string(),
                report: "run_report.json".to_string(),
            },
            event_stream: vec!["events.jsonl".to_string()],
        };

        let value = serde_json::to_value(response).expect("serialize runner response");
        assert_eq!(value["runId"], "sr-20260206-001");
        assert_eq!(value["state"], "finished");
        assert_eq!(value["artifacts"]["log"], "events.jsonl");
        assert_eq!(value["artifacts"]["report"], "run_report.json");
        assert_eq!(value["eventStream"][0], "events.jsonl");
    }

    #[test]
    fn prepare_creates_workdir_and_runtime_context() {
        let run_dir = new_temp_run_dir("prepare");
        let runner = runner_for_tests();
        let prepared = runner
            .prepare(sample_request(&run_dir))
            .expect("prepare should succeed");

        assert_eq!(prepared.state, RunState::Prepared);
        assert_eq!(prepared.runtime_context.timeout_sec, 300);
        assert!(prepared.workdir().exists());
        assert!(prepared.artifacts_dir().exists());
        assert!(prepared.firecracker_config_path().exists());
        assert!(prepared.runtime_context_path().exists());

        let runtime_context = std::fs::read_to_string(prepared.runtime_context_path())
            .expect("runtime context file exists");
        let runtime_json: serde_json::Value =
            serde_json::from_str(&runtime_context).expect("parse runtime context json");
        assert_eq!(runtime_json["timeoutSec"], 300);
        assert!(runtime_json["sampleIntervalMs"].is_null());
        assert!(runtime_json["cgroupPath"].is_null());

        let _ = fs::remove_dir_all(&run_dir);
    }

    #[test]
    fn launch_assembles_params_and_writes_required_events() {
        let run_dir = new_temp_run_dir("launch-success");
        let runner = runner_for_tests();
        let mut prepared = runner
            .prepare(sample_request(&run_dir))
            .expect("prepare should succeed");

        let response = runner.launch(&mut prepared).expect("launch should succeed");

        assert_eq!(response.state, RunState::Running);
        assert_eq!(response.run_id, prepared.run_id);
        assert_eq!(response.artifacts.log, "events.jsonl");
        assert!(prepared
            .launch_plan
            .jailer
            .args
            .contains(&"--id".to_string()));
        assert!(prepared
            .launch_plan
            .jailer
            .args
            .contains(&prepared.run_id.clone()));
        assert!(prepared
            .launch_plan
            .jailer
            .args
            .contains(&"--exec-file".to_string()));
        assert!(prepared
            .launch_plan
            .jailer
            .args
            .contains(&"/bin/true".to_string()));
        assert!(prepared
            .launch_plan
            .jailer
            .args
            .contains(&"--api-sock".to_string()));
        let api_sock_arg_idx = prepared
            .launch_plan
            .jailer
            .args
            .iter()
            .position(|arg| arg == "--api-sock")
            .expect("jailer args should include --api-sock");
        let expected_api_socket = prepared.api_socket_path().to_string_lossy().to_string();
        assert_eq!(
            prepared
                .launch_plan
                .jailer
                .args
                .get(api_sock_arg_idx + 1)
                .map(|value| value.as_str()),
            Some(expected_api_socket.as_str())
        );

        let events_raw =
            std::fs::read_to_string(prepared.event_log_path()).expect("read event stream");
        let lines: Vec<&str> = events_raw.lines().collect();
        assert_eq!(lines.len(), 2);

        let prepared_event: EvidenceEvent =
            serde_json::from_str(lines[0]).expect("parse run.prepared event");
        let started_event: EvidenceEvent =
            serde_json::from_str(lines[1]).expect("parse vm.started event");
        assert_eq!(prepared_event.event_type, "run.prepared");
        assert_eq!(started_event.event_type, "vm.started");
        assert_eq!(started_event.stage, "launch");

        let _ = fs::remove_dir_all(&run_dir);
    }

    #[test]
    fn launch_failure_returns_run_002_and_invokes_cleanup() {
        let run_dir = new_temp_run_dir("launch-failure");
        let runner = Runner::with_runtime(RunnerRuntime {
            jailer_bin: "/definitely-not-found/safe-run-jailer".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        });
        let mut prepared = runner
            .prepare(sample_request(&run_dir))
            .expect("prepare should succeed");

        let err = runner.launch(&mut prepared).expect_err("launch must fail");
        assert_eq!(err.code, SR_RUN_002);
        assert_eq!(prepared.state, RunState::Failed);
        assert!(prepared.cleanup_marker_path().exists());

        let events_raw =
            std::fs::read_to_string(prepared.event_log_path()).expect("read event stream");
        assert!(events_raw.contains("\"type\":\"run.prepared\""));
        assert!(events_raw.contains("\"type\":\"run.failed\""));

        let _ = fs::remove_dir_all(&run_dir);
    }

    #[test]
    fn launch_preflight_failure_on_missing_firecracker_returns_run_002() {
        let run_dir = new_temp_run_dir("launch-preflight-missing-firecracker");
        let runner = Runner::with_runtime(RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/definitely-not-found/safe-run-firecracker".to_string(),
        });
        let mut prepared = runner
            .prepare(sample_request(&run_dir))
            .expect("prepare should succeed");

        let err = runner.launch(&mut prepared).expect_err("launch must fail");
        assert_eq!(err.code, SR_RUN_002);
        assert_eq!(err.path, "launch.preflight.firecracker");
        assert_eq!(prepared.state, RunState::Failed);
        assert!(prepared.cleanup_marker_path().exists());

        let events_raw =
            std::fs::read_to_string(prepared.event_log_path()).expect("read event stream");
        assert!(events_raw.contains("\"type\":\"run.failed\""));
        assert!(events_raw.contains("launch.preflight"));

        let _ = fs::remove_dir_all(&run_dir);
    }

    #[test]
    fn monitor_collects_samples_and_records_vm_exit() {
        let run_dir = new_temp_run_dir("monitor-success");
        let cgroup_dir = run_dir.join("mock-cgroup");
        set_mock_cgroup(&cgroup_dir, 12345, 4096);

        let runner = runner_for_tests();
        let mut request = sample_request(&run_dir);
        request.runtime_context.timeout_sec = 3;
        request.runtime_context.sample_interval_ms = Some(20);
        request.runtime_context.cgroup_path = Some(cgroup_dir.to_string_lossy().to_string());

        let mut prepared = runner.prepare(request).expect("prepare should succeed");
        override_launch_to_sleep(&mut prepared, "0.15");
        runner.launch(&mut prepared).expect("launch should succeed");

        let result = runner
            .monitor(&mut prepared)
            .expect("monitor should succeed");
        assert_eq!(prepared.state, RunState::Finished);
        assert_eq!(result.exit_code, 0);
        assert!(!result.timed_out);
        assert!(result.sample_count > 0);

        let events_raw =
            std::fs::read_to_string(prepared.event_log_path()).expect("read event stream");
        assert!(events_raw.contains("\"type\":\"resource.sampled\""));
        assert!(events_raw.contains("\"type\":\"vm.exited\""));

        let _ = fs::remove_dir_all(&run_dir);
    }

    #[test]
    fn monitor_timeout_returns_run_003_and_sets_failed_state() {
        let run_dir = new_temp_run_dir("monitor-timeout");
        let cgroup_dir = run_dir.join("mock-cgroup");
        set_mock_cgroup(&cgroup_dir, 200, 8192);

        let runner = runner_for_tests();
        let mut request = sample_request(&run_dir);
        request.runtime_context.timeout_sec = 1;
        request.runtime_context.sample_interval_ms = Some(25);
        request.runtime_context.cgroup_path = Some(cgroup_dir.to_string_lossy().to_string());

        let mut prepared = runner.prepare(request).expect("prepare should succeed");
        override_launch_to_sleep(&mut prepared, "2");
        runner.launch(&mut prepared).expect("launch should succeed");

        let err = runner
            .monitor(&mut prepared)
            .expect_err("monitor should time out");
        assert_eq!(err.code, SR_RUN_003);
        assert_eq!(prepared.state, RunState::Failed);

        let events_raw =
            std::fs::read_to_string(prepared.event_log_path()).expect("read event stream");
        assert!(events_raw.contains("\"type\":\"resource.sampled\""));
        assert!(events_raw.contains("\"type\":\"vm.exited\""));
        assert!(events_raw.contains("\"type\":\"run.failed\""));

        let _ = fs::remove_dir_all(&run_dir);
    }

    #[test]
    fn cleanup_removes_transient_files_and_writes_run_cleaned_event() {
        let run_dir = new_temp_run_dir("cleanup-success");
        let cgroup_dir = run_dir.join("mock-cgroup");
        set_mock_cgroup(&cgroup_dir, 300, 12288);

        let runner = runner_for_tests();
        let mut request = sample_request(&run_dir);
        request.runtime_context.timeout_sec = 3;
        request.runtime_context.sample_interval_ms = Some(20);
        request.runtime_context.cgroup_path = Some(cgroup_dir.to_string_lossy().to_string());

        let mut prepared = runner.prepare(request).expect("prepare should succeed");
        override_launch_to_sleep(&mut prepared, "0.1");
        runner.launch(&mut prepared).expect("launch should succeed");
        runner
            .monitor(&mut prepared)
            .expect("monitor should succeed");
        runner
            .cleanup(&mut prepared)
            .expect("cleanup should succeed");

        assert!(prepared.cleanup_marker_path().exists());
        assert!(!prepared.firecracker_config_path().exists());
        assert!(!prepared.runtime_context_path().exists());
        assert!(!prepared.vm_pid_path().exists());

        let events_raw =
            std::fs::read_to_string(prepared.event_log_path()).expect("read event stream");
        assert!(events_raw.contains("\"type\":\"run.cleaned\""));
        assert_eq!(prepared.state, RunState::Finished);

        let _ = fs::remove_dir_all(&run_dir);
    }

    #[test]
    fn cleanup_failure_writes_run_failed_with_error_code() {
        let run_dir = new_temp_run_dir("cleanup-failure");
        let cgroup_dir = run_dir.join("mock-cgroup");
        set_mock_cgroup(&cgroup_dir, 400, 16384);

        let runner = runner_for_tests();
        let mut request = sample_request(&run_dir);
        request.runtime_context.timeout_sec = 3;
        request.runtime_context.sample_interval_ms = Some(20);
        request.runtime_context.cgroup_path = Some(cgroup_dir.to_string_lossy().to_string());

        let mut prepared = runner.prepare(request).expect("prepare should succeed");
        override_launch_to_sleep(&mut prepared, "0.1");
        runner.launch(&mut prepared).expect("launch should succeed");
        runner
            .monitor(&mut prepared)
            .expect("monitor should succeed");

        let firecracker_path = prepared.firecracker_config_path();
        fs::remove_file(&firecracker_path).expect("remove firecracker config");
        fs::create_dir(&firecracker_path).expect("create conflicting directory");

        let err = runner
            .cleanup(&mut prepared)
            .expect_err("cleanup must fail");
        assert_eq!(prepared.state, RunState::Failed);
        assert_eq!(err.code, SR_RUN_001);

        let events_raw =
            std::fs::read_to_string(prepared.event_log_path()).expect("read event stream");
        assert!(events_raw.contains("\"type\":\"run.failed\""));
        assert!(events_raw.contains(&format!("\"errorCode\":\"{SR_RUN_001}\"")));

        let _ = fs::remove_dir_all(&run_dir);
    }
}
