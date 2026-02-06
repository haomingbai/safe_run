# M1 架构设计

## 1. 运行时架构

- 控制面：`sr-cli` + `sr-runner`。
- 执行面：jailer + Firecracker microVM。
- 证据面：`sr-evidence` 事件管线 + 报告聚合。

## 2. 时序流程

1. CLI 读取策略并调用 `validate` / `compile`。
2. Runner 准备工作目录与 jailer 参数。
3. 启动 Firecracker，注入 boot/config。
4. 周期采样 cgroup 资源数据。
5. VM 退出后执行清理并汇总报告。

## 3. 安全决策

- 默认 `network=none`。
- 根文件系统只读优先（可在策略显式申请写层并严格限制）。
- 高权限操作集中在受控执行点，业务逻辑保持低权限。

## 4. 可观测性要求

- 所有状态迁移必须产生事件。
- 关键异常（启动失败、配置不一致、清理失败）必须有独立事件与错误码。
