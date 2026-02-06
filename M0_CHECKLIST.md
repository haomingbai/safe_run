# M0 完成检查单

本检查单用于确认 M0 交付满足 `plan/` 约束，且不越过阶段能力边界。

## 1. 文档依据

- `plan/OVERVIEW.md`
- `plan/INTERFACE_BASELINE.md`
- `plan/M0/OVERVIEW.md`
- `plan/M0/MODULES.md`
- `plan/M0/INTERFACES.md`
- `plan/M0/ARCHITECTURE.md`
- `plan/M0/REFERENCES.md`

## 2. 阶段边界（M0）

- [x] 仅提供 `safe-run validate` 与 `safe-run compile --dry-run`
- [x] 不进入真实 microVM 执行面
- [x] `network.mode` 仅允许 `none`
- [x] `CompileBundle.networkPlan` 固定为 `null`

## 3. 接口对齐

- [x] `I-PL-001`：`PolicySpec` 使用 `apiVersion: policy.safe-run.dev/v1alpha1`
- [x] `I-PL-001`：`runtime.args` 与 `mounts` 为显式必填字段（可为空数组但不可缺失）
- [x] `I-VA-001`：`ValidationResult` 输出 `valid/errors/warnings/normalizedPolicy`
- [x] `I-CP-001`：`CompileBundle` 输出 `firecrackerConfig/jailerPlan/cgroupPlan/networkPlan/evidencePlan`
- [x] `I-RN-001`、`I-EV-001`、`I-RP-001`：仅预留，不在 M0 启用

## 4. 错误码对齐

- [x] `SR-POL-001`：缺少必填字段
- [x] `SR-POL-002`：字段格式错误
- [x] `SR-POL-003`：策略语义冲突
- [x] `SR-CMP-001`：编译模板映射失败
- [x] `SR-CMP-002`：编译输出不完整或非法请求

## 5. 模块边界

- [x] `sr-policy`：解析、规范化、校验
- [x] `sr-compiler`：dry-run 编译计划生成
- [x] `sr-cli`：命令入口与 JSON 输出
- [x] `sr-common`：共享错误结构与错误码
- [x] 未将 Runner/Evidence 执行职责提前塞入 M0 模块

## 6. 测试矩阵

- [x] `tests/policy_valid_cases/minimal.yaml`
- [x] `tests/policy_invalid_cases/network_allowlist.yaml`
- [x] `tests/policy_invalid_cases/missing_runtime.yaml`
- [x] `tests/policy_invalid_cases/missing_runtime_args.yaml`
- [x] `tests/policy_invalid_cases/missing_mounts.yaml`
- [x] `tests/policy_invalid_cases/invalid_cpu_format.yaml`
- [x] `tests/policy_invalid_cases/invalid_memory_format.yaml`
- [x] `tests/compile_snapshot/expected_bundle.json`
- [x] 编译确定性测试（同输入同输出）
- [x] CLI 边界测试（`compile` 未带 `--dry-run` 失败）

## 7. 待确认项

- 无
