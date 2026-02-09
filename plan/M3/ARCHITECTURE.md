# M3 架构设计

## 1. 新增网络控制面

- `NetworkPolicyEngine`：规则语义校验与标准化。
- `NetworkPlanBuilder`：将策略编译为 nftables/TAP 计划。
- `NetworkLifecycleManager`：规则创建、绑定、释放。

## 2. 时序流程

1. `sr-policy` 校验 allowlist。
2. `sr-compiler` 生成 `networkPlan`。
3. `sr-runner` 在 VM 启动前应用规则。
4. 运行中采集规则命中事件。
5. 退出后清理网络状态并记录结果。

## 3. 安全策略

- 默认拒绝所有外连。
- 仅按显式 allowlist 开放流量。
- 网络控制与 runId 强绑定，避免跨运行污染。
- nftables 规则默认挂载在 `forward` 链，并按 TAP 设备与 runId 做隔离（细则见 `plan/M3/NETWORK_CLARIFICATIONS.md`）。

## 4. 运维要求

- 规则下发、命中、清理均要可观测。
- 清理失败要产生高优先级告警事件。
