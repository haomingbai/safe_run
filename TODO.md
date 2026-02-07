# TODO：M1 阶段执行计划（多上下文）

## 0. 使用规则

- 任务状态用 Markdown 勾选框维护：`[ ]` 未完成，`[x]` 已完成。
- 每次实现任务后，必须同步更新本文件对应勾选状态。
- 若与 `plan/` 设计约束冲突，以 `plan/` 为准，并先停下提问确认。

## 1. 依据文档（必须先读）

- `plan/OVERVIEW.md`
- `plan/INTERFACE_BASELINE.md`
- `plan/M1/OVERVIEW.md`
- `plan/M1/MODULES.md`
- `plan/M1/INTERFACES.md`
- `plan/M1/ARCHITECTURE.md`
- `plan/M1/REFERENCES.md`

## 2. 复杂度评估结论

- 结论：M1 从当前 M0 基线到完整交付，不适合在单一上下文窗口内一次性完成。
- 原因：需要新增 `sr-runner`、`sr-evidence` 两个模块，扩展 `sr-cli` 与 `sr-compiler`，并补齐运行失败路径、报告完整性与集成测试。
- 建议：按 8 个上下文窗口分批实现，每批完成后执行测试并回填勾选状态。

## 3. M1 阶段硬约束检查

- [ ] M1-C-001 网络模式固定 `network.mode=none`（不得提前实现 allowlist 生效）。
- [ ] M1-C-002 `I-PL-001`、`I-VA-001`、`I-CP-001` 字段与语义保持兼容。
- [ ] M1-C-003 仅新增字段（additive change），不得删除或重命名已发布字段。
- [ ] M1-C-004 `RunReport.schemaVersion` 固定为 `safe-run.report/v1`。
- [ ] M1-C-005 错误码命名空间必须使用 `SR-RUN-*`、`SR-EVD-*`（以及既有 `SR-POL-*`、`SR-CMP-*`）。

## 4. 上下文窗口拆分计划

### Context 1：脚手架与接口骨架

- [x] M1-CTX1-001 在 workspace 新增 crate：`crates/sr-runner`。
- [x] M1-CTX1-002 在 workspace 新增 crate：`crates/sr-evidence`。
- [x] M1-CTX1-003 更新根 `Cargo.toml` 的 workspace members。
- [x] M1-CTX1-004 在 `sr-common` 增加错误码常量：`SR-RUN-001`、`SR-RUN-002`、`SR-RUN-003`。
- [x] M1-CTX1-005 在 `sr-common` 增加错误码常量：`SR-EVD-001`、`SR-EVD-002`。
- [x] M1-CTX1-006 在 `sr-runner` 定义 `I-RN-001` 请求/响应结构体（含 `runId/state/artifacts`）。
- [x] M1-CTX1-007 在 `sr-evidence` 定义 `I-EV-001` 事件结构体（含 `hashPrev/hashSelf`）。
- [x] M1-CTX1-008 在 `sr-evidence` 定义 `I-RP-001` 的 M1 字段子集结构体。
- [x] M1-CTX1-009 为新增结构体补充最小序列化单元测试。

### Context 2：`sr-compiler` M1 可执行输出扩展

- [x] M1-CTX2-001 保留 `CompileBundle` 字段名与结构不变（兼容 M0）。
- [x] M1-CTX2-002 将编译输出从“仅 dry-run 演示”扩展到“可供 runner 使用”的配置内容。
- [x] M1-CTX2-003 明确并固化 `networkPlan=null`（M1 禁止网络能力）。
- [x] M1-CTX2-004 扩展 `evidencePlan.events`，覆盖 M1 事件类型所需集合。
- [x] M1-CTX2-005 新增编译失败分支测试（字段缺失/非法映射）。
- [x] M1-CTX2-006 新增编译输出兼容性快照测试（仅 additive 变化）。

### Context 3：`sr-runner` 的 `prepare/launch`

- [x] M1-CTX3-001 实现 `prepare`：创建 run 工作目录与产物目录。
- [x] M1-CTX3-002 实现 `prepare`：初始化运行上下文（含 `timeoutSec`）。
- [x] M1-CTX3-003 实现 `launch`：组装 jailer 与 Firecracker 启动参数。
- [x] M1-CTX3-004 实现 `launch`：启动前写入 `run.prepared` 事件。
- [x] M1-CTX3-005 实现 `launch`：成功启动后写入 `vm.started` 事件。
- [x] M1-CTX3-006 实现 `launch` 失败路径并映射 `SR-RUN-002`。
- [x] M1-CTX3-007 失败时触发统一清理入口（不可跳过 cleanup）。

### Context 3.5：代码拆分与注释规范化

- [x] M1-CTX3_5-001 按模块职责拆分 `sr-runner` 源码（`prepare/launch/event` 等），确保单文件职责单一。
- [x] M1-CTX3_5-002 保证单文件不超过 1000 行；若逼近上限，进一步拆分子模块。
- [x] M1-CTX3_5-003 保证单个函数不超过 50 行；确需超过时在函数注释中说明原因与不可拆分点。
- [x] M1-CTX3_5-004 为公共函数与关键逻辑补充清晰注释，明确函数功能与实现细节。

### Context 4：`sr-runner` 的 `monitor/cleanup`

- [x] M1-CTX4-001 实现 `monitor`：周期采样 cgroup v2 CPU/内存指标。
- [x] M1-CTX4-002 将采样结果写入 `resource.sampled` 事件。
- [x] M1-CTX4-003 实现超时控制并映射 `SR-RUN-003`。
- [x] M1-CTX4-004 实现退出采集并写入 `vm.exited` 事件（含 exitCode）。
- [x] M1-CTX4-005 实现 `cleanup`：清理临时资源与状态回收。
- [x] M1-CTX4-006 `cleanup` 完成后写入 `run.cleaned` 事件。
- [x] M1-CTX4-007 cleanup 异常时写入 `run.failed` 事件并保留错误码。
- [x] M1-CTX4-008 为状态机迁移增加单元测试（prepared/running/finished/failed）。

### Context 5：`sr-evidence` 事件链与报告生成

- [ ] M1-CTX5-001 实现 `event_writer`：事件顺序写入与落盘。
- [ ] M1-CTX5-002 实现 `hashing`：事件 `hashPrev -> hashSelf` 链式计算。
- [ ] M1-CTX5-003 实现 artifacts hash：`kernelHash/rootfsHash/policyHash/commandHash`。
- [ ] M1-CTX5-004 实现 `report_builder`：生成 `run_report.json`（M1 子集字段）。
- [ ] M1-CTX5-005 实现 `integrity.digest` 生成（可复算）。
- [ ] M1-CTX5-006 事件写入失败映射 `SR-EVD-001`。
- [ ] M1-CTX5-007 报告生成失败映射 `SR-EVD-002`。
- [ ] M1-CTX5-008 新增报告结构校验单元测试（字段完整性与类型）。

### Context 6：`sr-cli` 新增 `run` 子命令

- [ ] M1-CTX6-001 在 CLI 新增 `safe-run run --policy <file>`。
- [ ] M1-CTX6-002 `run` 命令复用 `validate -> compile -> runner -> evidence` 链路。
- [ ] M1-CTX6-003 `run` 成功返回 `runId`、`state`、`report` 路径。
- [ ] M1-CTX6-004 `run` 失败统一输出标准错误结构与错误码。
- [ ] M1-CTX6-005 保持 `validate`、`compile --dry-run` 既有行为不变。
- [ ] M1-CTX6-006 补充 CLI 命令分支与参数校验测试。

### Context 7：集成测试与示例

- [ ] M1-CTX7-001 新增 `tests/run_smoke`：最小任务执行与退出验证。
- [ ] M1-CTX7-002 新增 `tests/run_failure_paths`：启动失败、超时、异常退出。
- [ ] M1-CTX7-003 新增 `tests/report_schema_v1`：`run_report.json` 字段校验。
- [ ] M1-CTX7-004 新增事件链一致性测试：`hashPrev/hashSelf` 可重算。
- [ ] M1-CTX7-005 新增 `examples/`：无网执行示例。
- [ ] M1-CTX7-006 新增 `examples/`：只读根示例。
- [ ] M1-CTX7-007 新增 `examples/`：资源限制示例。

### Context 8：验收与归档

- [ ] M1-CTX8-001 增加 `M1_CHECKLIST.md`（仿照 M0 清单，逐项映射 M1 文档）。
- [ ] M1-CTX8-002 更新 `README.md` 到 M1 命令与边界说明。
- [ ] M1-CTX8-003 执行 `cargo test` 并记录结果。
- [ ] M1-CTX8-004 手工跑通 `safe-run run --policy ...` 最小链路并记录产物路径。
- [ ] M1-CTX8-005 核对错误码覆盖：`SR-RUN-001/002/003`、`SR-EVD-001/002`。
- [ ] M1-CTX8-006 核对阶段边界：确认没有实现 M2/M3 能力（挂载强化/网络白名单）。
- [ ] M1-CTX8-007 汇总文档依据并形成交付说明。

## 5. 待确认项（文档未完全明确，实施前需确认）

- [ ] M1-Q-001 Firecracker/jailer 二进制在本地与 CI 的标准路径与版本约束。
- [x] M1-Q-002 已确认：优先支持不启动真实 VM 的实现与测试；完成后补充真实 Firecracker 启动验证。
- [ ] M1-Q-003 `kernel/rootfs` 产物来源与校验基线（仓库内置、外部挂载或环境注入）。
- [ ] M1-Q-004 `integrity.digest` 的精确计算口径（整报告规范化 JSON 或字段拼接）。
- [x] M1-Q-005 已确认：采样周期提供默认值且可指定；`timeoutSec` 继续由 `runtimeContext.timeoutSec` 明确传入。

## 6. 完成定义（DoD）

- [ ] M1-DOD-001 `safe-run run` 可稳定执行最小不可信任务并退出。
- [ ] M1-DOD-002 `run_report.json` 满足 `safe-run.report/v1` M1 字段子集要求。
- [ ] M1-DOD-003 失败路径有事件留痕且返回标准错误码。
- [ ] M1-DOD-004 事件链与 artifacts hash 可重算并通过测试。
- [ ] M1-DOD-005 所有实现和文档均可追溯到 `plan/` 设计文件。
