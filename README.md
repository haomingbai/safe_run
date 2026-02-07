# Safe-Run Suite (M1)

M1 目标：完成最小执行闭环 `Policy -> Validate -> Compile -> Run -> Report`，默认无网络。

## 工程结构

- `crates/sr-common`: 共享错误项与错误码常量
- `crates/sr-policy`: `PolicySpec` 解析与 `ValidationResult` 校验
- `crates/sr-compiler`: `CompileBundle` 生成（M1 可执行配置）
- `crates/sr-runner`: `prepare/launch/monitor/cleanup` 执行编排
- `crates/sr-evidence`: 事件链、hash 与 `run_report.json` 生成
- `crates/sr-cli`: `safe-run` CLI（`validate` / `compile --dry-run` / `run`）
- `tests/policy_valid_cases`: 合法策略样例
- `tests/policy_invalid_cases`: 非法策略样例
- `tests/compile_snapshot`: 编译输出快照样例
- `crates/sr-runner/tests`: M1 集成测试（`run_smoke` / `run_failure_paths` / `report_schema_v1`）
- `examples/`: M1 示例策略（无网/只读根/资源限制）

## 本地命令

```bash
cargo run -p sr-cli -- validate tests/policy_valid_cases/minimal.yaml
cargo run -p sr-cli -- compile --dry-run --policy tests/compile_snapshot/minimal_policy.yaml
cargo run -p sr-cli -- run --policy examples/m1_network_none.yaml
cargo test
```

## 阶段化用法（M0/M1）

- M0 最小链路：`validate` + `compile --dry-run`；`compile` 不带 `--dry-run` 应视为非法调用。
- M1 在 M0 语义不变基础上新增 `run`；`validate` 与 `compile --dry-run` 行为必须保持兼容。
- 验收与回归时建议同时覆盖：
  - M0 清单：`M0_CHECKLIST.md`
  - M1 清单：`M1_CHECKLIST.md`

## 策略编写注意（来自 M0/M1 checklist）

- `runtime.args` 与 `mounts` 必须显式提供（可为空数组，但不可省略字段）。
- M0-M2 阶段仅允许 `network.mode=none`。
- M1 阶段 `CompileBundle.networkPlan` 固定为 `null`，不要为 allowlist 预置可执行逻辑。

## Firecracker/Jailer 本地自举（不依赖系统安装）

按 Firecracker 官方 getting-started 的 release 二进制方式，可直接下载到仓库本地：

```bash
./scripts/get_firecracker.sh
export PATH="$(pwd)/artifacts/bin:$PATH"
```

默认会在未设置代理环境变量时优先使用本地代理 `http://127.0.0.1:7890`。可通过以下环境变量调整：

```bash
SAFE_RUN_USE_LOCAL_PROXY=0 ./scripts/get_firecracker.sh
SAFE_RUN_PROXY_URL="http://127.0.0.1:7891" ./scripts/get_firecracker.sh
```

指定版本示例：

```bash
./scripts/get_firecracker.sh v1.13.1
export PATH="$(pwd)/artifacts/bin:$PATH"
```

脚本会把 `firecracker` 和 `jailer` 安装到 `artifacts/bin/`，便于 AGENTS 和开发者在无系统级安装的环境中运行。

## Rootfs 与内核产物

为 M1 运行准备 Firecracker 所需的 kernel/rootfs，可使用脚本：

```bash
./scripts/get_rootfs.sh
```

脚本会把 `vmlinux` 与 `rootfs.ext4` 放入 `artifacts/`（已加入 .gitignore）。
依赖工具：`curl`、`wget`、`unsquashfs`（squashfs-tools）、`mkfs.ext4`（e2fsprogs）。
`get_rootfs.sh` 与 `get_firecracker.sh` 使用同一组代理环境变量：`SAFE_RUN_USE_LOCAL_PROXY`、`SAFE_RUN_PROXY_URL`。

## M1 运行前置条件

- 需要可用的 `firecracker` 与 `jailer` 可执行文件（推荐使用 `./scripts/get_firecracker.sh` 下载本地版本）
- 需要可写运行目录（默认 `/tmp/safe-run/runs`）
- Firecracker API socket 需要可写路径（当前实现会写到 `<run_workdir>/artifacts/firecracker.socket`）
- 如需自定义运行目录，可设置环境变量：

```bash
export SAFE_RUN_WORKDIR_BASE=/your/writable/path
```

## M1 运行常见问题与排查

- 错误：`SR-RUN-002` + `path=launch.preflight.jailer`
  - 含义：未找到 `jailer` 可执行文件或不可执行。
  - 处理：安装 `jailer` 或将其加入 `PATH`；仅本地验证可使用兼容脚本（如 `/tmp/jailer`），生产环境应使用真实 jailer。
- 错误：`SR-RUN-002` + `path=launch.preflight.firecracker`
  - 含义：未找到 `firecracker` 可执行文件或不可执行。
  - 处理：安装 Firecracker 并确认执行权限。
- 错误：`Operation not permitted`（常见于受限沙箱）
  - 含义：当前运行环境不允许 KVM/相关系统调用或 socket 绑定。
  - 处理：在具备权限的目标 Linux 主机上执行真实 `run` 验收，并在记录中注明运行环境（沙箱内/外）。

## M1 能力边界

- 仅支持 `network.mode=none`
- `networkPlan` 固定为 `null`
- 不实现 M2 挂载安全强化与 M3 allowlist 生效

## M1 验收与错误码

- 验收清单：`M1_CHECKLIST.md`
- 策略错误码：`SR-POL-001`（缺少必填字段）、`SR-POL-002`（字段格式错误）、`SR-POL-003`（策略语义冲突）
- 编译错误码：`SR-CMP-001`（编译模板映射失败）、`SR-CMP-002`（编译输出不完整或非法请求）
- 运行错误码：`SR-RUN-001`（Runner 初始化失败）、`SR-RUN-002`（VM 启动失败）、`SR-RUN-003`（执行超时）
- 证据错误码：`SR-EVD-001`（事件写入失败）、`SR-EVD-002`（报告生成失败）
