# M3 过渡评估与 Stage 拆分（测试先行）

> 目标：在保持默认无网的前提下，支持显式网络 allowlist（`network.mode=allowlist`），并将网络规则下发/命中/回收纳入证据链与报告（见 `plan/M3/OVERVIEW.md`）。
>
> 本文件是“执行计划（How）”，不是接口事实来源（What）。接口与语义以 `plan/` 为准。

## 0. 依据（唯一事实来源）

- 全局：`plan/OVERVIEW.md`、`plan/INTERFACE_BASELINE.md`、`plan/ENGINEERING_CONVENTIONS.md`
- M3：`plan/M3/OVERVIEW.md`、`plan/M3/MODULES.md`、`plan/M3/INTERFACES.md`、`plan/M3/ARCHITECTURE.md`、`plan/M3/REFERENCES.md`
- M3 澄清定稿：`plan/M3/NETWORK_CLARIFICATIONS.md`

## 1. 全局硬约束（M3 仍必须满足）

- 接口兼容：仅允许 additive change；禁止删除/重命名已发布字段（见 `plan/INTERFACE_BASELINE.md`）。
- M0 语义保持：`safe-run compile` 仅允许 `--dry-run`（见 `plan/M0/OVERVIEW.md` 与现有 CLI 行为）。
- 模块边界不漂移：`sr-policy/sr-compiler/sr-runner/sr-evidence/sr-cli/sr-ops` 按职责扩展（见 `plan/OVERVIEW.md`、`plan/M3/MODULES.md`）。
- 默认拒绝：未显式 allowlist 时必须无网；网络资源必须随 run 生命周期创建与销毁，禁止残留（见 `plan/M3/OVERVIEW.md`）。

## 2. 测试代码标准（必须遵守）

1. 先写测试再写实现：每个 Stage 必须以“新增失败测试 -> 最小实现 -> 补覆盖测试”完成。
2. 默认测试不依赖 root/真实 nft/tap：
   - 系统调用侧通过 trait 注入（参考 `crates/sr-runner/src/mount_executor.rs` 的 `MountApplier/MountRollbacker` 模式）。
   - 真实系统集成仅允许放在 `#[ignore]` 测试中，并在用例名/文档中明确运行前置条件。
3. 测试应确定性（deterministic）：
   - 不依赖外部网络、DNS、系统时间随机性（可通过注入/固定输入规避）。
   - 快照测试（如编译输出）必须稳定，规则排序必须固定。
4. 断言应引用常量：
   - 错误码常量来自 `crates/sr-common`。
   - 事件类型常量来自 `crates/sr-evidence`（避免散落硬编码字符串）。

## 3. 测试结果标准（每个 Stage 的最小 DoD）

- `cargo test` 全绿（默认不运行 `#[ignore]`）。
- Stage 覆盖的 crate 必须至少运行一次定向测试，例如：`cargo test -p sr-policy`。
- 若新增/修改快照：必须在评审中解释“为何变更是预期的”，并保持与 `plan/` 一致。

## 4. Stage 划分（一次 Stage 一次闭环）

### Stage 0：文档澄清定稿与任务拆解（先完成，再开始写代码）

内容：

- 将已确认的关键决策固化到 `plan/M3/NETWORK_CLARIFICATIONS.md`。
- 同步更新 `plan/M3/INTERFACES.md` 与 `plan/M3/ARCHITECTURE.md`（例如 nft 链从 `output` 更正为 `forward`）。
- 更新 `AGENTS.md` 与 `TODO.md`，明确 M3 的执行顺序与验收口径。

验收标准：

- 相关文档均可追溯到 `plan/`，且不存在自相矛盾表述。
- `TODO.md` 中 M3 任务具备可勾选的验收标准与命令。

### Stage 1：sr-common 错误码 + sr-policy 网络规则校验（SR-POL-201）

先写代表性测试：

- `mode=allowlist` 且 `egress` 缺失/空数组 -> `valid=false`，`code=SR-POL-201`，`path=network.egress`。
- `protocol` 非 `tcp|udp`、端口越界、`host/cidr` 同时出现或同时缺失 -> `SR-POL-201` 且 path 精确到字段。

实现内容：

- `crates/sr-common`：新增 `SR_POL_201/SR_CMP_201/SR_RUN_201/SR_RUN_202` 常量（见 `plan/M3/INTERFACES.md`）。
- `crates/sr-policy`：新增 `network_constraints.rs` 并接入校验链（见 `plan/M3/MODULES.md`）。

验收标准：

- `cargo test -p sr-common -p sr-policy` 全绿。

### Stage 2：sr-compiler networkPlan 生成（SR-CMP-201）+ 快照回归

先写代表性测试：

- allowlist 策略可编译出非空 `networkPlan`，且字段结构与 `plan/M3/INTERFACES.md` 对齐。
- `network.mode=none` 仍输出 `networkPlan=null`（见 `plan/M3/NETWORK_CLARIFICATIONS.md`）。
- 编译输出确定性：同输入同输出（含规则排序）。
- 快照：新增 M3 allowlist 场景快照文件（保持现有 none 快照不变）。

实现内容：

- `crates/sr-compiler/src/network_plan.rs`：`NetworkPlanBuilder`（见 `plan/M3/MODULES.md`）。
- `crates/sr-compiler/src/lib.rs`：从“拒绝 allowlist”升级为“生成 networkPlan”。

验收标准：

- `cargo test -p sr-compiler` 全绿，且快照稳定。

### Stage 3：sr-runner 网络生命周期（apply/release）+ 失败清理（SR-RUN-201/202）

先写代表性测试（mock/recording，不依赖 root）：

- runner `launch` 前调用 network apply；apply 失败 -> `SR-RUN-201`，并走失败清理路径。
- runner `cleanup` 阶段必须调用 network release；release 失败 -> `SR-RUN-202`。
- 事件 gating：仅当 `evidencePlan.events` 包含 `network.*` 事件类型时才写入对应事件。

实现内容：

- `crates/sr-runner/src/network_lifecycle.rs`：`NetworkLifecycleManager`（见 `plan/M3/MODULES.md`）。
- `crates/sr-runner/src/runner.rs`：按生命周期在 launch/cleanup 织入 apply/release。

验收标准：

- `cargo test -p sr-runner` 全绿。

### Stage 4：证据链与报告 networkAudit（additive）

先写代表性测试：

- `sr-evidence`：给定网络事件流，`networkAudit` 聚合字段正确（规则数/命中数/模式）。
- `sr-cli`：构造事件流输入可生成包含 `networkAudit` 的 `run_report.json`，并保持 `integrity.digest` 可复算。

实现内容：

- `crates/sr-evidence`：新增网络事件常量 + `network_audit_from_events()` 聚合器，并将结果注入 `RunReport`（additive）。
- `crates/sr-cli`：报告构建路径补齐 `networkAudit`。

验收标准：

- `cargo test -p sr-evidence -p sr-cli` 全绿。

### Stage 5（待确认后再做）：`network.rule.hit` 采集与命中语义

前置条件：

- 必须先在 `plan/` 定稿 `network.rule.hit` 的 payload schema 与聚合口径（见 `plan/M3/NETWORK_CLARIFICATIONS.md` 的“待确认项”）。

验收标准（定稿后补充）：

- `cargo test` 全绿，且命中统计与 `RunReport.networkAudit` 一致。

### Stage 6（可选）：真实可出网闭环（kernel boot args 优先）

目标：

- 在具备权限的 Linux 主机完成真实 TAP + nft + NAT/路由 + guest IP 配置的集成验证。
- 建议通过 kernel boot args 提供更灵活的 guest 网络配置入口（需先在 `plan/` 定稿具体机制）。

测试策略：

- 增加 `#[ignore]` 集成测试（默认不跑），并在 README/检查单中给出手工验收步骤与环境声明（沙箱内/外、是否 root、依赖命令）。

验收标准：

- 在目标环境中可复现：
  - allowlist 目标可达；非 allowlist 目标不可达；
  - 规则命中可审计；异常退出后规则可回收。
