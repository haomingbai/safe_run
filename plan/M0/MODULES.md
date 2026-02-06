# M0 模块划分

## 1. sr-policy

- 职责：schema、解析、规范化、校验。
- 输入：Policy 文件。
- 输出：`ValidationResult`。

## 2. sr-compiler

- 职责：将规范化策略转成 `CompileBundle`。
- 输入：`normalizedPolicy`。
- 输出：Firecracker/Jailer/Cgroup 计划结构。

## 3. sr-cli

- 职责：命令入口与输出格式。
- 子命令：`validate`、`compile --dry-run`。

## 4. sr-common（建议）

- 职责：共享错误码、时间格式、ID 生成、serde 辅助。
- 约束：M0 只放稳定工具函数，避免过早抽象业务。

## 5. 测试模块

- `tests/policy_valid_cases`
- `tests/policy_invalid_cases`
- `tests/compile_snapshot`
