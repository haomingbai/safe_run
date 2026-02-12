# TODO：M3 冲刺计划（测试主导 / 先澄清后编码）

> 本节用于进入 M3 后的执行分解与进度跟踪；接口语义与阶段边界以 `plan/` 为准。
> 详细执行计划见：`M3_STAGE_PLAN.md`。

## 1. 依据（唯一事实来源）

- 全局：`plan/OVERVIEW.md`、`plan/INTERFACE_BASELINE.md`、`plan/ENGINEERING_CONVENTIONS.md`
- M3：`plan/M3/OVERVIEW.md`、`plan/M3/MODULES.md`、`plan/M3/INTERFACES.md`、`plan/M3/ARCHITECTURE.md`、`plan/M3/REFERENCES.md`
- M3 澄清：`plan/M3/NETWORK_CLARIFICATIONS.md`
- 执行计划：`M3_STAGE_PLAN.md`

## 2. 全局验收标准（M3 DoD）

- [ ] **测试结果**：`cargo test` 全绿（默认不运行 `#[ignore]`）。
- [ ] **阶段边界**：不破坏 M0-M2 语义（`compile --dry-run` 约束仍成立；none 模式仍为默认拒绝）。
- [ ] **接口兼容**：仅 additive change；任何语义变更先更新 `plan/` 再改代码。
- [ ] **可回收**：网络规则与资源必须随 run 生命周期创建与销毁，异常退出不得残留。

## 3. 测试代码标准（M3 强制）

- [ ] 先写测试再写实现（每个 Stage 以“失败测试 -> 最小实现 -> 补覆盖”闭环）。
- [ ] 默认测试不依赖 root/真实 nft/tap：系统调用通过 trait 注入（参考 mount_executor 模式）。
- [ ] 快照与序列化必须确定性：规则排序稳定；不引入随机/时间到编译输出。
- [ ] 断言引用常量：错误码来自 `crates/sr-common`；事件类型来自 `crates/sr-evidence`。

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

- [ ] 在具备权限的 Linux 主机手工验收：TAP + nft + NAT/路由 + guest IP 配置闭环。
- [ ] 新增 `#[ignore]` 集成测试（默认不跑），并在文档中写明环境与依赖命令。

验收：

- [ ] 目标环境可复现：allowlist 目标可达、非 allowlist 目标不可达、命中可审计、异常退出后规则可回收。
