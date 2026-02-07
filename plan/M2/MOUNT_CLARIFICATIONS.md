# M2 挂载安全规则澄清（供实现与后续阶段复用）

## 1. 目的

本文件用于消除 M2 阶段关于 `mounts` 的歧义点，确保测试、实现与后续阶段（M3/M4）扩展不冲突。

适用范围：M2（挂载与路径安全强化）。

## 2. 字段命名与兼容

- 规范字段名（canonical）：`source` / `target` / `read_only`。
- 兼容输入别名（additive）：
  - `hostPath -> source`
  - `guestPath -> target`
  - `readOnly -> read_only`
- 规范化输出：`ValidationResult.normalizedPolicy` 与编译产物使用 canonical 字段名。

## 3. Allowlist 配置文件（宿主路径白名单）

### 3.1 来源优先级

1. CLI 参数：`--mount-allowlist <path>`
2. 环境变量：`SAFE_RUN_MOUNT_ALLOWLIST=<path>`
3. 未提供时：内置默认 allowlist

### 3.2 YAML 格式（v1）

```yaml
schemaVersion: safe-run.mount-allowlist/v1
hostAllowPrefixes:
  - /var/lib/safe-run
guestAllowPrefixes:
  - /data
```

约束：

- `hostAllowPrefixes[]` 与 `guestAllowPrefixes[]` 都必须是绝对路径。
- 若文件缺失/解析失败/字段不合法，应在验证阶段返回 `SR-POL-101`（或对应字段错误码），并在 `path` 中标明问题字段。

### 3.3 内置默认值

为与现有示例保持一致，M2 内置默认：

- `hostAllowPrefixes = ['/var/lib/safe-run']`
- `guestAllowPrefixes = ['/data']`

## 4. target 命名空间与关键路径禁止覆盖

- `target` 必须匹配 `guestAllowPrefixes` 之一（按路径组件前缀匹配）。
- 以下关键路径不得作为 `target`，也不得被覆盖（`target` 不能等于这些路径，也不能落在其子路径内）：
  - `/`, `/proc`, `/sys`, `/dev`, `/run`, `/boot`, `/etc`, `/bin`, `/sbin`, `/lib`, `/lib64`, `/usr`
- 该 denylist 属于安全不变量：M2 仅允许扩展 allowlist，不允许通过配置绕开 denylist。

## 5. 可写挂载（read_only=false）的处理

### 5.1 设计依据

M2 验收标准包含“风险组合（可写挂载 + 高权限执行）被拒绝并输出明确错误码”（见 `plan/M2/OVERVIEW.md`）。

### 5.2 M2 定稿规则

- 由于当前 `PolicySpec` 未引入可表达“权限/用户身份/能力集”的字段，无法在接口层无歧义地区分“高权限执行”与“低权限执行”。
- 为保证规则可执行且不与后续阶段冲突，M2 采取最小落地：
  - 若存在 `read_only == false`（或 `readOnly == false`），按挂载策略组合冲突拒绝，并返回 `SR-POL-103`。

### 5.3 后续阶段扩展指引（非实现承诺）

若需要支持有限可写挂载，应先在 `plan/` 中新增可证明的权限模型字段（additive），并定义其与 `mounts[].read_only` 的组合语义，再放开该限制。

