mod common;

use common::{
    build_report_from_events, compile_bundle_from_policy, new_temp_dir, override_launch_command,
    parse_event_stream, remove_temp_dir, runner_with_mock_runtime, runtime_context,
    write_mock_cgroup_files, write_mock_vm_artifacts, GENESIS_HASH,
};
use sr_evidence::derive_event_hash;
use sr_runner::RunnerControlRequest;

#[test]
fn run_report_contains_required_m1_schema_fields() {
    let (workdir, report, events) = run_and_build_report("report-schema", "sleep 0.12");

    assert_eq!(report.schema_version, "safe-run.report/v1");
    assert!(!report.run_id.trim().is_empty());
    assert!(!report.started_at.trim().is_empty());
    assert!(!report.finished_at.trim().is_empty());
    assert!(report.artifacts.kernel_hash.starts_with("sha256:"));
    assert!(report.artifacts.rootfs_hash.starts_with("sha256:"));
    assert!(report.artifacts.policy_hash.starts_with("sha256:"));
    assert!(report.artifacts.command_hash.starts_with("sha256:"));
    assert_eq!(report.policy_summary.network, "none");
    assert_eq!(report.events, events);
    assert!(report.integrity.digest.starts_with("sha256:"));
    remove_temp_dir(&workdir);
}

#[test]
fn event_hash_chain_is_recomputable() {
    let (workdir, _, events) = run_and_build_report("event-chain", "sleep 0.08");
    let mut last_hash = GENESIS_HASH.to_string();

    for event in events {
        assert_eq!(event.hash_prev, last_hash);
        assert_eq!(event.hash_self, derive_event_hash(&event));
        last_hash = event.hash_self;
    }

    remove_temp_dir(&workdir);
}

fn run_and_build_report(
    label: &str,
    launch_command: &str,
) -> (
    std::path::PathBuf,
    sr_evidence::RunReport,
    Vec<sr_evidence::EvidenceEvent>,
) {
    let workdir = new_temp_dir(label);
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 42, 4096);

    let (policy, compile_bundle) = compile_bundle_from_policy();
    let request = RunnerControlRequest {
        compile_bundle: compile_bundle.clone(),
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };
    let runner = runner_with_mock_runtime();
    let mut prepared = runner.prepare(request).expect("prepare should succeed");

    override_launch_command(&mut prepared, launch_command);
    runner.launch(&mut prepared).expect("launch should succeed");
    let monitor_result = runner
        .monitor(&mut prepared)
        .expect("monitor should complete");
    runner
        .cleanup(&mut prepared)
        .expect("cleanup should succeed");
    let events = parse_event_stream(&prepared.event_log_path());
    let report = build_report_from_events(
        &workdir,
        &prepared.run_id,
        &monitor_result,
        &events,
        &policy,
        &compile_bundle,
    );
    (workdir, report, events)
}
