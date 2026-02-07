# Safe-Run Suite (M0)

M0 目标：完成策略解析 -> 校验 -> dry-run 编译闭环，不进入真实 microVM 执行。

## 工程结构

- `crates/sr-common`: 共享错误项与错误码常量
- `crates/sr-policy`: `PolicySpec` 解析与 `ValidationResult` 校验
- `crates/sr-compiler`: dry-run `CompileBundle` 生成
- `crates/sr-cli`: `safe-run` CLI（`validate` / `compile --dry-run`）
- `tests/policy_valid_cases`: 合法策略样例
- `tests/policy_invalid_cases`: 非法策略样例
- `tests/compile_snapshot`: 编译输出快照样例

## 本地命令

```bash
cargo run -p sr-cli -- validate tests/policy_valid_cases/minimal.yaml
cargo run -p sr-cli -- compile --dry-run --policy tests/compile_snapshot/minimal_policy.yaml
cargo test
```

## Rootfs 与内核产物

为 M1 运行准备 Firecracker 所需的 kernel/rootfs，可使用脚本：

```bash
./scripts/get_rootfs.sh
```

脚本会把 `vmlinux` 与 `rootfs.ext4` 放入 `artifacts/`（已加入 .gitignore）。
依赖工具：`curl`、`wget`、`unsquashfs`（squashfs-tools）、`mkfs.ext4`（e2fsprogs）。

## M0 能力边界

- 仅支持 `network.mode=none`
- `compile` 仅支持 `--dry-run`
- 输出 `CompileBundle.networkPlan = null`
- 不调用 jailer/Firecracker，不执行任何 host 变更

## M0 验收与错误码

- 验收清单：`M0_CHECKLIST.md`
- 策略错误码：`SR-POL-001`（缺少必填字段）、`SR-POL-002`（字段格式错误）、`SR-POL-003`（策略语义冲突）
- 编译错误码：`SR-CMP-001`（编译模板映射失败）、`SR-CMP-002`（编译输出不完整或非法请求）
