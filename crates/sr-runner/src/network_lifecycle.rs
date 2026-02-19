use sr_compiler::{NetworkPlan, NftRule};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, ToSocketAddrs};
use std::process::Command;

const LINUX_IFNAME_MAX: usize = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedNetwork {
    pub tap_name: String,
    pub table: String,
    pub chains: Vec<String>,
    pub rules: Vec<AppliedNetworkRule>,
    pub default_drop_rules: Vec<AppliedDefaultDropRule>,
    pub created_tap: bool,
    pub created_table: bool,
    pub created_chains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedDefaultDropRule {
    pub chain: String,
    pub comment: String,
    pub handle: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedNetworkRule {
    pub chain: String,
    pub protocol: String,
    pub target: String,
    pub port: u16,
    pub allow_comment: String,
    pub allow_handle: Option<u64>,
    pub block_comment: String,
    pub block_handle: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkRuleHit {
    pub chain: String,
    pub protocol: String,
    pub target: String,
    pub port: u16,
    pub allowed_hits: u64,
    pub blocked_hits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkLifecycleError {
    pub path: String,
    pub message: String,
}

impl NetworkLifecycleError {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

pub trait NetworkCommandExecutor: Send + Sync {
    fn ip(&self, args: &[String]) -> Result<String, NetworkLifecycleError>;
    fn nft(&self, args: &[String]) -> Result<String, NetworkLifecycleError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemNetworkCommandExecutor;

impl NetworkCommandExecutor for SystemNetworkCommandExecutor {
    fn ip(&self, args: &[String]) -> Result<String, NetworkLifecycleError> {
        run_command("ip", args)
    }

    fn nft(&self, args: &[String]) -> Result<String, NetworkLifecycleError> {
        run_command("nft", args)
    }
}

pub trait HostResolver: Send + Sync {
    fn resolve_ipv4(&self, host: &str, port: u16) -> Result<Vec<String>, NetworkLifecycleError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemHostResolver;

impl HostResolver for SystemHostResolver {
    fn resolve_ipv4(&self, host: &str, port: u16) -> Result<Vec<String>, NetworkLifecycleError> {
        resolve_host_ipv4(host, port)
    }
}

/// Network lifecycle adapter for runner apply/release hooks.
pub trait NetworkLifecycle {
    fn apply(
        &self,
        run_id: &str,
        plan: &NetworkPlan,
    ) -> Result<AppliedNetwork, NetworkLifecycleError>;
    fn sample_rule_hits(
        &self,
        _applied: &AppliedNetwork,
    ) -> Result<Vec<NetworkRuleHit>, NetworkLifecycleError> {
        Ok(Vec::new())
    }
    fn release(&self, applied: &AppliedNetwork) -> Result<(), NetworkLifecycleError>;
}

pub struct SystemNetworkLifecycle {
    executor: Box<dyn NetworkCommandExecutor>,
    resolver: Box<dyn HostResolver>,
}

impl Default for SystemNetworkLifecycle {
    fn default() -> Self {
        Self {
            executor: Box::new(SystemNetworkCommandExecutor),
            resolver: Box::new(SystemHostResolver),
        }
    }
}

impl SystemNetworkLifecycle {
    pub fn with_adapters<E: NetworkCommandExecutor + 'static, R: HostResolver + 'static>(
        executor: E,
        resolver: R,
    ) -> Self {
        Self {
            executor: Box::new(executor),
            resolver: Box::new(resolver),
        }
    }
}

impl NetworkLifecycle for SystemNetworkLifecycle {
    fn apply(
        &self,
        run_id: &str,
        plan: &NetworkPlan,
    ) -> Result<AppliedNetwork, NetworkLifecycleError> {
        if plan.nft.chains.is_empty() {
            return Err(NetworkLifecycleError::new(
                "launch.network.apply",
                "network plan must include at least one nft chain",
            ));
        }

        let tap_name = materialize_tap_name(&plan.tap.name, run_id);
        let created_tap = ensure_tap(self.executor.as_ref(), &tap_name)?;
        let created_table = ensure_table(self.executor.as_ref(), &plan.nft.table)?;
        let mut created_chains = Vec::new();
        for chain in &plan.nft.chains {
            if ensure_chain(self.executor.as_ref(), &plan.nft.table, chain)? {
                created_chains.push(chain.clone());
            }
        }

        let mut applied_rules = Vec::new();
        for (rule_idx, rule) in plan.nft.rules.iter().enumerate() {
            let targets = resolve_rule_targets_with_resolver(self.resolver.as_ref(), rule)?;
            for chain in &plan.nft.chains {
                for (target_idx, target) in targets.iter().enumerate() {
                    let allow_comment = rule_comment(run_id, "allow", rule_idx, target_idx);
                    let block_comment = rule_comment(run_id, "block", rule_idx, target_idx);
                    add_allow_rule(
                        self.executor.as_ref(),
                        &plan.nft.table,
                        chain,
                        &tap_name,
                        rule,
                        target,
                        &allow_comment,
                    )?;
                    applied_rules.push(AppliedNetworkRule {
                        chain: chain.clone(),
                        protocol: rule.protocol.clone(),
                        target: target.clone(),
                        port: rule.port,
                        allow_comment,
                        allow_handle: None,
                        block_comment,
                        block_handle: None,
                    });
                }
            }
        }

        for rule in &applied_rules {
            let rule_spec = NftRule {
                protocol: rule.protocol.clone(),
                host: None,
                cidr: Some(rule.target.clone()),
                port: rule.port,
            };
            add_block_rule(
                self.executor.as_ref(),
                &plan.nft.table,
                &rule.chain,
                &tap_name,
                &rule_spec,
                &rule.target,
                &rule.block_comment,
            )?;
        }

        let mut default_drop_rules = Vec::with_capacity(plan.nft.chains.len());
        for (chain_idx, chain) in plan.nft.chains.iter().enumerate() {
            let comment = rule_comment(run_id, "default_drop", chain_idx, 0);
            add_default_drop_rule(
                self.executor.as_ref(),
                &plan.nft.table,
                chain,
                &tap_name,
                &comment,
            )?;
            default_drop_rules.push(AppliedDefaultDropRule {
                chain: chain.clone(),
                comment,
                handle: None,
            });
        }

        hydrate_rule_handles(
            self.executor.as_ref(),
            &plan.nft.table,
            &plan.nft.chains,
            &mut applied_rules,
            &mut default_drop_rules,
        )?;

        Ok(AppliedNetwork {
            tap_name,
            table: plan.nft.table.clone(),
            chains: plan.nft.chains.clone(),
            rules: applied_rules,
            default_drop_rules,
            created_tap,
            created_table,
            created_chains,
        })
    }

    fn sample_rule_hits(
        &self,
        applied: &AppliedNetwork,
    ) -> Result<Vec<NetworkRuleHit>, NetworkLifecycleError> {
        let mut by_comment = BTreeMap::new();
        for chain in &applied.chains {
            let output = list_chain_with_handles(self.executor.as_ref(), &applied.table, chain)?;
            for (comment, info) in parse_chain_counters(&output) {
                by_comment.insert(comment, info);
            }
        }

        let mut hits = Vec::with_capacity(applied.rules.len());
        for rule in &applied.rules {
            let allowed_hits = by_comment
                .get(&rule.allow_comment)
                .map(|info| info.packets)
                .unwrap_or(0);
            let blocked_hits = by_comment
                .get(&rule.block_comment)
                .map(|info| info.packets)
                .unwrap_or(0);
            hits.push(NetworkRuleHit {
                chain: rule.chain.clone(),
                protocol: rule.protocol.clone(),
                target: rule.target.clone(),
                port: rule.port,
                allowed_hits,
                blocked_hits,
            });
        }
        Ok(hits)
    }

    fn release(&self, applied: &AppliedNetwork) -> Result<(), NetworkLifecycleError> {
        let mut errors = Vec::new();
        for rule in &applied.rules {
            if let Some(handle) = rule.allow_handle {
                if let Err(err) = delete_rule_by_handle(
                    self.executor.as_ref(),
                    &applied.table,
                    &rule.chain,
                    handle,
                ) {
                    errors.push(err.message);
                }
            }
            if let Some(handle) = rule.block_handle {
                if let Err(err) = delete_rule_by_handle(
                    self.executor.as_ref(),
                    &applied.table,
                    &rule.chain,
                    handle,
                ) {
                    errors.push(err.message);
                }
            }
        }

        for rule in &applied.default_drop_rules {
            if let Some(handle) = rule.handle {
                if let Err(err) = delete_rule_by_handle(
                    self.executor.as_ref(),
                    &applied.table,
                    &rule.chain,
                    handle,
                ) {
                    errors.push(err.message);
                }
            }
        }

        for chain in applied.created_chains.iter().rev() {
            if let Err(err) = delete_chain(self.executor.as_ref(), &applied.table, chain) {
                errors.push(err.message);
            }
        }
        if applied.created_table {
            if let Err(err) = delete_table(self.executor.as_ref(), &applied.table) {
                errors.push(err.message);
            }
        }
        if applied.created_tap {
            if let Err(err) = delete_tap(self.executor.as_ref(), &applied.tap_name) {
                errors.push(err.message);
            }
        }

        if errors.is_empty() {
            return Ok(());
        }
        Err(NetworkLifecycleError::new(
            "cleanup.network.release",
            format!("network release collected {} error(s): {}", errors.len(), errors.join("; ")),
        ))
    }
}

fn resolve_rule_targets_with_resolver(
    resolver: &dyn HostResolver,
    rule: &NftRule,
) -> Result<Vec<String>, NetworkLifecycleError> {
    if let Some(cidr) = rule.cidr.as_ref() {
        return Ok(vec![cidr.clone()]);
    }

    if let Some(host) = rule.host.as_deref() {
        let resolved = resolver.resolve_ipv4(host, rule.port)?;
        if resolved.is_empty() {
            return Err(NetworkLifecycleError::new(
                "launch.network.dns",
                format!("host '{host}' resolved to no IPv4 addresses"),
            ));
        }
        return Ok(resolved);
    }

    Err(NetworkLifecycleError::new(
        "launch.network.apply",
        "network rule is missing host/cidr target",
    ))
}

fn resolve_host_ipv4(host: &str, port: u16) -> Result<Vec<String>, NetworkLifecycleError> {
    let addrs = (host, port).to_socket_addrs().map_err(|err| {
        NetworkLifecycleError::new(
            "launch.network.dns",
            format!("failed to resolve host '{host}': {err}"),
        )
    })?;
    let mut ipv4 = BTreeSet::new();
    for addr in addrs {
        if let IpAddr::V4(v4) = addr.ip() {
            ipv4.insert(format!("{v4}/32"));
        }
    }
    Ok(ipv4.into_iter().collect())
}

fn run_command(program: &str, args: &[String]) -> Result<String, NetworkLifecycleError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| NetworkLifecycleError::new("launch.network.apply", format!("failed to run {program}: {err}")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    Err(NetworkLifecycleError::new(
        "launch.network.apply",
        format!(
            "{program} command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    ))
}

fn ensure_tap(
    executor: &dyn NetworkCommandExecutor,
    tap_name: &str,
) -> Result<bool, NetworkLifecycleError> {
    let add_args = vec![
        "tuntap".to_string(),
        "add".to_string(),
        "dev".to_string(),
        tap_name.to_string(),
        "mode".to_string(),
        "tap".to_string(),
    ];
    let created = match executor.ip(&add_args) {
        Ok(_) => true,
        Err(err) if is_already_exists(&err.message) => false,
        Err(err) => return Err(err),
    };
    executor.ip(&[
        "link".to_string(),
        "set".to_string(),
        tap_name.to_string(),
        "up".to_string(),
    ])?;
    Ok(created)
}

fn delete_tap(
    executor: &dyn NetworkCommandExecutor,
    tap_name: &str,
) -> Result<(), NetworkLifecycleError> {
    executor.ip(&[
        "link".to_string(),
        "del".to_string(),
        tap_name.to_string(),
    ])?;
    Ok(())
}

fn ensure_table(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
) -> Result<bool, NetworkLifecycleError> {
    if executor.nft(&[
        "list".to_string(),
        "table".to_string(),
        "inet".to_string(),
        table.to_string(),
    ]).is_ok()
    {
        return Ok(false);
    }
    executor.nft(&[
        "add".to_string(),
        "table".to_string(),
        "inet".to_string(),
        table.to_string(),
    ])?;
    Ok(true)
}

fn delete_table(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
) -> Result<(), NetworkLifecycleError> {
    executor.nft(&[
        "delete".to_string(),
        "table".to_string(),
        "inet".to_string(),
        table.to_string(),
    ])?;
    Ok(())
}

fn ensure_chain(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
    chain: &str,
) -> Result<bool, NetworkLifecycleError> {
    if executor
        .nft(&[
            "list".to_string(),
            "chain".to_string(),
            "inet".to_string(),
            table.to_string(),
            chain.to_string(),
        ])
        .is_ok()
    {
        return Ok(false);
    }

    executor.nft(&[
        "add".to_string(),
        "chain".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        format!("{{ type filter hook {chain} priority 0; policy accept; }}"),
    ])?;
    Ok(true)
}

fn delete_chain(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
    chain: &str,
) -> Result<(), NetworkLifecycleError> {
    executor.nft(&[
        "delete".to_string(),
        "chain".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
    ])?;
    Ok(())
}

fn add_allow_rule(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
    chain: &str,
    tap_name: &str,
    rule: &NftRule,
    target: &str,
    comment: &str,
) -> Result<(), NetworkLifecycleError> {
    executor.nft(&[
        "add".to_string(),
        "rule".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        "iifname".to_string(),
        tap_name.to_string(),
        "ip".to_string(),
        "daddr".to_string(),
        target.to_string(),
        rule.protocol.clone(),
        "dport".to_string(),
        rule.port.to_string(),
        "counter".to_string(),
        "accept".to_string(),
        "comment".to_string(),
        format!("\"{comment}\""),
    ])?;
    Ok(())
}

fn add_block_rule(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
    chain: &str,
    tap_name: &str,
    rule: &NftRule,
    target: &str,
    comment: &str,
) -> Result<(), NetworkLifecycleError> {
    executor.nft(&[
        "add".to_string(),
        "rule".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        "iifname".to_string(),
        tap_name.to_string(),
        rule.protocol.clone(),
        "dport".to_string(),
        rule.port.to_string(),
        "ip".to_string(),
        "daddr".to_string(),
        "!=".to_string(),
        target.to_string(),
        "counter".to_string(),
        "drop".to_string(),
        "comment".to_string(),
        format!("\"{comment}\""),
    ])?;
    Ok(())
}

fn add_default_drop_rule(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
    chain: &str,
    tap_name: &str,
    comment: &str,
) -> Result<(), NetworkLifecycleError> {
    executor.nft(&[
        "add".to_string(),
        "rule".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        "iifname".to_string(),
        tap_name.to_string(),
        "counter".to_string(),
        "drop".to_string(),
        "comment".to_string(),
        format!("\"{comment}\""),
    ])?;
    Ok(())
}

fn delete_rule_by_handle(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
    chain: &str,
    handle: u64,
) -> Result<(), NetworkLifecycleError> {
    executor.nft(&[
        "delete".to_string(),
        "rule".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        "handle".to_string(),
        handle.to_string(),
    ])?;
    Ok(())
}

fn hydrate_rule_handles(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
    chains: &[String],
    rules: &mut [AppliedNetworkRule],
    default_drop_rules: &mut [AppliedDefaultDropRule],
) -> Result<(), NetworkLifecycleError> {
    let mut by_comment = BTreeMap::new();
    for chain in chains {
        let output = list_chain_with_handles(executor, table, chain)?;
        for (comment, info) in parse_chain_counters(&output) {
            by_comment.insert(comment, info);
        }
    }

    for rule in rules {
        rule.allow_handle = by_comment.get(&rule.allow_comment).map(|info| info.handle);
        rule.block_handle = by_comment.get(&rule.block_comment).map(|info| info.handle);
    }
    for rule in default_drop_rules {
        rule.handle = by_comment.get(&rule.comment).map(|info| info.handle);
    }
    Ok(())
}

fn list_chain_with_handles(
    executor: &dyn NetworkCommandExecutor,
    table: &str,
    chain: &str,
) -> Result<String, NetworkLifecycleError> {
    executor.nft(&[
        "-a".to_string(),
        "list".to_string(),
        "chain".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
    ])
}

fn rule_comment(run_id: &str, kind: &str, rule_idx: usize, target_idx: usize) -> String {
    let mut hasher = DefaultHasher::new();
    run_id.hash(&mut hasher);
    let run_hash = hasher.finish() as u32;
    format!("safe_run_{run_hash:08x}_{kind}_{rule_idx}_{target_idx}")
}

fn materialize_tap_name(template: &str, run_id: &str) -> String {
    let candidate = template.replace("<runId>", run_id);
    if candidate.len() <= LINUX_IFNAME_MAX {
        return candidate;
    }

    let mut hasher = DefaultHasher::new();
    candidate.hash(&mut hasher);
    let suffix = format!("{:08x}", hasher.finish() as u32);
    format!("sr-tap-{suffix}")
}

fn is_already_exists(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("file exists") || normalized.contains("exists")
}

#[derive(Debug, Clone, Copy)]
struct RuleCounterInfo {
    handle: u64,
    packets: u64,
}

fn parse_chain_counters(output: &str) -> BTreeMap<String, RuleCounterInfo> {
    let mut parsed = BTreeMap::new();
    for line in output.lines() {
        let Some(comment) = extract_comment(line) else {
            continue;
        };
        let Some(handle) = extract_handle(line) else {
            continue;
        };
        let packets = extract_packets(line).unwrap_or(0);
        parsed.insert(comment, RuleCounterInfo { handle, packets });
    }
    parsed
}

fn extract_comment(line: &str) -> Option<String> {
    let marker = "comment \"";
    let start = line.find(marker)? + marker.len();
    let end = line[start..].find('"')?;
    Some(line[start..start + end].to_string())
}

fn extract_handle(line: &str) -> Option<u64> {
    let marker = "# handle ";
    let start = line.rfind(marker)? + marker.len();
    line[start..].trim().parse::<u64>().ok()
}

fn extract_packets(line: &str) -> Option<u64> {
    let marker = "packets ";
    let start = line.find(marker)? + marker.len();
    let end = line[start..]
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(line.len() - start);
    line[start..start + end].parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_compiler::{NetworkPlan, NftPlan, NftRule, TapPlan};
    use std::sync::{Arc, Mutex};

    #[derive(Default, Clone)]
    struct RecordingExecutor {
        ip_calls: Arc<Mutex<Vec<Vec<String>>>>,
        nft_calls: Arc<Mutex<Vec<Vec<String>>>>,
        nft_chain_list_output: Arc<Mutex<String>>,
    }

    impl RecordingExecutor {
        fn with_chain_output(output: impl Into<String>) -> Self {
            Self {
                nft_chain_list_output: Arc::new(Mutex::new(output.into())),
                ..Self::default()
            }
        }
    }

    impl NetworkCommandExecutor for RecordingExecutor {
        fn ip(&self, args: &[String]) -> Result<String, NetworkLifecycleError> {
            self.ip_calls
                .lock()
                .expect("lock ip calls")
                .push(args.to_vec());
            Ok(String::new())
        }

        fn nft(&self, args: &[String]) -> Result<String, NetworkLifecycleError> {
            self.nft_calls
                .lock()
                .expect("lock nft calls")
                .push(args.to_vec());
            if args.first().is_some_and(|arg| arg == "-a") {
                return Ok(self
                    .nft_chain_list_output
                    .lock()
                    .expect("lock chain output")
                    .clone());
            }
            Ok(String::new())
        }
    }

    #[derive(Default, Clone, Copy)]
    struct StaticResolver;

    impl HostResolver for StaticResolver {
        fn resolve_ipv4(&self, host: &str, _port: u16) -> Result<Vec<String>, NetworkLifecycleError> {
            Ok(vec![format!("{host}/32")])
        }
    }

    fn sample_plan() -> NetworkPlan {
        NetworkPlan {
            tap: TapPlan {
                name: "sr-tap-<runId>".to_string(),
            },
            nft: NftPlan {
                table: "safe_run".to_string(),
                chains: vec!["forward".to_string()],
                rules: vec![NftRule {
                    protocol: "tcp".to_string(),
                    host: None,
                    cidr: Some("1.1.1.1/32".to_string()),
                    port: 443,
                }],
            },
        }
    }

    #[test]
    fn apply_replaces_tap_name_with_run_id() {
        let allow_comment = rule_comment("sr-20260210-001", "allow", 0, 0);
        let block_comment = rule_comment("sr-20260210-001", "block", 0, 0);
        let default_drop_comment = rule_comment("sr-20260210-001", "default_drop", 0, 0);
        let lifecycle = SystemNetworkLifecycle::with_adapters(
            RecordingExecutor::with_chain_output(format!(
                "chain forward {{\n                    iifname \"sr-tap-sr-20260210-001\" ip daddr 1.1.1.1/32 tcp dport 443 counter packets 0 bytes 0 accept comment \"{allow_comment}\" # handle 10\n                    iifname \"sr-tap-sr-20260210-001\" tcp dport 443 ip daddr != 1.1.1.1/32 counter packets 0 bytes 0 drop comment \"{block_comment}\" # handle 11\n                    iifname \"sr-tap-sr-20260210-001\" counter packets 0 bytes 0 drop comment \"{default_drop_comment}\" # handle 12\n                }}"
            )),
            StaticResolver,
        );
        let applied = lifecycle
            .apply("sr-20260210-001", &sample_plan())
            .expect("apply network plan");
        assert!(applied.tap_name.starts_with("sr-tap-"));
        assert!(applied.tap_name.len() <= LINUX_IFNAME_MAX);
        assert!(applied.created_tap);
        assert!(applied.chains.contains(&"forward".to_string()));
    }

    #[test]
    fn apply_issues_ip_and_nft_commands() {
        let allow_comment = rule_comment("sr-20260210-001", "allow", 0, 0);
        let block_comment = rule_comment("sr-20260210-001", "block", 0, 0);
        let default_drop_comment = rule_comment("sr-20260210-001", "default_drop", 0, 0);
        let executor = RecordingExecutor::with_chain_output(format!(
            "chain forward {{\n                iifname \"sr-tap-sr-20260210-001\" ip daddr 1.1.1.1/32 tcp dport 443 counter packets 2 bytes 100 accept comment \"{allow_comment}\" # handle 10\n                iifname \"sr-tap-sr-20260210-001\" tcp dport 443 ip daddr != 1.1.1.1/32 counter packets 1 bytes 50 drop comment \"{block_comment}\" # handle 11\n                iifname \"sr-tap-sr-20260210-001\" counter packets 0 bytes 0 drop comment \"{default_drop_comment}\" # handle 12\n            }}"
        ));
        let ip_calls = executor.ip_calls.clone();
        let nft_calls = executor.nft_calls.clone();
        let lifecycle = SystemNetworkLifecycle::with_adapters(executor, StaticResolver);

        let applied = lifecycle
            .apply("sr-20260210-001", &sample_plan())
            .expect("apply network plan");

        assert_eq!(applied.rules.len(), 1);
        assert_eq!(applied.rules[0].chain, "forward");
        assert_eq!(applied.rules[0].target, "1.1.1.1/32");
        assert_eq!(applied.rules[0].allow_handle, Some(10));
        assert_eq!(applied.rules[0].block_handle, Some(11));
        assert_eq!(applied.default_drop_rules.len(), 1);
        assert_eq!(applied.default_drop_rules[0].handle, Some(12));

        let ip_calls = ip_calls.lock().expect("lock ip calls").clone();
        assert!(ip_calls.iter().any(|args| {
            args.len() == 6
                && args[0] == "tuntap"
                && args[1] == "add"
                && args[2] == "dev"
                && args[3].starts_with("sr-tap-")
                && args[4] == "mode"
                && args[5] == "tap"
        }));

        let nft_calls = nft_calls.lock().expect("lock nft calls").clone();
        assert!(nft_calls
            .iter()
            .any(|args| args.windows(2).any(|window| window == ["add".to_string(), "rule".to_string()])));
    }

    #[test]
    fn sample_rule_hits_reads_allow_and_block_counters() {
        let allow_comment = rule_comment("sr-20260210-001", "allow", 0, 0);
        let block_comment = rule_comment("sr-20260210-001", "block", 0, 0);
        let lifecycle = SystemNetworkLifecycle::with_adapters(
            RecordingExecutor::with_chain_output(format!(
                "chain forward {{\n                    iifname \"sr-tap-sr-20260210-001\" ip daddr 1.1.1.1/32 tcp dport 443 counter packets 7 bytes 777 accept comment \"{allow_comment}\" # handle 10\n                    iifname \"sr-tap-sr-20260210-001\" tcp dport 443 ip daddr != 1.1.1.1/32 counter packets 3 bytes 333 drop comment \"{block_comment}\" # handle 11\n                }}"
            )),
            StaticResolver,
        );
        let applied = AppliedNetwork {
            tap_name: "sr-tap-sr-20260210-001".to_string(),
            table: "safe_run".to_string(),
            chains: vec!["forward".to_string()],
            rules: vec![AppliedNetworkRule {
                chain: "forward".to_string(),
                protocol: "tcp".to_string(),
                target: "1.1.1.1/32".to_string(),
                port: 443,
                allow_comment,
                allow_handle: Some(10),
                block_comment,
                block_handle: Some(11),
            }],
            default_drop_rules: vec![],
            created_tap: true,
            created_table: true,
            created_chains: vec!["forward".to_string()],
        };

        let hits = lifecycle
            .sample_rule_hits(&applied)
            .expect("sample hits should succeed");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].allowed_hits, 7);
        assert_eq!(hits[0].blocked_hits, 3);
    }
}
