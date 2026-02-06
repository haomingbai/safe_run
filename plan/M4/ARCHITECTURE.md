# M4 架构设计

## 1. 交付态架构

- 执行主链：`policy -> compiler -> runner -> evidence`。
- 校验支链：`report -> verifier -> archive`。
- 运维支链：`packaging -> deployment -> diagnostics`。

## 2. 关键流程

1. 执行阶段产出 `run_report.json`。
2. 校验阶段复算 hash 与事件链。
3. 归档阶段写入 bundle 并记录元数据。
4. 检索阶段按 `runId/bundleId` 查询。

## 3. 发布与稳定性策略

- 接口冻结：仅允许 additive 更新。
- 测试门禁：单元、集成、回归、验收脚本全部通过才发布。
- 可运维性：关键故障路径必须有日志与可定位建议。

## 4. 合规输出映射

- 对软著：提供模块说明、命令截图、代码目录与功能闭环。
- 对专利：提供方法流程图、系统结构图、技术效果验证数据。
