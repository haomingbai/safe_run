# Safe-Run Suite (M0-M2)

M0-M2 目标：在最小执行闭环 `Policy -> Validate -> Compile -> Run -> Report` 上，完成挂载与路径安全强化，且默认无网络。

## 工程结构

- `crates/sr-common`: 共享错误项与错误码常量
- `crates/sr-policy`: `PolicySpec` 解析与 `ValidationResult` 校验
- `crates/sr-compiler`: `CompileBundle` 生成（M0-M2 可执行配置）
- `crates/sr-runner`: `prepare/launch/monitor/cleanup` 执行编排
- `crates/sr-evidence`: 事件链、hash 与 `run_report.json` 生成
- `crates/sr-cli`: `safe-run` CLI（`validate` / `compile --dry-run` / `run`）
- `tests/policy_valid_cases`: 合法策略样例
- `tests/policy_invalid_cases`: 非法策略样例
- `tests/compile_snapshot`: 编译输出快照样例
- `crates/sr-runner/tests`: M0-M2 集成测试（`run_smoke` / `run_failure_paths` / `report_schema_v1`）
- `examples/`: M0-M2 示例策略（无网/只读根/资源限制/挂载只读）

## 本地命令

```bash
cargo run -p sr-cli -- validate tests/policy_valid_cases/minimal.yaml
cargo run -p sr-cli -- compile --dry-run --policy tests/compile_snapshot/minimal_policy.yaml
cargo run -p sr-cli -- run --policy examples/m1_network_none.yaml
cargo run -p sr-cli -- validate examples/m2_mount_readonly.yaml
cargo test
```

## 阶段化用法（M0-M2）

- M0 最小链路：`validate` + `compile --dry-run`；`compile` 不带 `--dry-run` 应视为非法调用。
- M1 在 M0 语义不变基础上新增 `run`；`validate` 与 `compile --dry-run` 行为必须保持兼容。
- M2 在 M1 基础上新增挂载与路径安全约束（`mounts[].read_only=true`、allowlist/canonicalize、挂载审计事件）。
- 验收与回归时建议同时覆盖：
  - M0 清单：`M0_CHECKLIST.md`
  - M1 清单：`M1_CHECKLIST.md`
  - M2 计划与验收项：`M2_STAGE_PLAN.md`

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

## 运行前置条件（M1-M2）

- 需要可用的 `firecracker` 与 `jailer` 可执行文件（推荐使用 `./scripts/get_firecracker.sh` 下载本地版本）
- 需要可写运行目录（默认 `/tmp/safe-run/runs`）
- Firecracker API socket 需要可写路径（当前实现会写到 `<run_workdir>/artifacts/firecracker.socket`）
- 如需自定义运行目录，可设置环境变量：

```bash
export SAFE_RUN_WORKDIR_BASE=/your/writable/path
```

## 运行常见问题与排查（M1-M2）

- 错误：`SR-RUN-002` + `path=launch.preflight.jailer`
  - 含义：未找到 `jailer` 可执行文件或不可执行。
  - 处理：安装 `jailer` 或将其加入 `PATH`；仅本地验证可使用兼容脚本（如 `/tmp/jailer`），生产环境应使用真实 jailer。
- 错误：`SR-RUN-002` + `path=launch.preflight.firecracker`
  - 含义：未找到 `firecracker` 可执行文件或不可执行。
  - 处理：安装 Firecracker 并确认执行权限。
- 错误：`Operation not permitted`（常见于受限沙箱）
  - 含义：当前运行环境不允许 KVM/相关系统调用或 socket 绑定。
  - 处理：在具备权限的目标 Linux 主机上执行真实 `run` 验收，并在记录中注明运行环境（沙箱内/外）。

## M0-M2 能力边界

- 仅支持 `network.mode=none`
- `networkPlan` 固定为 `null`
- 不支持 `network.mode=allowlist` 的真实生效（M3 才启用）

## 示例命名约定

- `examples/m1_*.yaml`：M1 最小执行闭环示例。
- `examples/m2_*.yaml`：M2 挂载与路径安全示例。

## 文档回链

- 总体设计：`plan/OVERVIEW.md`
- 接口基线：`plan/INTERFACE_BASELINE.md`
- M2 设计：`plan/M2/OVERVIEW.md`、`plan/M2/INTERFACES.md`、`plan/M2/MOUNT_CLARIFICATIONS.md`
- 工程规范：`plan/ENGINEERING_CONVENTIONS.md`

## 网络 allowlist 验收闭环（具备权限 Linux 主机）

> 依据：`plan/M3/NETWORK_CLARIFICATIONS.md`（对应 M3 可选验收项，真实 TAP+nft+NAT/路由+guest IP 配置闭环在具备权限环境执行）。

### 环境与依赖命令

- Linux 主机（仅 Linux）
- Root 权限（`id -u` 为 `0`）或可用 `sudo`
- Linux 网络工具：`ip`、`nft`、`sysctl`
- 验收命令依赖：`curl`、`cargo`
- Safe-Run 运行所需：`firecracker`、`jailer`（推荐 `./scripts/get_firecracker.sh` 后 `export PATH="$(pwd)/artifacts/bin:$PATH"`）

建议先确认：

```bash
id -u
command -v ip nft sysctl firecracker jailer
```

### 一键验收脚本（推荐）

已提供脚本：`scripts/network_allowlist_acceptance.sh`

执行：

```bash
./scripts/network_allowlist_acceptance.sh
```

脚本行为：

1. 在隔离 `ip netns` 中构建网络 allowlist 验收拓扑（TAP + bridge + veth + NAT/route + guest IP）；
2. 在 netns 内下发 probe 专用 `output` 过滤规则（仅用于稳定复现“允许/阻断”探针，不替代 runner 规则验收）；
3. 注入环境变量并运行 `#[ignore]` 用例：`stage6_real_network_allowlist_closure`（由 runner 在宿主机 `safe_run/forward` 下发规则）；
4. 使用审计探针校验 `safe_run/forward` 命中计数；
5. 自动清理临时命名空间/网卡/`nft` 表（含 NAT 表）。

### `#[ignore]` 集成测试（默认不跑）

已新增：`crates/sr-runner/tests/network_stage6_real_world.rs`

该测试通过环境变量注入“手工验收命令”，用于验证以下闭环：

1. allowlist 目标可达；
2. 非 allowlist 目标不可达；
3. 命中可审计；
4. 异常或结束后规则可回收。

必须设置的环境变量：

- `SAFE_RUN_STAGE6_SETUP_CMD`：准备并启动测试场景（TAP+nft+NAT/路由+guest IP 配置）
- `SAFE_RUN_STAGE6_ALLOWED_PROBE_CMD`：allowlist 目标探测（应成功）
- `SAFE_RUN_STAGE6_BLOCKED_PROBE_CMD`：非 allowlist 目标探测（应失败）
- `SAFE_RUN_STAGE6_AUDIT_PROBE_CMD`：命中审计探测（应成功）
- `SAFE_RUN_STAGE6_CLEANUP_PROBE_CMD`：清理结果探测（应成功，确认无残留）

可选环境变量：

- `SAFE_RUN_STAGE6_CLEANUP_CMD`：测试结束后统一清理命令（`Drop` 钩子执行）

执行命令：

```bash
cargo test -p sr-runner stage6_real_network_allowlist_closure -- --ignored --nocapture
```

> 注意：该测试不会替你生成具体网络拓扑；它执行你提供的命令并断言结果，适配不同 Linux 主机网络环境。

当前仓库默认推荐直接使用 `scripts/network_allowlist_acceptance.sh`，它已固化可复现拓扑与清理逻辑。

## 验收与错误码

- 验收清单：`M1_CHECKLIST.md`
- 策略错误码：`SR-POL-001`（缺少必填字段）、`SR-POL-002`（字段格式错误）、`SR-POL-003`（策略语义冲突）
- 编译错误码：`SR-CMP-001`（编译模板映射失败）、`SR-CMP-002`（编译输出不完整或非法请求）
- 运行错误码：`SR-RUN-001`（Runner 初始化失败）、`SR-RUN-002`（VM 启动失败）、`SR-RUN-003`（执行超时）
- 证据错误码：`SR-EVD-001`（事件写入失败）、`SR-EVD-002`（报告生成失败）
