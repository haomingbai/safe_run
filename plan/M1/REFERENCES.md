# M1 技术选型与参考资料

## 1. 技术选型

- VMM：Firecracker（首选）。
- 隔离：jailer。
- 资源控制：cgroup v2。
- 报告：JSON + SHA-256 完整性摘要。

## 2. 选型理由

- Firecracker/jailer 组合可快速达成最小攻击面执行环境。
- cgroup v2 便于统一 CPU/内存采样口径。
- JSON 报告与 hash 摘要易于后续审计和合规留存。

## 3. 参考资料（本阶段）

- Firecracker 快速启动与 API 使用文档。
- jailer 使用说明与权限模型。
- Linux cgroup v2 指标读取文档。
- 项目草案：`DRAFT.md`。
