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

- [x] M1-C-001 网络模式固定 `network.mode=none`（不得提前实现 allowlist 生效）。
- [x] M1-C-002 `I-PL-001`、`I-VA-001`、`I-CP-001` 字段与语义保持兼容。
- [x] M1-C-003 仅新增字段（additive change），不得删除或重命名已发布字段。
- [x] M1-C-004 `RunReport.schemaVersion` 固定为 `safe-run.report/v1`。
- [x] M1-C-005 错误码命名空间必须使用 `SR-RUN-*`、`SR-EVD-*`（以及既有 `SR-POL-*`、`SR-CMP-*`）。

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

- [x] M1-CTX5-001 实现 `event_writer`：事件顺序写入与落盘。
- [x] M1-CTX5-002 实现 `hashing`：事件 `hashPrev -> hashSelf` 链式计算。
- [x] M1-CTX5-003 实现 artifacts hash：`kernelHash/rootfsHash/policyHash/commandHash`。
- [x] M1-CTX5-004 实现 `report_builder`：生成 `run_report.json`（M1 子集字段）。
- [x] M1-CTX5-005 实现 `integrity.digest` 生成（可复算）。
- [x] M1-CTX5-006 事件写入失败映射 `SR-EVD-001`。
- [x] M1-CTX5-007 报告生成失败映射 `SR-EVD-002`。
- [x] M1-CTX5-008 新增报告结构校验单元测试（字段完整性与类型）。

### Context 6：`sr-cli` 新增 `run` 子命令

- [x] M1-CTX6-001 在 CLI 新增 `safe-run run --policy <file>`。
- [x] M1-CTX6-002 `run` 命令复用 `validate -> compile -> runner -> evidence` 链路。
- [x] M1-CTX6-003 `run` 成功返回 `runId`、`state`、`report` 路径。
- [x] M1-CTX6-004 `run` 失败统一输出标准错误结构与错误码。
- [x] M1-CTX6-005 保持 `validate`、`compile --dry-run` 既有行为不变。
- [x] M1-CTX6-006 补充 CLI 命令分支与参数校验测试。

### Context 7：集成测试与示例

- [x] M1-CTX7-001 新增 `tests/run_smoke`：最小任务执行与退出验证。
- [x] M1-CTX7-002 新增 `tests/run_failure_paths`：启动失败、超时、异常退出。
- [x] M1-CTX7-003 新增 `tests/report_schema_v1`：`run_report.json` 字段校验。
- [x] M1-CTX7-004 新增事件链一致性测试：`hashPrev/hashSelf` 可重算。
- [x] M1-CTX7-005 新增 `examples/`：无网执行示例。
- [x] M1-CTX7-006 新增 `examples/`：只读根示例。
- [x] M1-CTX7-007 新增 `examples/`：资源限制示例。

### Context 8：验收与归档

- [x] M1-CTX8-001 增加 `M1_CHECKLIST.md`（仿照 M0 清单，逐项映射 M1 文档）。
- [x] M1-CTX8-002 更新 `README.md` 到 M1 命令与边界说明。
- [x] M1-CTX8-003 执行 `cargo test` 并记录结果。
- [x] M1-CTX8-004 手工跑通 `safe-run run --policy ...` 最小链路并记录产物路径。（沙箱外验证：`runId=sr-1770459110-232306729`，`state=finished`，`report=/tmp/safe-run/runs/sr-1770459110-232306729/artifacts/run_report.json`）
- [x] M1-CTX8-005 核对错误码覆盖：`SR-RUN-001/002/003`、`SR-EVD-001/002`。
- [x] M1-CTX8-006 核对阶段边界：确认没有实现 M2/M3 能力（挂载强化/网络白名单）。
- [x] M1-CTX8-007 汇总文档依据并形成交付说明。

### Context 8 后续修复任务（真实执行阻塞）

- [x] M1-FIX8-001 在 `sr-runner` 启动参数中显式传入 `--api-sock <writable_path>`，避免 Firecracker API socket 权限问题（对齐 M1-Q-001）。
- [x] M1-FIX8-002 增加 jailer 可执行文件预检与更明确的启动前错误提示，避免运行到 launch 阶段才失败。
- [x] M1-FIX8-003 在具备真实 `firecracker`（并使用本机兼容 `jailer` 脚本）环境后重新执行 `safe-run run --policy examples/m1_network_none.yaml`，结果 `state=finished`。

### Context 8 文档与工具补充（后续开发可用性）

- [x] M1-DOC8-001 在 `AGENTS.md` 固化 jailer/firecracker 缺失时的处理要求：先执行本地下载脚本，再进行真实运行验证。
- [x] M1-DOC8-002 在 `README.md` 补充“本地自举 firecracker/jailer”与对应 PATH 用法，减少对系统预装依赖。
- [x] M1-OPS8-001 新增 `scripts/get_firecracker.sh`，可按 release 版本下载并安装 `firecracker+jailer` 到 `artifacts/bin/`。
- [x] M1-OPS8-002 为下载脚本增加默认本地代理 `127.0.0.1:7890` 与可配置开关，提升受限网络环境可用性。

## 5. 待确认项（文档未完全明确，实施前需确认）

- [x] M1-Q-001 已确认：本地 Firecracker 可用, 但是不存在 jailer，且需显式传入 `--api-sock`（例如 `/tmp/firecracker.socket`）。
- [x] M1-Q-002 已确认：优先支持不启动真实 VM 的实现与测试；完成后补充真实 Firecracker 启动验证。
- [x] M1-Q-003 已确认：按 Firecracker 官方文档获取 rootfs/kernel，脚本落库、产物忽略提交。
- [x] M1-Q-004 已确认：`integrity.digest` 为“报告 JSON（将 `integrity.digest` 置空）”的规范化 JSON SHA-256。
- [x] M1-Q-006 已确认：`policyHash` 与 `commandHash` 采用规范化 JSON SHA-256（统一 JSON 规范化规则）。
- [x] M1-Q-005 已确认：采样周期提供默认值且可指定；`timeoutSec` 继续由 `runtimeContext.timeoutSec` 明确传入。

## 6. 完成定义（DoD）

- [x] M1-DOD-001 `safe-run run` 可稳定执行最小不可信任务并退出。
- [x] M1-DOD-002 `run_report.json` 满足 `safe-run.report/v1` M1 字段子集要求。
- [x] M1-DOD-003 失败路径有事件留痕且返回标准错误码。
- [x] M1-DOD-004 事件链与 artifacts hash 可重算并通过测试。
- [x] M1-DOD-005 所有实现和文档均可追溯到 `plan/` 设计文件。

## 7. M1 验收补齐修复计划（新增）

> 要求：运行问题先写测试暴露，再修复；文档问题直接修复。

### 7.1 运行问题：工作目录内核/根文件系统不可达

- [x] M1-FIX9-001 先新增失败复现测试：在 `sr-runner` 真实/模拟运行中强制使用运行工作目录，验证 `firecrackerConfig` 相对路径在 workdir 下不可达会失败（应覆盖 `SR-RUN-002` 或 `SR-EVD-002` 的明确错误路径）。
- [x] M1-FIX9-002 修复：在 `prepare` 阶段将 `kernel_image_path` 与 `rootfs` 解析为可用路径（如复制到 workdir 或改写为绝对路径），并更新相关单元测试与快照测试。

### 7.2 一致性问题：`compile` 事件缺失

- [x] M1-FIX9-003 先新增测试：编译 + 运行链路输出的事件流必须包含 `compile` 事件（对齐 `evidencePlan.events`）。
- [x] M1-FIX9-004 修复：在编译或运行链路中补写 `compile` 事件，并确保 hash 链可重算。

### 7.3 文档/文案问题（直接修复）

- [x] M1-FIX9-005 直接修复 CLI 文案：`safe-run` 的 `about` 从 M0 更新为 M1。

## 8. M1 验收遗留问题修复计划（新增）

> 问题：cleanup 先删 `firecracker-config.json`，导致报告阶段读取失败。

### 8.1 先用测试复现

- [x] M1-FIX10-001 新增 CLI 回归测试：执行 `prepare -> cleanup -> build_report` 路径，断言报告可生成（当前应失败）。
- [x] M1-FIX10-002 为上述测试补齐最小 fake 产物（kernel/rootfs/config/events）与临时工作目录清理。

### 8.2 方案确定与实现

- [x] M1-FIX10-003 方案评估：在 `cleanup` 前生成报告，或在 `cleanup` 中保留 `firecracker-config.json`，或在 `prepare` 中将关键字段缓存到报告输入。
- [x] M1-FIX10-004 选定方案并实现（需保持 M1 语义与接口不变，避免引入 M2/M3 能力）。

### 8.3 兼容性与回归验证

- [x] M1-FIX10-005 更新/新增单元测试与集成测试，确保 `run_report.json` 始终可生成。
- [x] M1-FIX10-006 校验 `integrity.digest`、`policyHash`、`commandHash` 仍可复算。
- [x] M1-FIX10-007 更新 `M1_CHECKLIST.md` 验收记录（包含修复后验证结果）。

## 9. M1 验收错误码回归修复计划（新增）

> 问题：异常退出（非零 exit code）未返回标准 `SR-RUN-*` 错误码。

### 9.1 复现与定位

- [x] M1-FIX11-001 复现并确认问题：`monitor` 非零退出仅置 `RunState::Failed`，`run` 命令仍返回成功码。

### 9.2 修复实现

- [x] M1-FIX11-002 在 `sr-runner monitor` 非零退出路径补写 `run.failed` 事件，并附 `errorCode=SR-RUN-001`。
- [x] M1-FIX11-003 在 `sr-cli run` 生成报告后根据失败状态返回 `SR-RUN-001`，不再返回成功退出码。

### 9.3 验证与归档

- [x] M1-FIX11-004 补充并通过测试：异常退出事件留痕 + CLI 错误码映射。
- [x] M1-FIX11-005 更新 `M1_CHECKLIST.md` 验收记录。
