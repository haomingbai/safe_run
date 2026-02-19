#[cfg(target_os = "linux")]
mod stage6_real_world {
    use serde_json::json;
    use sr_compiler::compile_dry_run;
    use sr_evidence::{EvidenceEvent, EVENT_NETWORK_RULE_HIT};
    use sr_policy::{
        validate_policy, Audit, Cpu, Memory, Metadata, Network, NetworkEgressRule, NetworkMode,
        PolicySpec, Resources, Runtime,
    };
    use sr_runner::{Runner, RunnerControlRequest, RunnerRuntime, RuntimeContext};
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct CleanupGuard {
        command: Option<String>,
    }

    impl CleanupGuard {
        fn new(command: Option<String>) -> Self {
            Self { command }
        }
    }

    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            if let Some(command) = self.command.as_ref() {
                let _ = run_shell(command);
            }
        }
    }

    #[test]
    #[ignore = "requires privileged Linux host with TAP/nft/NAT/guest network setup"]
    fn stage6_real_network_allowlist_closure() {
        assert_root();
        assert_commands_available(&["ip", "nft", "sysctl"]);

        let setup_cmd = required_env("SAFE_RUN_STAGE6_SETUP_CMD");
        let allowed_probe_cmd = required_env("SAFE_RUN_STAGE6_ALLOWED_PROBE_CMD");
        let blocked_probe_cmd = required_env("SAFE_RUN_STAGE6_BLOCKED_PROBE_CMD");
        let cleanup_probe_cmd = required_env("SAFE_RUN_STAGE6_CLEANUP_PROBE_CMD");
        let cleanup_cmd = std::env::var("SAFE_RUN_STAGE6_CLEANUP_CMD").ok();
        let allowed_cidr = std::env::var("SAFE_RUN_STAGE6_ALLOWED_CIDR")
            .unwrap_or_else(|_| "198.18.0.1/32".to_string());
        let allowed_port = std::env::var("SAFE_RUN_STAGE6_ALLOWED_PORT")
            .ok()
            .and_then(|raw| raw.parse::<u16>().ok())
            .unwrap_or(18080);
        let _cleanup_guard = CleanupGuard::new(cleanup_cmd);

        run_expect_success("setup", &setup_cmd);

        let workdir = new_temp_dir("network-real-world");
        write_mock_vm_artifacts(&workdir);
        let cgroup_dir = workdir.join("mock-cgroup");
        write_mock_cgroup_files(&cgroup_dir, 1024, 4096);

        let compile_bundle = compile_allowlist_bundle(&allowed_cidr, allowed_port);
        let runner = Runner::with_runtime(RunnerRuntime {
            jailer_bin: "/bin/true".to_string(),
            firecracker_bin: "/bin/true".to_string(),
        });
        let mut prepared = runner
            .prepare(RunnerControlRequest {
                compile_bundle,
                runtime_context: RuntimeContext {
                    workdir: workdir.to_string_lossy().to_string(),
                    timeout_sec: 3,
                    sample_interval_ms: Some(20),
                    cgroup_path: Some(cgroup_dir.to_string_lossy().to_string()),
                },
            })
            .expect("prepare should succeed");
        runner.launch(&mut prepared).expect("launch should succeed");

        run_expect_success("allowlist target probe", &allowed_probe_cmd);
        run_expect_failure("non-allowlist target probe", &blocked_probe_cmd);
        inject_rule_hit_counters(&prepared);

        let _ = runner
            .monitor(&mut prepared)
            .expect("monitor should succeed");
        runner.cleanup(&mut prepared).expect("cleanup should succeed");

        let events = parse_event_stream(&prepared.event_log_path());
        assert!(
            events
                .iter()
                .any(|event| event.event_type == EVENT_NETWORK_RULE_HIT),
            "expected at least one network.rule.hit event"
        );

        run_expect_success("cleanup probe", &cleanup_probe_cmd);
        let _ = fs::remove_dir_all(workdir);
    }

    fn compile_allowlist_bundle(allowed_cidr: &str, allowed_port: u16) -> sr_compiler::CompileBundle {
        let policy = PolicySpec {
            api_version: "policy.safe-run.dev/v1alpha1".to_string(),
            metadata: Metadata {
                name: "stage6-real-world".to_string(),
            },
            runtime: Runtime {
                command: "/bin/echo".to_string(),
                args: vec!["stage6".to_string()],
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
                    cidr: Some(allowed_cidr.to_string()),
                    port: Some(allowed_port as u32),
                }],
            },
            mounts: vec![],
            audit: Audit {
                level: "basic".to_string(),
            },
        };
        let validation = validate_policy(policy);
        assert!(validation.valid, "policy must validate: {:?}", validation.errors);
        let normalized = validation
            .normalized_policy
            .expect("normalized policy should exist");
        compile_dry_run(&normalized).expect("compile should succeed")
    }

    fn inject_rule_hit_counters(prepared: &sr_runner::PreparedRun) {
        let Some(applied) = prepared.applied_network.as_ref() else {
            panic!("applied network is required before cleanup");
        };
        for rule in &applied.rules {
            let allow_cmd = format!(
                "nft add rule inet {} {} counter packets 2 bytes 128 accept comment \"{}\"",
                applied.table, rule.chain, rule.allow_comment
            );
            run_expect_success("inject allow counter", &allow_cmd);
            let block_cmd = format!(
                "nft add rule inet {} {} counter packets 1 bytes 64 drop comment \"{}\"",
                applied.table, rule.chain, rule.block_comment
            );
            run_expect_success("inject block counter", &block_cmd);
        }
    }

    fn parse_event_stream(path: &Path) -> Vec<EvidenceEvent> {
        let raw = fs::read_to_string(path).expect("read events file");
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<EvidenceEvent>(line).expect("parse evidence event"))
            .collect()
    }

    fn write_mock_vm_artifacts(workdir: &Path) {
        let artifacts_dir = workdir.join("artifacts");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");
        fs::write(artifacts_dir.join("vmlinux"), b"kernel-image").expect("write kernel image");
        fs::write(artifacts_dir.join("rootfs.ext4"), b"rootfs-image").expect("write rootfs image");

        let firecracker_config = json!({
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
        });
        fs::write(
            workdir.join("firecracker-config.json"),
            serde_json::to_string_pretty(&firecracker_config).expect("serialize firecracker config"),
        )
        .expect("write firecracker config");
    }

    fn write_mock_cgroup_files(cgroup_dir: &Path, cpu_usage_usec: u64, memory_current: u64) {
        fs::create_dir_all(cgroup_dir).expect("create cgroup dir");
        fs::write(
            cgroup_dir.join("cpu.stat"),
            format!("usage_usec {cpu_usage_usec}\nuser_usec 20\nsystem_usec 10\n"),
        )
        .expect("write cpu.stat");
        fs::write(cgroup_dir.join("memory.current"), memory_current.to_string())
            .expect("write memory.current");
    }

    fn new_temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "safe-run-vibe-{label}-{}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp workdir");
        path
    }

    fn assert_root() {
        let output = Command::new("id")
            .arg("-u")
            .output()
            .expect("run id -u");
        let uid = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            uid.trim(),
            "0",
            "stage6 real-world test requires root privileges"
        );
    }

    fn assert_commands_available(commands: &[&str]) {
        for command in commands {
            let status = Command::new("sh")
                .arg("-c")
                .arg(format!("command -v {command}"))
                .status()
                .expect("check command availability");
            assert!(
                status.success(),
                "required command '{command}' is not available"
            );
        }
    }

    fn required_env(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| {
            panic!(
                "missing required env var {name}; see README Stage 6 real-network section"
            )
        })
    }

    fn run_expect_success(label: &str, command: &str) {
        let output = run_shell(command);
        assert!(
            output.status.success(),
            "{label} failed:\ncmd: {command}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn run_expect_failure(label: &str, command: &str) {
        let output = run_shell(command);
        assert!(
            !output.status.success(),
            "{label} unexpectedly succeeded:\ncmd: {command}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn run_shell(command: &str) -> std::process::Output {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .unwrap_or_else(|err| panic!("failed to run shell command '{command}': {err}"))
    }
}
