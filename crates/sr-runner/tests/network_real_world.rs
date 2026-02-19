#[cfg(target_os = "linux")]
mod stage6_real_world {
    use std::process::Command;

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
        let audit_probe_cmd = required_env("SAFE_RUN_STAGE6_AUDIT_PROBE_CMD");
        let cleanup_probe_cmd = required_env("SAFE_RUN_STAGE6_CLEANUP_PROBE_CMD");
        let cleanup_cmd = std::env::var("SAFE_RUN_STAGE6_CLEANUP_CMD").ok();
        let _cleanup_guard = CleanupGuard::new(cleanup_cmd);

        run_expect_success("setup", &setup_cmd);
        run_expect_success("allowlist target probe", &allowed_probe_cmd);
        run_expect_failure("non-allowlist target probe", &blocked_probe_cmd);
        run_expect_success("network audit probe", &audit_probe_cmd);
        run_expect_success("cleanup probe", &cleanup_probe_cmd);
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
