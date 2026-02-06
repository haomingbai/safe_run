# M1 阶段总览：安全执行最小闭环

## 1. 阶段目标

在 M0 接口基线上实现最小可用执行链：
`Policy -> Validate -> Compile -> Run -> Report`，默认无网络。

## 2. 交付代码

- `crates/sr-runner`：jailer + Firecracker 生命周期编排。
- `crates/sr-evidence`：基础事件记录与 `RunReport` 生成。
- `crates/sr-cli`：`safe-run run --policy policy.yaml`。
- `examples/`：无网执行、只读根、资源限制三类示例。

## 3. 接口交付

- 启用接口：
  - `I-RN-001` RunnerControl
  - `I-EV-001` EvidenceEvent（基础事件）
  - `I-RP-001` RunReport v1（基础字段）
- 兼容要求：
  - `I-PL-001`、`I-VA-001`、`I-CP-001` 语义保持不变。

## 4. 架构边界

- 网络固定 `none`。
- Runner 必须支持失败清理和状态回收。
- 报告必须可复算 artifact hash。

## 5. 验收标准

- 能在目标 Linux 主机稳定执行一个不可信任务并退出。
- 生成 `run_report.json` 且字段符合 `safe-run.report/v1`。
- 失败路径（启动失败/超时/异常退出）均可留痕并返回标准错误码。
