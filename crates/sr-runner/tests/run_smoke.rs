mod common;

use common::{
    build_report_from_events, compile_bundle_from_policy, new_temp_dir, override_launch_command,
    parse_event_stream, remove_temp_dir, runner_with_mock_runtime, runtime_context,
    write_mock_cgroup_files, write_mock_vm_artifacts, write_report,
};
use sr_evidence::{
    EVENT_COMPILE, EVENT_RESOURCE_SAMPLED, EVENT_RUN_CLEANED, EVENT_RUN_PREPARED, EVENT_VM_EXITED,
    EVENT_VM_STARTED,
};
use sr_runner::RunnerControlRequest;

#[test]
fn minimal_run_executes_and_writes_report() {
    let workdir = new_temp_dir("run-smoke");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 1000, 2048);

    let (policy, compile_bundle) = compile_bundle_from_policy();
    let request = RunnerControlRequest {
        compile_bundle: compile_bundle.clone(),
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };

    let runner = runner_with_mock_runtime();
    let mut prepared = runner.prepare(request).expect("prepare should succeed");
    override_launch_command(&mut prepared, "sleep 0.12");
    runner.launch(&mut prepared).expect("launch should succeed");
    let monitor_result = runner
        .monitor(&mut prepared)
        .expect("monitor should succeed");
    runner
        .cleanup(&mut prepared)
        .expect("cleanup should succeed");

    assert_eq!(monitor_result.exit_code, 0);
    assert!(!monitor_result.timed_out);
    assert!(monitor_result.sample_count > 0);

    let events = parse_event_stream(&prepared.event_log_path());
    assert!(events.iter().any(|event| event.event_type == EVENT_COMPILE));
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_RUN_PREPARED));
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_VM_STARTED));
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_RESOURCE_SAMPLED));
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_VM_EXITED));
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_RUN_CLEANED));

    let report = build_report_from_events(
        &workdir,
        &prepared.run_id,
        &monitor_result,
        &events,
        &policy,
        &compile_bundle,
    );
    let report_path = prepared.artifacts_dir().join(&prepared.artifacts.report);
    write_report(&report_path, &report);

    assert_eq!(report.schema_version, "safe-run.report/v1");
    assert_eq!(report.run_id, prepared.run_id);
    assert!(report_path.exists());
    remove_temp_dir(&workdir);
}
