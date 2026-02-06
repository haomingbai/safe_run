# M3 技术选型与参考资料

## 1. 技术选型

- Host 网络控制：`nftables`。
- VM 出口连接：TAP 设备。
- 规则生命周期：按 `runId` 管理资源命名与回收。

## 2. 选型理由

- nftables 规则模型清晰，便于程序化生成和回收。
- TAP 方案与 MicroVM 网络模型兼容性较好。
- runId 绑定可降低资源泄漏和污染风险。

## 3. 参考资料（本阶段）

- nftables 规则模型与命令语义文档。
- Firecracker 网络设备配置资料。
- 项目草案：`DRAFT.md`。
