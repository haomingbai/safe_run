# TODO：M3 前代码清理与规范化计划（统一范式 / 低耦合 / 文档一致）

> 本 TODO 的目标不是“加功能”，而是在进入 M3（网络 allowlist）之前把 M0-M2 代码整理到可维护状态：
>
> - 注释清晰、命名一致、模块职责单一、实现路径唯一
> - 同一类能力只保留一种实现方式（避免多个风格/多个实现并存）
> - 文档与实现一致（以 plan/ 为唯一事实来源）

## 0. 依据（唯一事实来源）

- 全局：plan/OVERVIEW.md、plan/INTERFACE_BASELINE.md
- M2：plan/M2/OVERVIEW.md、plan/M2/MODULES.md、plan/M2/INTERFACES.md、plan/M2/MOUNT_CLARIFICATIONS.md
- 工程规范（本次新增）：plan/ENGINEERING_CONVENTIONS.md

## 1. 本次清理的硬边界（必须始终满足）

- [x] 不突破阶段边界：M0-M2 不引入 `network.mode=allowlist` 的可执行能力；CompileBundle.networkPlan 必须保持 null。
- [x] 接口兼容：仅 additive change，禁止删除/重命名已发布字段（见 plan/INTERFACE_BASELINE.md）。
- [x] 不改语义：只做重构/命名/注释/抽取公共组件，确保 `cargo test` 全绿。

## 2. P0：文案与注释对齐（低风险、立刻降噪）

- [x] sr-cli：把 CLI 描述从 “M1” 修正为 “M0-M2” 或“当前阶段”（避免误导）。
- [x] sr-policy：把网络错误信息中的 “M0 only supports …” 修正为 “M0-M2 only supports …”。
- [x] sr-compiler：把编译网络约束信息中的 “M1 compile requires …” 修正为与基线一致的表述。
- [x] 统一注释风格：公共函数/关键逻辑必须有 doc comment，说明“职责 + 边界 + 错误码映射”。

验收：

- [x] `cargo fmt`、`cargo test` 全通过。
- [x] 关键提示文本不再出现阶段误标（抽样检查）。

## 3. P1：常量与命名统一（减少硬编码与重复）

目标：事件类型、文件名、默认路径、错误码 path label 的风格统一。

- [x] 事件类型常量统一来源：
  - 将 `run.prepared`、`vm.started`、`vm.exited`、`run.cleaned`、`run.failed`、`resource.sampled`、`compile` 等事件类型，像 mount.* 一样集中定义为常量，并在 runner/tests 中消灭硬编码字符串。
  - 只允许一个权威来源（建议 sr-evidence 导出常量，runner/cli/compiler 引用）。
- [x] “stage” 字段常量化：`compile` / `mount` / `launch` / `monitor` / `cleanup` 等 stage 名称避免散落字符串。
- [x] artifacts 文件名常量集中：runner 的 artifacts 文件名常量保持集中定义并避免重复定义。

验收：

- [x] 全仓 `grep` 硬编码事件字符串显著减少（只保留常量定义处/必要的序列化）。
- [x] `cargo test` 全通过。

## 4. P2：错误处理范式统一（ErrorItem / path label / 错误码）

目标：所有模块返回错误的方式一致，可聚合、可测试、可追踪。

- [x] 统一 ErrorItem.path 命名规则（例如：模块.子系统.动作 或 I-xxx 字段路径），并在 plan/ENGINEERING_CONVENTIONS.md 固化。
- [x] 统一“错误码选择”与“人类可读 message”的风格：
  - 校验错误（SR-POL-*）尽量指向具体字段路径（如 `mounts[i].source`）。
  - 编译错误（SR-CMP-*）指向 compile 输出缺失/非法请求的路径（如 `mountPlan.enabled`）。
  - 运行错误（SR-RUN-*）区分 preflight / mount / vm / cleanup 的 path 段。
- [x] 消除重复的错误构造样板：在各模块内抽取 `error_helpers`（不跨模块塞逻辑）。

验收：

- [x] 新增 1-2 个断言测试，确保错误 path 与 code 稳定。
- [x] `cargo test` 全通过。

## 5. P3：模块职责再核对与文件拆分（降低耦合，提升可读性）

目标：每个 crate 只做自己该做的事；避免“同一能力多处实现”。

- [x] sr-cli：只保留“参数解析 + I/O + 用户交互”；可复用纯逻辑下沉到对应 crate。
- [x] sr-policy：path_security 只负责 allowlist + canonicalize；mount_constraints 只负责敏感路径/guest 规则；校验链保持单一入口。
- [x] sr-compiler：mount_plan builder 与 evidencePlan/ensure_bundle_complete 职责边界明确。
- [x] sr-runner：Runner 编排与 MountExecutor/rollback 交互保持单一路径，避免未来 M3 再造一套。
- [x] sr-evidence：hashing/normalize/report_builder 的公共能力在 crate 内唯一来源；禁止 cli/runner 复制实现。

验收：

- [x] 每个 crate 的入口文件可在 2-3 分钟内读懂（主流程清晰）。
- [x] `cargo test` 全通过。

## 6. P4：开发范式统一（测试、目录、命名、示例）

- [x] 测试组织统一：单测在 crate 内；跨 crate 行为验证在 `crates/*/tests`；顶层 `tests/` 仅放样例/快照与验收数据。
- [x] 示例与样例命名统一：`m1_*.yaml`、`m2_*.yaml` 的策略文件命名规则明确并写入 README/plan。
- [x] 文档回链：README 与 plan/ 索引保持一致，避免过期阶段说明。

验收：

- [x] README 命令示例与当前 CLI 一致。
- [x] `cargo test` 全通过。

## 7. 执行顺序建议

1) P0（文案/注释） → 2) P1（常量统一） → 3) P2（错误范式） → 4) P3（拆分与职责） → 5) P4（测试/示例/文档）

## 附：已发现的“统一点候选”（用于落任务时逐项勾选）

- [x] 事件类型字符串在 sr-runner 与测试中多处硬编码，建议集中常量化。
- [x] 部分提示文本仍含 “M0/M1 only supports …” 的过期表述，需与“网络固定 none（M0-M2）”对齐。
- [x] CLI about 仍写 “M1 CLI”，与当前能力不一致。
