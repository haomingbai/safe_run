# Safe-Run Suite 开发总体计划（M0-M4）

## 1. 目标与范围

本计划基于 `DRAFT.md`，将项目拆分为五个阶段（M0-M4），确保以下三条主线同步推进：

1. 策略驱动：将高层安全策略编译为可执行且可证明的 MicroVM/Host 约束。
2. 安全编排：以最小特权方式完成 microVM 生命周期管理与宿主控制。
3. 审计证据链：形成可追溯、可验证、可归档的运行证据。

非目标（本期不承诺）：

- 不承诺抵御未知 VM escape 0day。
- 不做 Kubernetes/CNI 全套集成。
- 不 fork 或改造 VMM 内核实现。

## 2. 统一模块基线（全阶段不变）

统一模块边界如下，阶段内只增量扩展能力，不改变模块职责：

- `sr-policy`：策略 schema、解析、语义校验、组合约束。
- `sr-compiler`：策略转译为 Firecracker 配置与 Host 执行计划。
- `sr-runner`：jailer/Firecracker 生命周期编排、状态采样、清理。
- `sr-evidence`：artifact hash、事件时间线、报告生成与验证。
- `sr-cli`：命令行入口、参数校验、错误码与用户交互。
- `sr-ops`：打包、发布、运维脚本、演示与验收工具。

## 3. 统一接口基线（全阶段不冲突）

统一接口 ID 和版本策略：

- `I-PL-001` PolicySpec（`policy.safe-run.dev/v1alpha1`）
- `I-VA-001` ValidationResult（JSON 输出）
- `I-CP-001` CompileBundle（内部结构化输出）
- `I-RN-001` RunnerControl（运行编排请求/响应）
- `I-EV-001` EvidenceEvent（事件模型）
- `I-RP-001` RunReport（`safe-run.report/v1`）
- `I-VF-001` ReportVerify（报告校验接口）

兼容规则：

- 只允许“新增字段（additive change）”，禁止删除/重命名已发布字段。
- `PolicySpec` 与 `RunReport` 使用显式 `apiVersion/schemaVersion`。
- 阶段升级不改变已交付命令行为语义（除非在文档中显式标记破坏性变更并给迁移方案）。

## 4. 阶段总览

### M0（基础打底）

- 目标：建立可运行骨架、统一接口、策略校验与 dry-run 编译链。
- 关键交付：`safe-run validate`、`safe-run compile --dry-run`、初版 schema 与测试框架。

### M1（核心执行闭环）

- 目标：跑通“策略 -> microVM 执行 -> 报告输出”最小闭环（默认无网络）。
- 关键交付：`safe-run run`、最小 Runner、`run_report.json` v1。

### M2（挂载与路径安全）

- 目标：强化路径 canonicalization、挂载安全与组合约束。
- 关键交付：挂载策略增强、只读根与最小挂载策略完善、攻击面回归测试。

### M3（网络白名单控制）

- 目标：引入 nftables/TAP 方案，实现“默认拒绝 + 显式放行”。
- 关键交付：网络策略编译与应用、网络事件入证据链、网络回归测试矩阵。

### M4（交付打磨与合规化）

- 目标：形成可申报、可验收、可运维交付包。
- 关键交付：`safe-run report verify`、EvidenceChain 工具化、完整文档与演示。

## 5. 里程碑与验收口径

- 里程碑 A（M1 完成）：具备软著主程序初步申报材料。
- 里程碑 B（M3 完成）：具备专利方法/系统撰写素材（流程、结构、效果对比）。
- 里程碑 C（M4 完成）：可形成主软著 + 可选证据链模块软著。

统一验收维度：

- 功能：命令、接口、输出格式按文档一致。
- 安全：默认拒绝、最小权限、危险组合拦截。
- 可追溯：报告字段完整、hash 可复算、事件可追踪。
- 可运维：日志、错误码、排障与演示脚本可用。

## 6. 文档索引

- 全局：`plan/OVERVIEW.md`、`plan/INTERFACE_BASELINE.md`
- 阶段：`plan/M0-M4/OVERVIEW.md` + 各阶段详细设计文档
