# M3 模块划分

## 1. sr-policy

- 新增：`network_constraints.rs`。
- 职责：allowlist 语法与语义校验。

## 2. sr-compiler

- 新增：`network_plan.rs`。
- 职责：输出 nftables/TAP 结构化计划。

## 3. sr-runner

- 新增：`network_lifecycle.rs`。
- 职责：网络规则应用、命中采样、退出清理。

## 4. sr-evidence

- 新增：网络事件聚合器。
- 职责：构建 `networkAudit` 报告字段。

## 5. 测试模块

- `tests/network_default_deny`
- `tests/network_allowlist_pass`
- `tests/network_cleanup_recovery`
