# M0 阶段总览：基础打底与统一接口定稿

## 1. 阶段目标

M0 目标是搭建最小可运行框架，完成“策略解析 -> 校验 -> dry-run 编译 -> 结构化输出”闭环，不进入真实 microVM 执行。

## 2. 交付代码

- `crates/sr-policy`：`PolicySpec` v1alpha1 解析与基础校验。
- `crates/sr-compiler`：`CompileBundle` dry-run 生成。
- `crates/sr-cli`：
  - `safe-run validate policy.yaml`
  - `safe-run compile --dry-run --policy policy.yaml`
- `tests/`：策略合法/非法样例与快照测试。

## 3. 接口交付

- 发布并冻结（语义冻结）接口：
  - `I-PL-001`
  - `I-VA-001`
  - `I-CP-001`
- 预留但不启用：`I-RN-001`、`I-EV-001`、`I-RP-001`。

## 4. 架构边界

- 仅允许到“编译计划层”，禁止调用 jailer/Firecracker。
- 输出均为可序列化 JSON/YAML，便于 M1 直接接入 Runner。

## 5. 验收标准

- 能稳定解析并校验策略，输出明确错误码（`SR-POL-*`）。
- dry-run 可生成确定性 `CompileBundle`（同输入同输出）。
- 文档与字段定义与 `plan/INTERFACE_BASELINE.md` 完全一致。
