# M0 技术选型与参考资料

## 1. 技术选型

- 语言：Rust（默认禁用 unsafe）。
- 配置：`serde` + `serde_yaml` + `serde_json`。
- CLI：`clap`。
- 日志：`tracing`。
- 测试：`cargo test` + 快照测试框架。

## 2. 选型理由

- Rust 可在低级系统编排场景控制内存安全风险。
- 结构化序列化便于后续证据链落盘与回放。
- 统一 CLI 与日志栈降低后续运维复杂度。

## 3. 参考资料（本阶段）

- Firecracker API 配置模型（用于 M1 前置理解）。
- Linux cgroup v2 控制项文档。
- 项目草案：`DRAFT.md`。
