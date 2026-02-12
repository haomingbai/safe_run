pub(crate) const ARTIFACTS_DIR: &str = "artifacts";
pub(crate) const EVENTS_FILE: &str = "events.jsonl";
pub(crate) const REPORT_FILE: &str = "run_report.json";
pub(crate) const FIRECRACKER_CONFIG_FILE: &str = "firecracker-config.json";
pub(crate) const FIRECRACKER_API_SOCKET_FILE: &str = "firecracker.socket";
pub(crate) const RUNTIME_CONTEXT_FILE: &str = "runtime-context.json";
pub(crate) const VM_PID_FILE: &str = "vm.pid";
pub(crate) const CLEANUP_MARKER_FILE: &str = "cleanup.invoked";
pub(crate) const CGROUP_CPU_STAT_FILE: &str = "cpu.stat";
pub(crate) const CGROUP_MEMORY_CURRENT_FILE: &str = "memory.current";
pub(crate) const DEFAULT_SAMPLE_INTERVAL_MS: u64 = 1000;
pub(crate) const DEFAULT_CGROUP_PATH: &str = "/sys/fs/cgroup";
pub(crate) const GENESIS_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
pub(crate) const STAGE_COMPILE: &str = sr_evidence::STAGE_COMPILE;
pub(crate) const STAGE_PREPARE: &str = sr_evidence::STAGE_PREPARE;
pub(crate) const STAGE_MOUNT: &str = sr_evidence::STAGE_MOUNT;
pub(crate) const STAGE_LAUNCH: &str = sr_evidence::STAGE_LAUNCH;
pub(crate) const STAGE_MONITOR: &str = sr_evidence::STAGE_MONITOR;
pub(crate) const STAGE_CLEANUP: &str = sr_evidence::STAGE_CLEANUP;
pub(crate) const EVENT_COMPILE: &str = sr_evidence::EVENT_COMPILE;
pub(crate) const EVENT_RUN_PREPARED: &str = sr_evidence::EVENT_RUN_PREPARED;
pub(crate) const EVENT_MOUNT_VALIDATED: &str = sr_evidence::EVENT_MOUNT_VALIDATED;
pub(crate) const EVENT_MOUNT_REJECTED: &str = sr_evidence::EVENT_MOUNT_REJECTED;
pub(crate) const EVENT_MOUNT_APPLIED: &str = sr_evidence::EVENT_MOUNT_APPLIED;
pub(crate) const EVENT_NETWORK_PLAN_GENERATED: &str = sr_evidence::EVENT_NETWORK_PLAN_GENERATED;
pub(crate) const EVENT_NETWORK_RULE_APPLIED: &str = sr_evidence::EVENT_NETWORK_RULE_APPLIED;
#[allow(dead_code)]
pub(crate) const EVENT_NETWORK_RULE_HIT: &str = sr_evidence::EVENT_NETWORK_RULE_HIT;
pub(crate) const EVENT_NETWORK_RULE_RELEASED: &str = sr_evidence::EVENT_NETWORK_RULE_RELEASED;
pub(crate) const EVENT_NETWORK_RULE_CLEANUP_FAILED: &str =
    sr_evidence::EVENT_NETWORK_RULE_CLEANUP_FAILED;
pub(crate) const EVENT_VM_STARTED: &str = sr_evidence::EVENT_VM_STARTED;
pub(crate) const EVENT_RESOURCE_SAMPLED: &str = sr_evidence::EVENT_RESOURCE_SAMPLED;
pub(crate) const EVENT_VM_EXITED: &str = sr_evidence::EVENT_VM_EXITED;
pub(crate) const EVENT_RUN_CLEANED: &str = sr_evidence::EVENT_RUN_CLEANED;
pub(crate) const EVENT_RUN_FAILED: &str = sr_evidence::EVENT_RUN_FAILED;
