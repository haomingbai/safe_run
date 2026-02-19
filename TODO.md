# TODO：M3 冲刺计划（测试主导 / 先澄清后编码）

> 本节用于进入 M3 后的执行分解与进度跟踪；接口语义与阶段边界以 `plan/` 为准。
> 详细执行计划见：`M3_STAGE_PLAN.md`。

## 1. 依据（唯一事实来源）

- 全局：`plan/OVERVIEW.md`、`plan/INTERFACE_BASELINE.md`、`plan/ENGINEERING_CONVENTIONS.md`
- M3：`plan/M3/OVERVIEW.md`、`plan/M3/MODULES.md`、`plan/M3/INTERFACES.md`、`plan/M3/ARCHITECTURE.md`、`plan/M3/REFERENCES.md`
- M3 澄清：`plan/M3/NETWORK_CLARIFICATIONS.md`
- 执行计划：`M3_STAGE_PLAN.md`

## 2. 全局验收标准（M3 DoD）

- [x] **测试结果**：`cargo test` 全绿（默认不运行 `#[ignore]`）。
- [x] **阶段边界**：不破坏 M0-M2 语义（`compile --dry-run` 约束仍成立；none 模式仍为默认拒绝）。
- [x] **接口兼容**：仅 additive change；任何语义变更先更新 `plan/` 再改代码。
- [x] **可回收**：网络规则与资源必须随 run 生命周期创建与销毁，异常退出不得残留。

## 3. 测试代码标准（M3 强制）

- [x] 先写测试再写实现（每个 Stage 以“失败测试 -> 最小实现 -> 补覆盖”闭环）。
- [x] 默认测试不依赖 root/真实 nft/tap：系统调用通过 trait 注入（参考 mount_executor 模式）。
- [x] 快照与序列化必须确定性：规则排序稳定；不引入随机/时间到编译输出。
- [x] 断言引用常量：错误码来自 `crates/sr-common`；事件类型来自 `crates/sr-evidence`。

## 4. Stage 任务清单（按顺序执行）

### 4.1 Stage 0：文档澄清定稿与拆解

- [x] 新增 `plan/M3/NETWORK_CLARIFICATIONS.md` 并固化已确认决策。
- [x] 同步更新 `plan/M3/INTERFACES.md`（nft 链示例更正为 `forward`）与 `plan/M3/ARCHITECTURE.md`。
- [x] 更新 `AGENTS.md`，索引 M3 澄清并补充检查项。
- [x] 新增 `M3_STAGE_PLAN.md`，提供可执行的测试先行 Stage 拆分与验收口径。

验收：

- [x] 文档之间无自相矛盾表述；并且所有新增约束均落在 `plan/` 中（未定稿项明确标注“待确认”）。

### 4.2 Stage 1：sr-common 错误码 + sr-policy 网络规则校验（SR-POL-201）

- [x] 先写测试：allowlist 缺失/空规则、非法 protocol/port、host|cidr 互斥规则；`mode=none` + 非空 `egress` 直接报错。
- [x] 实现：新增网络校验模块并接入校验链；新增 M3 错误码常量。

验收：

- [x] `cargo test -p sr-common -p sr-policy` 全绿。
- [x] 错误码与 path 精确到 `network.egress[i].<field>`（或 `network.egress`）。

### 4.3 Stage 2：sr-compiler networkPlan 生成（SR-CMP-201）+ 快照

- [x] 先写测试：allowlist 编译输出包含非空 networkPlan；none 模式仍为 null；编译确定性。
- [x] 先写快照：新增 M3 allowlist 场景快照（不修改现有 none 快照）。
- [x] 实现：新增 `network_plan.rs` builder，并将其注入 `CompileBundle`。

验收：

- [x] `cargo test -p sr-compiler` 全绿。
- [x] 新快照稳定（规则排序固定），且与 `plan/M3/INTERFACES.md` 对齐。

### 4.4 Stage 3：sr-runner 网络生命周期（apply/release）+ 失败清理（SR-RUN-201/202）

- [x] 先写测试（mock/recording）：apply 失败/cleanup 失败/异常路径均能回收，并返回正确错误码。
- [x] 实现：新增 `network_lifecycle.rs`，在 runner 生命周期织入 apply/release。

验收：

- [x] `cargo test -p sr-runner` 全绿。
- [x] network 相关事件写入遵守 evidencePlan gating（不在事件列表则不写）。

### 4.5 Stage 4：证据链与报告 networkAudit（additive）

- [x] 先写测试：从事件流聚合 `networkAudit`；并验证 `integrity.digest` 可复算。
- [x] 实现：`sr-evidence` 增量扩展 RunReport；`sr-cli` 报告组装补齐 networkAudit。

验收：

- [x] `cargo test -p sr-evidence -p sr-cli` 全绿。

### 4.6 Stage 5：`network.rule.hit` 命中语义与采集

- [x] 先在 `plan/` 定稿：`network.rule.hit` payload schema、统计口径、`networkAudit` 在 `mode=none` 时的输出策略。
- [x] 定稿后：补齐 runner 采样逻辑与证据/报告聚合测试。

验收：

- [x] `cargo test` 全绿，且命中统计与 `networkAudit` 一致。

### 4.7 Stage 6（可选）：真实可出网闭环（kernel boot args 优先）

- [x] 在具备权限的 Linux 主机手工验收：TAP + nft + NAT/路由 + guest IP 配置闭环（采用隔离 `ip netns`，避免影响宿主机全局网络）。
- [x] 新增 `#[ignore]` 集成测试（默认不跑），并在文档中写明环境与依赖命令。
- [x] 当前主机能力探测与修复复验（2026-02-19）：已完成 `sudo` 授权并通过隔离 `ip netns + nft` 方案执行 `stage6_real_network_allowlist_closure`（`--ignored`）验收；同时确认无临时 nft 表/网卡/命名空间残留。
- [x] 固化脚本与运行条件：新增 `scripts/network_allowlist_acceptance.sh`（一键执行 Stage 6 验收），运行条件与安全边界记录于 `README.md` 的“网络 allowlist 验收闭环”章节。

验收：

- [x] 目标环境可复现：allowlist 目标可达、非 allowlist 目标不可达、命中可审计、异常退出后规则可回收（隔离 netns 方案）。

---

## 5. M3 真实网络闭环修复计划（仅当验收要求“端到端真实落地”时启用）

> 背景：当前 M3 的 P0/P1 已满足“测试主导 / mock 闭环”（`cargo test` 默认全绿）。
> 若验收方要求“runner 在 Linux 上真实下发/回收 nftables + TAP，并采集命中计数”，则需要补齐以下工作。
> 依据：`plan/M3/NETWORK_CLARIFICATIONS.md`（forward 主链、运行期 DNS 解析、命中计数事件口径）与 `plan/M3/ARCHITECTURE.md`。

### 5.1 Stage 6b-0：确认验收口径与环境（阻塞点澄清）

- [x] 是否必须通过 safe-run runner 的 `NetworkLifecycle` 实现来下发/回收（而不是脚本手写 nft）
- [x] 是否要求对 microVM egress 生效（TAP/转发路径），还是允许以 host/netns 进程流量替代验证
- [x] 是否允许 root/sudo（以及是否允许在 CI 跑；若不允许，则仅能保留 `#[ignore]`）
- [x] 当前 M3 定稿主链为 `forward`；但 netns 内本地进程流量天然走 `output` hook
- [x] 验收路径选择：保持 `forward`（验收走“转发路径”，例如 TAP -> bridge -> 外网）
- [ ] 验收路径选择：使用 `output`（仅在先更新 `plan/M3/NETWORK_CLARIFICATIONS.md` 的前提下允许）

验收（本小节）：

- [x] 在验收记录中写明：宿主机环境、是否沙箱、是否 root、验收路径（forward/output）

当前探测记录（2026-02-19）：

- [x] 宿主机为 Linux，已检测到 `ip`/`nft`/`sudo` 可执行文件。
- [x] `sudo -n` 未通过（当前会话需要交互密码），自动化执行 Stage 6b `#[ignore]` 用例仍受阻；本次通过交互密码完成验收。

### 5.2 Stage 6b-1：sr-runner 实现真实 SystemNetworkLifecycle（apply/release/sample）

目标：把当前“内存展开 + DNS 解析”的 system 适配器升级为“真实 nft/tap 下发 + 回收 + 计数采样”。

- [x] 新增 trait（示例命名）：`NetworkCommandExecutor`，提供 `ip(args)` / `nft(args)` 执行能力
- [x] 默认实现走 `std::process::Command`，测试用 recording/mock 实现
- [x] 使用 `tap.name` 模板替换 `<runId>` 生成真实 tap 名
- [x] 按 `plan.nft.table` 创建/复用 nft table（实现必须与 `plan/M3/NETWORK_CLARIFICATIONS.md` 的最小结构一致）
- [x] 确保 `forward` 链存在且不误伤宿主转发：base chain 使用 `policy accept`，并通过 `iifname=<tap>` 的默认 drop 规则实现 deny-by-default
- [x] 将 allowlist 规则（含 host 运行期解析展开为多个 `/32`）转换为 nft 规则并启用 counter
- [x] 将“规则定位信息”写入 `AppliedNetwork`，便于 sample 与 release 精确定位
- [x] 读取 nft counter，按 `plan/M3/NETWORK_CLARIFICATIONS.md` 的 `network.rule.hit` 口径生成 `allowed_hits/blocked_hits`
- [x] 仅当 `allowed+blocked>0` 才返回该条命中（保持与事件写入逻辑一致）
- [x] 删除本次 run 创建的规则/链/表（至少保证无残留；并考虑“并发运行”场景避免误删其它 run 的资源）
- [x] release 失败必须向上返回，触发 `SR-RUN-202` 路径

测试（默认不依赖 root）：

- [x] 对 `NetworkCommandExecutor` 做 recording 断言：apply 会发出预期 nft/ip 命令序列
- [x] 对 `sample_rule_hits` 做 parsing 断言：从固定 `nft -a list ruleset`/json 输出构造出 hits
- [x] 覆盖 apply 失败、sample 失败、release 失败返回 `SR-RUN-201/202`

验收（本小节）：

- [x] `cargo test -p sr-runner`

补充记录（2026-02-19）：

- [x] 修复规则下发顺序为“先 allow、后 block、最后 tap 默认 drop”，避免多目标/多规则场景下 `ip daddr != target` 规则提前拦截合法流量。

### 5.3 Stage 6b-2：补齐事件字段与聚合一致性

- [x] 位置：`crates/sr-runner/src/runner.rs` 写入 `EVENT_NETWORK_PLAN_GENERATED` 的 payload
- [x] 值：当存在 `network_plan` 时应为 `allowlist`；否则 `none`

验收（本小节）：

- [x] `cargo test -p sr-evidence`

### 5.4 Stage 6b-3：端到端 `#[ignore]` 验收用例与脚本对齐（确保验证 safe-run 链路）

- [x] 由测试代码调用 `Runner`（使用真实 SystemNetworkLifecycle）触发 apply/release
- [x] 通过探测命令制造 allowlist 命中与阻断（curl/ping/udp 探测等）
- [x] 断言事件流中存在 `network.rule.hit` 且计数符合预期
- [x] 让脚本验证对象从“脚本手写 nft”切换到“safe-run runner 下发的 nft”（或在脚本中显式标注两者的区别并同时验证）
- [x] 运行命令固定为：`cargo test -p sr-runner stage6_real_network_allowlist_closure -- --ignored --nocapture`（需要 root，且依赖 `SAFE_RUN_STAGE6_*` 环境变量；建议直接跑脚本）
- [x] 脚本结束必须做“无残留探测”（netns / link / nft table）。
- [x] 脚本已对齐 M3 主链口径：移除 netns `output` 规则，审计探针改为检查 runner 下发的 `safe_run/forward` 命中。

验收（本小节）：

- [x] `./scripts/network_allowlist_acceptance.sh`
- [ ] （可选）手动运行：确保已设置 `SAFE_RUN_STAGE6_SETUP_CMD` / `SAFE_RUN_STAGE6_ALLOWED_PROBE_CMD` / `SAFE_RUN_STAGE6_BLOCKED_PROBE_CMD` / `SAFE_RUN_STAGE6_AUDIT_PROBE_CMD` / `SAFE_RUN_STAGE6_CLEANUP_PROBE_CMD`（以及可选的 `SAFE_RUN_STAGE6_CLEANUP_CMD`），再执行 `cargo test -p sr-runner stage6_real_network_allowlist_closure -- --ignored --nocapture`
