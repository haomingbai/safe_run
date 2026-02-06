# M3 阶段总览：网络白名单控制

## 1. 阶段目标

在保持默认无网的前提下，支持显式网络白名单（`network.mode=allowlist`），实现可控联网与网络审计。

## 2. 交付代码

- `sr-policy`：网络 allowlist 规则校验。
- `sr-compiler`：`networkPlan` 生成（nftables + TAP）。
- `sr-runner`：网络规则应用、恢复与异常清理。
- `sr-evidence`：网络事件采集与报告落盘。

## 3. 接口交付

- `I-PL-001.network`：从“仅 `none`”升级为支持 `allowlist`。
- `I-CP-001.networkPlan`：从 `null` 升级为结构化计划。
- `I-EV-001`：新增网络规则与命中事件。
- `I-RP-001`：新增网络审计字段（additive）。

## 4. 架构边界

- 仍坚持 deny-by-default：不配置 allowlist 即无网络。
- 网络规则必须随 run 生命周期创建与销毁，禁止残留。

## 5. 验收标准

- allowlist 外流量被拒绝，规则命中可审计。
- VM 异常退出后网络规则能自动回收。
- 报告可完整还原“允许了什么、拦截了什么”。
