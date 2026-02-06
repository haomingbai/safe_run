# M2 模块划分

## 1. sr-policy

- 新增：`path_security.rs`, `mount_constraints.rs`。
- 职责：挂载安全判定与组合约束。

## 2. sr-compiler

- 新增：`mount_plan.rs`。
- 职责：输出可回滚的挂载执行计划。

## 3. sr-runner

- 新增：`mount_executor.rs`, `rollback.rs`。
- 职责：按计划应用挂载并保障失败可清理。

## 4. sr-evidence

- 新增：挂载审计聚合器。
- 职责：将挂载决策注入报告 `mountAudit`。

## 5. 测试模块

- `tests/mount_allowlist`
- `tests/mount_escape_cases`
- `tests/mount_rollback`
