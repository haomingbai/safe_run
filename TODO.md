# TODO：M2 阶段执行计划（测试先行 / 单窗口 Stage）

## 0. 使用规则

- 任务状态用 Markdown 勾选框维护：`[ ]` 未完成，`[x]` 已完成。
- 每完成一个 Stage：至少跑对应 `cargo test ...`，并回填本文件。
- 所有结论与实现必须可追溯到 `plan/`；若需改变接口语义，必须先更新 `plan/` 并获得确认（见 AGENTS.md）。

## 1. 依据文档（唯一事实来源）

- plan/OVERVIEW.md
- plan/INTERFACE_BASELINE.md
- plan/M2/OVERVIEW.md
- plan/M2/MODULES.md
- plan/M2/INTERFACES.md
- plan/M2/ARCHITECTURE.md
- plan/M2/REFERENCES.md
- 本执行拆分来源：M2_STAGE_PLAN.md（仅作拆分与执行口径，不覆盖 plan/ 约束）

## 2. M2 阶段硬约束（必须始终满足）

- [ ] M2-C-001 接口兼容：仅 additive change；禁止删除/重命名已发布字段。
- [ ] M2-C-002 M0-M2 网络固定：`network.mode=none`；`CompileBundle.networkPlan` 必须保持 `null`。
- [ ] M2-C-003 `PolicySpec.runtime.args` 与 `PolicySpec.mounts` 必须显式存在（可为空数组但不可缺失）。
- [ ] M2-C-004 挂载决策必须走 `sr-policy` 统一策略引擎，禁止绕过。

## 3. 待确认（信息不足会导致“臆造实现”）

> 你在 M2_STAGE_PLAN.md 已给出方向，但仍需要把“可执行参数/机制”定稿，否则无法写出不歧义的测试与实现。

- [x] M2-Q-001 宿主路径白名单“配置文件”形态与加载位置已定稿：
  - 来源优先级：CLI `--mount-allowlist` > 环境变量 `SAFE_RUN_MOUNT_ALLOWLIST` > 内置默认。
  - 格式：YAML（带 `schemaVersion: safe-run.mount-allowlist/v1`）。
  - 未提供配置时：使用内置默认白名单（与现有示例对齐）。
- [x] M2-Q-002 guestPath 命名空间规约已定稿：
  - guest allowlist 前缀来自 allowlist 配置（默认 `['/data']`）。
  - 关键系统路径 denylist 不可绕开；M2 仅允许扩展 allowlist，不允许配置绕开 denylist。
- [x] M2-Q-003 “风险组合”已定稿（不新增字段）：
  - 依据 `plan/M2/OVERVIEW.md` 的验收项“可写挂载 + 高权限执行”拒绝。
  - M2 采取最小且不歧义规则：`mounts[].read_only` 必须为 `true`；否则返回 `SR-POL-103`。
  - 若后续阶段要支持可写挂载，必须先在 `plan/` 中新增可证明的权限模型字段（additive）再放开。
- [x] M2-Q-004 mounts 字段命名映射已定稿：
  - 规范字段名：`source/target/read_only`；兼容输入别名：`hostPath/guestPath/readOnly`。

## Stage 0：需求/设计确认与 plan/ 补丁（必须先完成）

**目标**：让 Stage 1-6 的测试与实现都有明确可执行的规范。

- [x] M2-S0-001 将“挂载白名单配置文件”最小规范写入 plan/M2（来源优先级 + YAML schemaVersion + 默认值）。
- [x] M2-S0-002 将“guestPath 命名空间 + 关键路径 denylist 不可绕开”写入 plan/M2。
- [x] M2-S0-003 将“风险组合（可写挂载）处理：M2 禁止可写挂载，不新增字段”写入 plan/M2。
- [x] M2-S0-004 将 mounts 字段映射策略（canonical + alias）写入 plan/INTERFACE_BASELINE.md 与 plan/M2/INTERFACES.md。

### 验收标准（可执行）

- [ ] M2-A0-001 完成上述确认后，本 TODO 的 Stage 1-6 不再依赖“口头假设”。

---

## Stage 1：接口对齐与错误码脚手架（测试先行）

- [x] M2-S1-T001（先测试）sr-policy：解析 `hostPath/guestPath/readOnly` 样例成功并规范化到内部结构。
- [x] M2-S1-T002（先测试）sr-policy：新增无效用例覆盖 `hostPath/source` 为空、`guestPath/target` 非绝对路径。
- [x] M2-S1-I001（实现）sr-policy::Mount 增加 serde alias：`hostPath->source`、`guestPath->target`、`readOnly->read_only`（additive）。
- [x] M2-S1-I002（实现）sr-common 增加错误码常量：`SR_POL_101/102/103`、`SR_RUN_101`（按 plan/M2/INTERFACES.md）。

### Stage 1 验收标准

- [x] M2-S1-A001 `cargo test -p sr-policy`
- [x] M2-S1-A002 `cargo test -p sr-common`（如该 crate 有测试）

## Stage 2：PathSecurityEngine（canonicalize + 白名单前缀）

- [x] M2-S2-T001（先测试）临时目录下 allowlisted 路径通过。
- [x] M2-S2-T002（先测试）allowlisted 目录内 symlink 指向非白名单目录时拒绝。
- [x] M2-S2-T003（先测试）`..` 穿越 canonicalize 后落到白名单外时拒绝。
- [x] M2-S2-I001（实现）新增 `PathSecurityEngine`（crates/sr-policy/src/path_security.rs）。
- [x] M2-S2-I002（实现）在 `validate_policy()` 中调用；失败映射 `SR-POL-101`。

### Stage 2 验收标准

- [x] M2-S2-A001 `cargo test -p sr-policy`

## Stage 3：MountConstraints（敏感宿主路径 + guestPath 命名空间）

- [ ] M2-S3-T001（先测试）source canonicalize 后位于敏感宿主路径（如 `/proc`、`/sys`、`/dev`）拒绝。
- [ ] M2-S3-T002（先测试）target 不在允许命名空间或覆盖关键路径时拒绝。
- [ ] M2-S3-I001（实现）新增 `mount_constraints.rs`，落地 denylist + 命名空间规则。
- [ ] M2-S3-I002（实现）错误码映射：target/命名空间违规使用 `SR-POL-102`。

### Stage 3 验收标准

- [ ] M2-S3-A001 `cargo test -p sr-policy`

## Stage 4：编译阶段 MountPlanBuilder（可回滚计划 + 快照测试）

- [ ] M2-S4-T001（先测试）sr-compiler：包含 mounts 的 PolicySpec 编译输出必须包含“挂载计划”（需以 Stage 0 的设计确认决定承载位置）。
- [ ] M2-S4-T002（先测试）挂载计划快照测试，确保规则变更可感知。
- [ ] M2-S4-I001（实现）新增 crates/sr-compiler/src/mount_plan.rs（MountPlanBuilder）。
- [ ] M2-S4-I002（实现）注入编译输出，且保持 `networkPlan=null`。

### Stage 4 验收标准

- [ ] M2-S4-A001 `cargo test -p sr-compiler`

## Stage 5：Runner 挂载执行与失败回滚（尽量不依赖 root）

- [ ] M2-S5-T001（先测试）挂载计划执行顺序/回滚顺序的纯逻辑测试（不做真实 mount）。
- [ ] M2-S5-T002（可选）root-only 最小集成测试使用 `#[ignore]`。
- [ ] M2-S5-I001（实现）新增 mount_executor.rs / rollback.rs，按计划应用并逆序回滚。
- [ ] M2-S5-I002（实现）挂载应用失败返回 `SR-RUN-101`。

### Stage 5 验收标准

- [ ] M2-S5-A001 `cargo test -p sr-runner`

## Stage 6：证据链与报告 mountAudit（additive）

- [ ] M2-S6-T001（先测试）从 events.jsonl（含 mount.validated/rejected/applied）生成报告，断言包含 `mountAudit` 且计数/reasons 正确。
- [ ] M2-S6-I001（实现）sr-evidence 增加挂载审计聚合器，注入 `RunReport.mountAudit`（additive）。
- [ ] M2-S6-I002（实现）补齐新事件类型：mount.validated / mount.rejected / mount.applied。

### Stage 6 验收标准

- [ ] M2-S6-A001 `cargo test -p sr-evidence`
- [ ] M2-S6-A002 `cargo test`

---

## M2 总体验收标准（阶段 DoD，可编译可运行验证）

- [ ] M2-DOD-001 路径逃逸（`..` / symlink / 非白名单）稳定拦截并返回正确错误码（SR-POL-101/102/103）。
- [ ] M2-DOD-002 编译输出包含可回滚挂载计划，且快照测试能感知变更。
- [ ] M2-DOD-003 挂载应用失败能回滚并返回 SR-RUN-101。
- [ ] M2-DOD-004 run_report.json additive 增加 mountAudit，并可从事件流重建。
- [ ] M2-DOD-005 约束回归：network 固定 none，CompileBundle.networkPlan 仍为 null。
