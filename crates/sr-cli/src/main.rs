use clap::{Parser, Subcommand};
use sr_common::{ErrorItem, SR_CMP_002, SR_EVD_002, SR_RUN_001};
use sr_compiler::{compile_dry_run, CompileBundle};
use sr_evidence::{
    build_report, compute_artifact_hashes_from_json, compute_integrity_digest, event_time_range,
    mount_audit_from_events, network_audit_from_events, resource_usage_from_events,
    ArtifactJsonInputs, EvidenceEvent, PolicySummary, RunReport,
};
use sr_policy::{load_policy_from_path, validate_policy_with_allowlist, NetworkMode, PolicySpec};
use sr_runner::{MonitorResult, RunState, Runner, RunnerControlRequest, RuntimeContext};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Parser)]
#[command(name = "safe-run")]
#[command(about = "Safe-Run CLI (M0-M2)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Validate {
        policy: String,
        #[arg(long = "mount-allowlist")]
        mount_allowlist: Option<String>,
    },
    Compile {
        #[arg(long = "dry-run", default_value_t = false)]
        dry_run: bool,
        #[arg(long)]
        policy: String,
        #[arg(long = "mount-allowlist")]
        mount_allowlist: Option<String>,
    },
    Run {
        #[arg(long)]
        policy: String,
        #[arg(long = "mount-allowlist")]
        mount_allowlist: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate {
            policy,
            mount_allowlist,
        } => validate_cmd(&policy, mount_allowlist.as_deref()),
        Commands::Compile {
            dry_run,
            policy,
            mount_allowlist,
        } => compile_cmd(dry_run, &policy, mount_allowlist.as_deref()),
        Commands::Run {
            policy,
            mount_allowlist,
        } => run_cmd(&policy, mount_allowlist.as_deref()),
    }
}

fn validate_cmd(policy_path: &str, allowlist_path: Option<&str>) -> ExitCode {
    match load_policy_from_path(policy_path) {
        Ok(policy) => {
            let result = validate_policy_with_allowlist(policy, allowlist_path);
            print_json_value(&serde_json::to_value(&result).expect("convert validation result"));
            if result.valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(err) => {
            print_error_result(&err);
            ExitCode::from(2)
        }
    }
}

fn compile_cmd(dry_run: bool, policy_path: &str, allowlist_path: Option<&str>) -> ExitCode {
    if !dry_run {
        let err = ErrorItem::new(
            SR_CMP_002,
            "compile.dryRun",
            "M0-M2 only supports compile --dry-run",
        );
        print_error_result(&err);
        return ExitCode::from(2);
    }

    let policy = match load_policy_from_path(policy_path) {
        Ok(policy) => policy,
        Err(err) => {
            print_error_result(&err);
            return ExitCode::from(2);
        }
    };

    let validation = validate_policy_with_allowlist(policy, allowlist_path);
    if !validation.valid {
        print_json_value(&serde_json::to_value(&validation).expect("convert validation result"));
        return ExitCode::from(2);
    }

    let normalized = validation
        .normalized_policy
        .expect("normalized policy exists on valid result");

    match compile_dry_run(&normalized) {
        Ok(bundle) => {
            print_json_value(&serde_json::to_value(&bundle).expect("convert compile bundle"));
            ExitCode::SUCCESS
        }
        Err(err) => {
            print_error_result(&err);
            ExitCode::from(2)
        }
    }
}

fn run_cmd(policy_path: &str, allowlist_path: Option<&str>) -> ExitCode {
    let normalized = match load_and_validate_policy(policy_path, allowlist_path) {
        Ok(policy) => policy,
        Err(code) => return code,
    };
    let compile_bundle = match compile_dry_run(&normalized) {
        Ok(bundle) => bundle,
        Err(err) => return exit_with_error(&err),
    };
    let run_id = derive_run_id();
    let (prepared, monitor_result) = match execute_run(&compile_bundle, &run_id) {
        Ok(result) => result,
        Err(err) => return exit_with_error(&err),
    };
    let report_path = prepared.artifacts_dir().join(&prepared.artifacts.report);
    match build_and_write_report(&prepared, &normalized, &monitor_result, &report_path) {
        Ok(report) => {
            if let Some(err) = run_outcome_error(prepared.state, &monitor_result, &report_path) {
                return exit_with_error(&err);
            }
            print_json_value(&serde_json::json!({
                "runId": report.run_id,
                "state": state_label(prepared.state),
                "report": report_path
            }));
            ExitCode::SUCCESS
        }
        Err(err) => exit_with_error(&err),
    }
}

fn run_outcome_error(
    state: RunState,
    monitor_result: &MonitorResult,
    report_path: &Path,
) -> Option<ErrorItem> {
    if state == RunState::Failed || monitor_result.exit_code != 0 {
        return Some(ErrorItem::new(
            SR_RUN_001,
            "run.exitCode",
            format!(
                "run exited abnormally with code {} (report: {})",
                monitor_result.exit_code,
                report_path.display()
            ),
        ));
    }
    None
}

fn load_and_validate_policy(
    policy_path: &str,
    allowlist_path: Option<&str>,
) -> Result<PolicySpec, ExitCode> {
    let policy = match load_policy_from_path(policy_path) {
        Ok(policy) => policy,
        Err(err) => {
            print_error_result(&err);
            return Err(ExitCode::from(2));
        }
    };
    let validation = validate_policy_with_allowlist(policy, allowlist_path);
    if !validation.valid {
        print_json_value(&serde_json::to_value(&validation).expect("convert validation result"));
        return Err(ExitCode::from(2));
    }
    Ok(validation
        .normalized_policy
        .expect("normalized policy exists on valid result"))
}

fn execute_run(
    compile_bundle: &CompileBundle,
    run_id: &str,
) -> Result<(sr_runner::PreparedRun, MonitorResult), ErrorItem> {
    let runtime_context = default_runtime_context(run_id);
    let request = RunnerControlRequest {
        compile_bundle: compile_bundle.clone(),
        runtime_context,
    };
    let runner = Runner::new();
    let mut prepared = runner.prepare(request)?;
    if let Err(err) = runner.launch(&mut prepared) {
        let _ = runner.cleanup(&mut prepared);
        return Err(err);
    }
    let monitor_result = match runner.monitor(&mut prepared) {
        Ok(result) => result,
        Err(err) => {
            let _ = runner.cleanup(&mut prepared);
            return Err(err);
        }
    };
    if let Err(err) = runner.cleanup(&mut prepared) {
        return Err(err);
    }
    Ok((prepared, monitor_result))
}

fn exit_with_error(err: &ErrorItem) -> ExitCode {
    print_error_result(err);
    ExitCode::from(2)
}

fn default_runtime_context(run_id: &str) -> RuntimeContext {
    RuntimeContext {
        workdir: default_workdir_for_run(run_id)
            .to_string_lossy()
            .to_string(),
        timeout_sec: 300,
        sample_interval_ms: None,
        cgroup_path: detect_default_cgroup_path(),
    }
}

fn default_workdir_for_run(run_id: &str) -> PathBuf {
    let base = std::env::var("SAFE_RUN_WORKDIR_BASE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/tmp/safe-run/runs".to_string());
    PathBuf::from(base).join(run_id)
}

fn derive_run_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("sr-{}-{:09}", now.as_secs(), now.subsec_nanos())
}

fn build_and_write_report(
    prepared: &sr_runner::PreparedRun,
    policy: &PolicySpec,
    monitor_result: &MonitorResult,
    report_path: &Path,
) -> Result<RunReport, ErrorItem> {
    let events = load_events(prepared.event_log_path().as_path())?;
    let mut report = build_report_from_events(prepared, policy, monitor_result, &events)?;
    let digest = compute_integrity_digest(&report)?;
    report.integrity.digest = digest;
    write_report(report_path, &report)?;
    Ok(report)
}

fn build_report_from_events(
    prepared: &sr_runner::PreparedRun,
    policy: &PolicySpec,
    monitor_result: &MonitorResult,
    events: &[EvidenceEvent],
) -> Result<RunReport, ErrorItem> {
    let (started_at, finished_at) = event_time_range(events);
    let resource_usage = resource_usage_from_events(events);
    let network_mode = network_label(&policy.network.mode).to_string();
    let policy_summary = PolicySummary {
        network: network_mode.clone(),
        mounts: policy.mounts.len(),
    };
    let mount_audit = mount_audit_from_events(events);
    let network_audit =
        network_audit_from_events(events, &network_mode, policy.network.egress.len());
    let artifacts = compute_report_artifacts(prepared, policy)?;
    Ok(build_report(
        prepared.run_id.clone(),
        started_at,
        finished_at,
        monitor_result.exit_code,
        artifacts,
        policy_summary,
        resource_usage,
        events.to_vec(),
        mount_audit,
        network_audit,
        String::new(),
    ))
}

fn compute_report_artifacts(
    prepared: &sr_runner::PreparedRun,
    policy: &PolicySpec,
) -> Result<sr_evidence::ReportArtifacts, ErrorItem> {
    let policy_json = serde_json::to_value(policy).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "report.policy",
            format!("failed to serialize policy for hashing: {err}"),
        )
    })?;
    let command_json = serde_json::json!({
        "command": policy.runtime.command,
        "args": policy.runtime.args
    });
    let firecracker_config = load_firecracker_config(prepared.firecracker_config_path())?;
    let (kernel_path, rootfs_path) =
        resolve_artifact_paths(prepared.workdir(), &firecracker_config)?;
    compute_artifact_hashes_from_json(ArtifactJsonInputs {
        kernel_path: kernel_path.as_path(),
        rootfs_path: rootfs_path.as_path(),
        policy_json: &policy_json,
        command_json: &command_json,
    })
}

fn load_firecracker_config(path: PathBuf) -> Result<serde_json::Value, ErrorItem> {
    let raw = fs::read_to_string(&path).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "report.firecrackerConfig",
            format!(
                "failed to read firecracker config '{}': {err}",
                path.display()
            ),
        )
    })?;
    serde_json::from_str(&raw).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "report.firecrackerConfig",
            format!(
                "failed to parse firecracker config '{}': {err}",
                path.display()
            ),
        )
    })
}

fn load_events(path: &Path) -> Result<Vec<EvidenceEvent>, ErrorItem> {
    let raw = fs::read_to_string(path).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "report.events",
            format!("failed to read event stream: {err}"),
        )
    })?;
    let mut events = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let event: EvidenceEvent = serde_json::from_str(trimmed).map_err(|err| {
            ErrorItem::new(
                SR_EVD_002,
                "report.events",
                format!("failed to parse event at line {}: {err}", idx + 1),
            )
        })?;
        events.push(event);
    }
    Ok(events)
}

fn resolve_artifact_paths(
    workdir: &Path,
    firecracker_config: &serde_json::Value,
) -> Result<(PathBuf, PathBuf), ErrorItem> {
    let kernel_raw = json_string_at(
        firecracker_config,
        "/boot-source/kernel_image_path",
        "artifacts.kernel",
    )?;
    let rootfs_raw = json_string_at(firecracker_config, "/rootfs/path", "artifacts.rootfs")
        .or_else(|_| json_string_at(firecracker_config, "/drives/0/path", "artifacts.rootfs"))?;
    let kernel_path = resolve_path(workdir, &kernel_raw);
    let rootfs_path = resolve_path(workdir, &rootfs_raw);
    Ok((kernel_path, rootfs_path))
}

#[cfg(test)]
mod allowlist_tests {
    use super::*;
    use sr_common::SR_POL_101;
    use sr_policy::ValidationResult;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn repo_file(path: &str) -> String {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../../");
        p.push(path);
        p.to_string_lossy().to_string()
    }

    fn validate_from_path(policy_path: &str, allowlist_path: Option<&str>) -> ValidationResult {
        let policy = load_policy_from_path(policy_path).expect("load policy");
        validate_policy_with_allowlist(policy, allowlist_path)
    }

    #[test]
    fn mount_allowlist_cli_overrides_env() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let policy_path = repo_file("tests/policy_valid_cases/minimal.yaml");
        let env_allowlist = repo_file("tests/mount_allowlist/allowlist-invalid.yaml");
        let cli_allowlist = repo_file("tests/mount_allowlist/allowlist-valid.yaml");

        std::env::set_var("SAFE_RUN_MOUNT_ALLOWLIST", &env_allowlist);
        let result = validate_from_path(&policy_path, Some(&cli_allowlist));
        std::env::remove_var("SAFE_RUN_MOUNT_ALLOWLIST");

        assert!(result.valid, "expected CLI allowlist to override env");
        assert!(result.errors.is_empty());
    }

    #[test]
    fn mount_allowlist_env_is_used_when_cli_missing() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let policy_path = repo_file("tests/policy_valid_cases/minimal.yaml");
        let env_allowlist = repo_file("tests/mount_allowlist/allowlist-invalid.yaml");

        std::env::set_var("SAFE_RUN_MOUNT_ALLOWLIST", &env_allowlist);
        let result = validate_from_path(&policy_path, None);
        std::env::remove_var("SAFE_RUN_MOUNT_ALLOWLIST");

        assert!(!result.valid);
        assert!(result.errors.iter().any(|err| err.code == SR_POL_101));
    }
}

fn json_string_at(
    value: &serde_json::Value,
    pointer: &str,
    error_path: &str,
) -> Result<String, ErrorItem> {
    value
        .pointer(pointer)
        .and_then(|value| value.as_str())
        .map(|raw| raw.to_string())
        .ok_or_else(|| {
            ErrorItem::new(
                SR_EVD_002,
                error_path,
                format!("missing required field {pointer}"),
            )
        })
}

fn resolve_path(workdir: &Path, raw: &str) -> PathBuf {
    let raw_path = Path::new(raw);
    if raw_path.is_absolute() {
        return raw_path.to_path_buf();
    }
    let workdir_path = workdir.join(raw_path);
    if workdir_path.exists() {
        workdir_path
    } else {
        raw_path.to_path_buf()
    }
}

fn detect_default_cgroup_path() -> Option<String> {
    let root = Path::new("/sys/fs/cgroup");
    if has_cgroup_files(root) {
        return Some(root.to_string_lossy().to_string());
    }
    let raw = fs::read_to_string("/proc/self/cgroup").ok()?;
    for line in raw.lines() {
        let mut parts = line.splitn(3, ':');
        let _ = parts.next();
        let _ = parts.next();
        let relative = parts.next().unwrap_or_default().trim();
        if relative.is_empty() || !relative.starts_with('/') {
            continue;
        }
        let candidate = root.join(relative.trim_start_matches('/'));
        if has_cgroup_files(candidate.as_path()) {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn has_cgroup_files(path: &Path) -> bool {
    path.join("cpu.stat").is_file() && path.join("memory.current").is_file()
}

fn network_label(mode: &NetworkMode) -> &'static str {
    match mode {
        NetworkMode::None => "none",
        NetworkMode::Allowlist => "allowlist",
    }
}

fn write_report(path: &Path, report: &RunReport) -> Result<(), ErrorItem> {
    let content = serde_json::to_string_pretty(report).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "report.serialize",
            format!("failed to serialize run report: {err}"),
        )
    })?;
    fs::write(path, content).map_err(|err| {
        ErrorItem::new(
            SR_EVD_002,
            "report.write",
            format!("failed to write run report: {err}"),
        )
    })
}

fn state_label(state: RunState) -> &'static str {
    match state {
        RunState::Prepared => "prepared",
        RunState::Running => "running",
        RunState::Finished => "finished",
        RunState::Failed => "failed",
    }
}

fn print_json_value(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serialize json output")
    );
}

fn print_error_result(err: &ErrorItem) {
    print_json_value(&serde_json::json!({
        "valid": false,
        "errors": [err],
        "warnings": [],
        "normalizedPolicy": null
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_compiler::compile_dry_run;
    use sr_evidence::{EVENT_NETWORK_RULE_HIT, STAGE_CLEANUP};
    use sr_policy::{
        Audit, Cpu, Memory, Metadata, Network, NetworkEgressRule, NetworkMode, Resources, Runtime,
    };
    use sr_runner::RunnerRuntime;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn compile_requires_dry_run_flag() {
        let code = compile_cmd(false, "unused.yaml", None);
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn run_rejects_missing_policy_file() {
        let code = run_cmd("/tmp/safe-run-cli-missing.yaml", None);
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn run_rejects_invalid_policy() {
        let path = temp_policy_path("invalid-run-policy");
        let mut file = File::create(&path).expect("create policy file");
        writeln!(
            file,
            "apiVersion: policy.safe-run.dev/v1alpha1\nmetadata:\n  name: demo\nruntime:\n  command: /bin/echo\n  args: [ok]\nresources:\n  cpu:\n    max: '100000 100000'\n  memory:\n    max: 256Mi\nnetwork:\n  mode: allowlist\nmounts: []\naudit:\n  level: basic\n"
        )
        .expect("write policy");

        let code = run_cmd(path.to_string_lossy().as_ref(), None);
        assert_eq!(code, ExitCode::from(2));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn report_build_succeeds_after_cleanup() {
        let run_id = "sr-test-report-after-cleanup";
        let workdir = temp_run_dir(run_id);
        write_mock_vm_artifacts(&workdir);

        let policy = sample_policy();
        let compile_bundle = compile_dry_run(&policy).expect("compile should succeed");
        let request = RunnerControlRequest {
            compile_bundle,
            runtime_context: RuntimeContext {
                workdir: workdir.to_string_lossy().to_string(),
                timeout_sec: 1,
                sample_interval_ms: None,
                cgroup_path: None,
            },
        };
        let runner = Runner::with_runtime(RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        });

        let mut prepared = runner.prepare(request).expect("prepare should succeed");
        runner
            .cleanup(&mut prepared)
            .expect("cleanup should succeed");

        let report_path = prepared.artifacts_dir().join(&prepared.artifacts.report);
        let monitor_result = MonitorResult {
            exit_code: 0,
            timed_out: false,
            sample_count: 0,
        };
        let result = build_and_write_report(&prepared, &policy, &monitor_result, &report_path);
        assert!(prepared.firecracker_config_path().exists());
        assert!(prepared.event_log_path().exists());
        let report = result.unwrap_or_else(|err| {
            panic!("report build should succeed after cleanup, got: {err:?}");
        });
        let recomputed = compute_integrity_digest(&report).expect("recompute integrity digest");
        assert_eq!(report.integrity.digest, recomputed);
        assert_eq!(report.network_audit.mode, "none");
        assert_eq!(report.network_audit.rules_total, 0);
        assert_eq!(report.network_audit.allowed_hits, 0);
        assert_eq!(report.network_audit.blocked_hits, 0);

        let report_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report_path).expect("read report file"))
                .expect("parse report json");
        assert_eq!(report_json["networkAudit"]["mode"], "none");
        assert_eq!(report_json["networkAudit"]["rulesTotal"], 0);

        let _ = fs::remove_dir_all(&workdir);
    }

    #[test]
    fn report_build_allowlist_uses_policy_defaults_for_network_audit() {
        let run_id = "sr-test-report-allowlist";
        let workdir = temp_run_dir(run_id);
        write_mock_vm_artifacts(&workdir);

        let mut policy = sample_policy();
        policy.network.mode = NetworkMode::Allowlist;
        policy.network.egress = vec![NetworkEgressRule {
            protocol: Some("tcp".to_string()),
            host: None,
            cidr: Some("1.1.1.1/32".to_string()),
            port: Some(443),
        }];
        let compile_bundle = compile_dry_run(&policy).expect("compile should succeed");
        let request = RunnerControlRequest {
            compile_bundle,
            runtime_context: RuntimeContext {
                workdir: workdir.to_string_lossy().to_string(),
                timeout_sec: 1,
                sample_interval_ms: None,
                cgroup_path: None,
            },
        };
        let runner = Runner::with_runtime(RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        });

        let mut prepared = runner.prepare(request).expect("prepare should succeed");
        runner
            .cleanup(&mut prepared)
            .expect("cleanup should succeed");

        let report_path = prepared.artifacts_dir().join(&prepared.artifacts.report);
        let monitor_result = MonitorResult {
            exit_code: 0,
            timed_out: false,
            sample_count: 0,
        };
        let report = build_and_write_report(&prepared, &policy, &monitor_result, &report_path)
            .expect("report build should succeed");
        let recomputed = compute_integrity_digest(&report).expect("recompute digest");
        assert_eq!(report.integrity.digest, recomputed);
        assert_eq!(report.network_audit.mode, "allowlist");
        assert_eq!(report.network_audit.rules_total, 1);
        assert_eq!(report.network_audit.allowed_hits, 0);
        assert_eq!(report.network_audit.blocked_hits, 0);

        let report_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report_path).expect("read report file"))
                .expect("parse report json");
        assert_eq!(report_json["networkAudit"]["mode"], "allowlist");
        assert_eq!(report_json["networkAudit"]["rulesTotal"], 1);

        let _ = fs::remove_dir_all(&workdir);
    }

    #[test]
    fn report_build_allowlist_aggregates_network_rule_hit_events() {
        let run_id = "sr-test-report-allowlist-hit";
        let workdir = temp_run_dir(run_id);
        write_mock_vm_artifacts(&workdir);

        let mut policy = sample_policy();
        policy.network.mode = NetworkMode::Allowlist;
        policy.network.egress = vec![NetworkEgressRule {
            protocol: Some("tcp".to_string()),
            host: None,
            cidr: Some("1.1.1.1/32".to_string()),
            port: Some(443),
        }];
        let compile_bundle = compile_dry_run(&policy).expect("compile should succeed");
        let request = RunnerControlRequest {
            compile_bundle,
            runtime_context: RuntimeContext {
                workdir: workdir.to_string_lossy().to_string(),
                timeout_sec: 1,
                sample_interval_ms: None,
                cgroup_path: None,
            },
        };
        let runner = Runner::with_runtime(RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        });

        let mut prepared = runner.prepare(request).expect("prepare should succeed");
        runner
            .cleanup(&mut prepared)
            .expect("cleanup should succeed");

        let synthetic_hit = EvidenceEvent {
            timestamp: "2026-02-12T10:00:00Z".to_string(),
            run_id: prepared.run_id.clone(),
            stage: STAGE_CLEANUP.to_string(),
            event_type: EVENT_NETWORK_RULE_HIT.to_string(),
            payload: serde_json::json!({
                "tap": format!("sr-tap-{}", prepared.run_id),
                "table": "safe_run",
                "chain": "forward",
                "protocol": "tcp",
                "target": "1.1.1.1/32",
                "port": 443,
                "allowedHits": 5,
                "blockedHits": 2
            }),
            hash_prev: "sha256:test-prev".to_string(),
            hash_self: "sha256:test-self".to_string(),
        };
        let mut log = std::fs::OpenOptions::new()
            .append(true)
            .open(prepared.event_log_path())
            .expect("open event log");
        writeln!(
            log,
            "{}",
            serde_json::to_string(&synthetic_hit).expect("serialize synthetic hit event")
        )
        .expect("append synthetic hit event");

        let report_path = prepared.artifacts_dir().join(&prepared.artifacts.report);
        let monitor_result = MonitorResult {
            exit_code: 0,
            timed_out: false,
            sample_count: 0,
        };
        let report = build_and_write_report(&prepared, &policy, &monitor_result, &report_path)
            .expect("report build should succeed");
        assert_eq!(report.network_audit.mode, "allowlist");
        assert_eq!(report.network_audit.rules_total, 1);
        assert_eq!(report.network_audit.allowed_hits, 5);
        assert_eq!(report.network_audit.blocked_hits, 2);

        let _ = fs::remove_dir_all(&workdir);
    }

    #[test]
    fn run_outcome_error_returns_sr_run_001_for_failed_state() {
        let monitor_result = MonitorResult {
            exit_code: 17,
            timed_out: false,
            sample_count: 2,
        };
        let err = run_outcome_error(
            RunState::Failed,
            &monitor_result,
            Path::new("/tmp/safe-run/report.json"),
        )
        .expect("failed state should map to SR-RUN-001");
        assert_eq!(err.code, SR_RUN_001);
        assert_eq!(err.path, "run.exitCode");
        assert!(err.message.contains("17"));
    }

    #[test]
    fn run_outcome_error_returns_none_for_finished_state() {
        let monitor_result = MonitorResult {
            exit_code: 0,
            timed_out: false,
            sample_count: 1,
        };
        let err = run_outcome_error(
            RunState::Finished,
            &monitor_result,
            Path::new("/tmp/safe-run/report.json"),
        );
        assert!(err.is_none());
    }

    fn sample_policy() -> PolicySpec {
        PolicySpec {
            api_version: "policy.safe-run.dev/v1alpha1".to_string(),
            metadata: Metadata {
                name: "cli-report-test".to_string(),
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
                egress: vec![],
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        }
    }

    fn temp_run_dir(run_id: &str) -> PathBuf {
        let mut base = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        base.push(format!("safe-run-cli-{run_id}-{nanos}"));
        let workdir = base.join(run_id);
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&workdir).expect("create workdir");
        workdir
    }

    fn write_mock_vm_artifacts(workdir: &Path) {
        let artifacts_dir = workdir.join("artifacts");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");
        fs::write(artifacts_dir.join("vmlinux"), b"kernel-image").expect("write mock kernel");
        fs::write(artifacts_dir.join("rootfs.ext4"), b"rootfs-image").expect("write mock rootfs");
    }

    fn temp_policy_path(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        path.push(format!("safe-run-cli-{label}-{id}.yaml"));
        path
    }
}
