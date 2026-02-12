mod common;

use common::{
    new_temp_dir, override_launch_command, parse_event_stream, remove_temp_dir, runtime_context,
    write_mock_cgroup_files, write_mock_vm_artifacts,
};
use sr_common::{SR_RUN_001, SR_RUN_201, SR_RUN_202};
use sr_compiler::{compile_dry_run, CompileBundle};
use sr_evidence::{
    EVENT_NETWORK_PLAN_GENERATED, EVENT_NETWORK_RULE_APPLIED, EVENT_NETWORK_RULE_CLEANUP_FAILED,
    EVENT_NETWORK_RULE_RELEASED, EVENT_RUN_FAILED,
};
use sr_policy::{
    validate_policy, Audit, Cpu, Memory, Metadata, Network, NetworkEgressRule, NetworkMode,
    PolicySpec, Resources, Runtime,
};
use sr_runner::{
    AppliedNetwork, AppliedNetworkRule, NetworkLifecycle, NetworkLifecycleError, Runner,
    RunnerControlRequest, RunnerRuntime,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct RecordingNetworkLifecycle {
    calls: Arc<Mutex<Vec<String>>>,
    fail_apply: bool,
    fail_release: bool,
}

impl NetworkLifecycle for RecordingNetworkLifecycle {
    fn apply(
        &self,
        run_id: &str,
        plan: &sr_compiler::NetworkPlan,
    ) -> Result<AppliedNetwork, NetworkLifecycleError> {
        self.calls
            .lock()
            .expect("lock calls")
            .push(format!("apply:{run_id}"));
        if self.fail_apply {
            return Err(NetworkLifecycleError::new(
                "launch.network.apply",
                "mock apply failure",
            ));
        }

        let tap_name = plan.tap.name.replace("<runId>", run_id);
        let mut rules = Vec::new();
        for chain in &plan.nft.chains {
            for rule in &plan.nft.rules {
                let target = rule
                    .cidr
                    .as_ref()
                    .or(rule.host.as_ref())
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
                rules.push(AppliedNetworkRule {
                    chain: chain.clone(),
                    protocol: rule.protocol.clone(),
                    target,
                    port: rule.port,
                });
            }
        }

        Ok(AppliedNetwork {
            tap_name,
            table: plan.nft.table.clone(),
            chains: plan.nft.chains.clone(),
            rules,
        })
    }

    fn release(&self, applied: &AppliedNetwork) -> Result<(), NetworkLifecycleError> {
        self.calls
            .lock()
            .expect("lock calls")
            .push(format!("release:{}", applied.tap_name));
        if self.fail_release {
            return Err(NetworkLifecycleError::new(
                "cleanup.network.release",
                "mock release failure",
            ));
        }
        Ok(())
    }
}

#[test]
fn network_apply_failure_returns_sr_run_201() {
    let workdir = new_temp_dir("network-apply-failure");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 200, 1024);

    let compile_bundle = compile_allowlist_bundle(false);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let runner = Runner::with_network_lifecycle(
        RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        },
        RecordingNetworkLifecycle {
            calls: calls.clone(),
            fail_apply: true,
            fail_release: false,
        },
    );

    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };
    let mut prepared = runner.prepare(request).expect("prepare should succeed");
    let err = runner.launch(&mut prepared).expect_err("launch must fail");
    assert_eq!(err.code, SR_RUN_201);
    assert_eq!(err.path, "launch.network.apply");
    assert!(prepared.cleanup_marker_path().exists());

    let events = parse_event_stream(&prepared.event_log_path());
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_RUN_FAILED));
    assert_eq!(
        *calls.lock().expect("lock calls"),
        vec![format!("apply:{}", prepared.run_id)]
    );

    remove_temp_dir(&workdir);
}

#[test]
fn network_cleanup_failure_returns_sr_run_202() {
    let workdir = new_temp_dir("network-cleanup-failure");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 210, 2048);

    let compile_bundle = compile_allowlist_bundle(true);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let runner = Runner::with_network_lifecycle(
        RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        },
        RecordingNetworkLifecycle {
            calls: calls.clone(),
            fail_apply: false,
            fail_release: true,
        },
    );

    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };
    let mut prepared = runner.prepare(request).expect("prepare should succeed");
    override_launch_command(&mut prepared, "sleep 0.05");
    runner.launch(&mut prepared).expect("launch should succeed");
    let _ = runner
        .monitor(&mut prepared)
        .expect("monitor should succeed");

    let err = runner
        .cleanup(&mut prepared)
        .expect_err("cleanup must fail");
    assert_eq!(err.code, SR_RUN_202);
    assert_eq!(err.path, "cleanup.network.release");

    let events = parse_event_stream(&prepared.event_log_path());
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_NETWORK_RULE_CLEANUP_FAILED));
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_RUN_FAILED));

    let calls = calls.lock().expect("lock calls").clone();
    assert!(calls.iter().any(|call| call.starts_with("apply:")));
    assert!(calls.iter().any(|call| call.starts_with("release:")));

    remove_temp_dir(&workdir);
}

#[cfg(unix)]
#[test]
fn launch_failure_after_network_apply_triggers_release() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let workdir = new_temp_dir("network-launch-failure-release");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 220, 4096);

    let compile_bundle = compile_allowlist_bundle(true);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let runner = Runner::with_network_lifecycle(
        RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        },
        RecordingNetworkLifecycle {
            calls: calls.clone(),
            fail_apply: false,
            fail_release: false,
        },
    );

    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };
    let mut prepared = runner.prepare(request).expect("prepare should succeed");

    let artifacts_dir = prepared.artifacts_dir().to_path_buf();
    let metadata = fs::metadata(&artifacts_dir).expect("inspect artifacts dir");
    let original_mode = metadata.permissions().mode();
    let mut readonly = metadata.permissions();
    readonly.set_mode(0o500);
    fs::set_permissions(&artifacts_dir, readonly).expect("set readonly artifacts dir");

    let err = runner.launch(&mut prepared).expect_err("launch must fail");
    assert_eq!(err.code, SR_RUN_001);
    assert_eq!(err.path, "launch.vmPid");

    let mut restore = fs::metadata(&artifacts_dir)
        .expect("inspect readonly artifacts dir")
        .permissions();
    restore.set_mode(original_mode);
    fs::set_permissions(&artifacts_dir, restore).expect("restore artifacts permissions");

    let events = parse_event_stream(&prepared.event_log_path());
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_NETWORK_RULE_RELEASED));
    assert!(events
        .iter()
        .any(|event| event.event_type == EVENT_RUN_FAILED));

    let calls = calls.lock().expect("lock calls").clone();
    assert!(calls.iter().any(|call| call.starts_with("apply:")));
    assert!(calls.iter().any(|call| call.starts_with("release:")));

    remove_temp_dir(&workdir);
}

#[test]
fn network_events_follow_evidence_gating() {
    let workdir = new_temp_dir("network-event-gating");
    let cgroup_dir = workdir.join("mock-cgroup");
    write_mock_vm_artifacts(&workdir);
    write_mock_cgroup_files(&cgroup_dir, 230, 3072);

    let compile_bundle = compile_allowlist_bundle(false);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let runner = Runner::with_network_lifecycle(
        RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        },
        RecordingNetworkLifecycle {
            calls: calls.clone(),
            fail_apply: false,
            fail_release: false,
        },
    );

    let request = RunnerControlRequest {
        compile_bundle,
        runtime_context: runtime_context(&workdir, Some(&cgroup_dir), 3, 20),
    };
    let mut prepared = runner.prepare(request).expect("prepare should succeed");
    override_launch_command(&mut prepared, "sleep 0.05");
    runner.launch(&mut prepared).expect("launch should succeed");
    let _ = runner
        .monitor(&mut prepared)
        .expect("monitor should succeed");
    runner
        .cleanup(&mut prepared)
        .expect("cleanup should succeed");

    let events = parse_event_stream(&prepared.event_log_path());
    assert!(
        !events
            .iter()
            .any(|event| event.event_type.starts_with("network.")),
        "network events must be gated by evidencePlan.events"
    );
    let calls = calls.lock().expect("lock calls").clone();
    assert!(calls.iter().any(|call| call.starts_with("apply:")));
    assert!(calls.iter().any(|call| call.starts_with("release:")));

    remove_temp_dir(&workdir);
}

fn compile_allowlist_bundle(include_network_events: bool) -> CompileBundle {
    let policy = PolicySpec {
        api_version: "policy.safe-run.dev/v1alpha1".to_string(),
        metadata: Metadata {
            name: "network-stage3".to_string(),
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
            mode: NetworkMode::Allowlist,
            egress: vec![NetworkEgressRule {
                protocol: Some("tcp".to_string()),
                host: None,
                cidr: Some("1.1.1.1/32".to_string()),
                port: Some(443),
            }],
        },
        mounts: vec![],
        audit: Audit {
            level: "basic".to_string(),
        },
    };
    let validation = validate_policy(policy);
    assert!(
        validation.valid,
        "validation failed: {:?}",
        validation.errors
    );
    let mut bundle = compile_dry_run(
        &validation
            .normalized_policy
            .expect("normalized policy should exist"),
    )
    .expect("compile should succeed");
    if include_network_events {
        ensure_network_event(&mut bundle, EVENT_NETWORK_PLAN_GENERATED);
        ensure_network_event(&mut bundle, EVENT_NETWORK_RULE_APPLIED);
        ensure_network_event(&mut bundle, EVENT_NETWORK_RULE_RELEASED);
        ensure_network_event(&mut bundle, EVENT_NETWORK_RULE_CLEANUP_FAILED);
    }
    bundle
}

fn ensure_network_event(bundle: &mut CompileBundle, event_type: &str) {
    if bundle
        .evidence_plan
        .events
        .iter()
        .any(|event| event == event_type)
    {
        return;
    }
    bundle.evidence_plan.events.push(event_type.to_string());
}
