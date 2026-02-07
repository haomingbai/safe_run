# 全局接口与版本基线（M0-M4 统一约束）

## 1. 设计原则

- 统一语义：接口名称、字段语义、错误码在各阶段保持一致。
- 向后兼容：仅允许新增字段，不允许删除/重命名已发布字段。
- 可审计：关键输入输出都可被序列化、存档、校验。

## 2. 外部接口

### 2.1 I-PL-001 PolicySpec

- 类型：YAML/JSON
- 版本：`apiVersion: policy.safe-run.dev/v1alpha1`
- 最小字段：
  - `metadata.name`
  - `runtime.command` / `runtime.args`
  - `resources.cpu.max` / `resources.memory.max`
  - `network.mode`（`none|allowlist`，M0-M2 仅支持 `none`）
  - `mounts[]`（M2 起强化）
  - `audit.level`

`mounts[]` 字段命名约定（与现有实现对齐）：

- 规范字段名：`source` / `target` / `read_only`。
- 兼容输入别名（additive，便于文档迁移）：`hostPath -> source`、`guestPath -> target`、`readOnly -> read_only`。

### 2.2 I-RP-001 RunReport

- 类型：JSON
- 版本：`schemaVersion: safe-run.report/v1`
- 最小字段：
  - `runId`, `startedAt`, `finishedAt`, `exitCode`
  - `artifacts.{kernelHash,rootfsHash,policyHash,commandHash}`
  - `policySummary`
  - `resourceUsage`
  - `events[]`
  - `integrity.digest`

### 2.4 JSON 规范化与 SHA-256 口径（全阶段统一）

为保证 `policyHash`、`commandHash` 与 `integrity.digest` 可复算，所有 JSON 参与 SHA-256 计算前必须先进行规范化：

- JSON 必须转换为 UTF-8 字符串。
- 对象（object）字段必须按字典序排序。
- 数组（array）保持原有顺序。
- 序列化时不得包含空格、换行等额外空白字符。

规范化后，使用 SHA-256 计算并输出为 `sha256:<hex>`。

### 2.3 I-VF-001 ReportVerify

- 调用：`safe-run report verify <run_report.json>`
- 返回：`valid|invalid` + 失败原因列表。

## 3. 内部接口

### 3.1 I-VA-001 ValidationResult

- 输入：`PolicySpec`
- 输出：
  - `valid: bool`
  - `errors[]`（阻断）
  - `warnings[]`（非阻断）
  - `normalizedPolicy`

### 3.2 I-CP-001 CompileBundle

- 输入：`ValidationResult.normalizedPolicy`
- 输出：
  - `firecrackerConfig`
  - `jailerPlan`
  - `cgroupPlan`
  - `networkPlan`（M3 起非空）
  - `evidencePlan`

### 3.3 I-RN-001 RunnerControl

- 输入：`CompileBundle` + runtime context
- 输出：
  - `runId`
  - `state`（`prepared|running|finished|failed`）
  - `artifacts` 路径
  - `eventStream`

### 3.4 I-EV-001 EvidenceEvent

- 通用字段：`timestamp`, `runId`, `stage`, `type`, `payload`, `hashPrev`, `hashSelf`
- 阶段扩展：
  - M1：启动/退出/资源采样
  - M2：挂载决策与路径校验事件
  - M3：网络规则与命中事件
  - M4：报告验证与归档事件

## 4. 错误码基线

- `SR-POL-*`：策略/校验错误
- `SR-CMP-*`：编译错误
- `SR-RUN-*`：运行时编排错误
- `SR-EVD-*`：证据链生成/验证错误
- `SR-OPS-*`：运维与环境错误

## 5. 阶段升级规则

- M0 -> M1：可新增 `RunReport` 字段，不可改已有字段语义。
- M1 -> M2：`PolicySpec.mounts` 语义从“基本校验”升级为“安全校验”，保持字段名不变。
- M2 -> M3：`PolicySpec.network.mode=allowlist` 从“不支持”变为“支持”，保持 schema 不变。
- M3 -> M4：新增 `report verify` 与归档字段，`RunReport` 保持 v1，仅做增量字段扩展。
