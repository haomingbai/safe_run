# M0 架构设计

## 1. 目标架构（无执行面）

- CLI 层：参数处理与结果渲染。
- 应用层：`ValidateService`、`CompileService`。
- 领域层：Policy AST、Constraint Engine、Plan Model。
- 基础设施层：序列化/反序列化、配置加载、日志。

## 2. 数据流

1. `safe-run validate` 读取 `policy.yaml`。
2. `sr-policy` 生成 AST 并执行规则校验。
3. 输出 `ValidationResult`。
4. `safe-run compile --dry-run` 调用 `sr-compiler` 生成 `CompileBundle`。

## 3. 关键设计决策

- 决策 1：先冻结数据模型，再进入执行层，降低 M1 返工。
- 决策 2：编译结果采用结构化模型，不拼接 shell 文本。
- 决策 3：统一错误码命名空间，提前打通后续运维可观测性。

## 4. 风险与缓解

- 风险：策略字段定义过早锁死。
- 缓解：通过 `v1alpha1` + additive 规则保留扩展能力。
