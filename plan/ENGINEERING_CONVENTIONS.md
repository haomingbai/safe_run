# 工程规范：代码清理与统一范式（M0-M2 基线）

> 本文件用于固化“工程层面的统一规则”，避免出现同一能力多种写法、同一概念多套命名。
> 本文件不引入新能力、不改变接口语义；若与阶段设计冲突，以 plan/INTERFACE_BASELINE.md 与对应阶段文档为准。

## 1. 模块职责（不可漂移）

- sr-common：错误码常量、ErrorItem 等共享结构。
- sr-policy：PolicySpec 解析、语义校验、规范化输出（normalizedPolicy）。
- sr-compiler：将 normalizedPolicy 编译为 CompileBundle（含 mountPlan/evidencePlan 等）。
- sr-runner：执行编排（prepare/launch/monitor/cleanup），写入事件流。
- sr-evidence：事件链、hash 口径、报告生成与验证。
- sr-cli：参数解析、与用户交互、调用各 crate 并输出 JSON。

## 2. 命名规范（统一词汇表）

- allowlist：仅指“显式允许的前缀/规则”；不混用其它同义词。
- mountPlan：编译产物字段名（JSON）；Rust 结构用 MountPlan/MountPlanEntry。
- EvidenceEvent.type：统一采用点分隔固定集合（例如 mount.validated）。
- EvidenceEvent.stage：统一使用固定集合（例如 mount、launch）。

## 3. 常量统一（禁止硬编码散落）

- 事件类型常量：应集中定义并导出，runner/cli/compiler/tests 通过常量引用。
- stage 常量：同上。
- artifacts 文件名常量：runner 内集中定义，禁止跨文件重复定义。

## 4. 错误处理范式

- ErrorItem：必须包含 code/path/message，且 path 应可用于定位（字段路径或子系统路径）。
- ErrorItem.path 命名规则：
  - 字段校验错误：使用 schema 字段路径（例如 `mounts[0].source`、`network.mode`）。
  - 编译产物完整性错误：使用编译输出路径（例如 `mountPlan.enabled`、`evidencePlan.events`）。
  - 运行时编排错误：使用 `阶段.子系统[.动作]`（例如 `launch.preflight.firecracker`、`mount.apply`、`mount.rollback`、`monitor.timeout`）。
- 错误码：严格使用基线命名空间（SR-POL/SR-CMP/SR-RUN/SR-EVD/SR-OPS）。
- message：面向用户可读，避免出现过期阶段信息（例如 “M0 only supports …” 需与当前阶段口径一致）。
- 错误构造样板：各 crate 内应提供局部 helper（例如 `pol_error/cmp_error/mount_error`）避免重复拼接。

## 5. 注释与结构

- 公共函数必须有 doc comment，说明：职责、输入输出、错误码映射、阶段边界。
- 单文件职责单一；避免“入口文件堆业务细节”。

## 6. 测试组织

- 纯逻辑/结构测试：放 crate 内 #[cfg(test)] 或 crates/<crate>/tests/*。
- 跨模块验收数据：放顶层 tests/ 目录（YAML/JSON 快照与样例）。
- 测试断言尽量引用常量（事件类型、错误码），避免字符串漂移。
