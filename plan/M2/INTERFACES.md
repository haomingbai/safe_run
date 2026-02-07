# M2 接口设计

## 1. I-PL-001 mounts 语义增强（字段不变）

示例：

```yaml
mounts:
  - source: /var/lib/safe-run/input
    target: /data/input
    read_only: true
```

字段命名与兼容：

- **规范字段名（canonical）**：`source` / `target` / `read_only`。
- **兼容输入别名（additive）**：允许将 `hostPath` 解析为 `source`、`guestPath` 解析为 `target`、`readOnly` 解析为 `read_only`。
- `ValidationResult.normalizedPolicy` 与编译产物中应使用规范字段名输出，便于后续阶段统一与 hash 可复算。

新增强约束：

- `hostPath` 必须 canonicalize 后匹配白名单前缀。
- 禁止挂载宿主敏感路径（如 `/proc`, `/sys`, `/dev` 默认拒绝）。
- `guestPath` 必须位于允许命名空间，禁止覆盖关键系统路径。

读写策略（M2 定稿）：

- M2 阶段将“可写挂载 + 高权限执行”视为不可接受的风险组合；由于当前 `PolicySpec` 未引入可表达“权限/用户身份/能力集”的字段，M2 采取最小且不歧义的规则：
  - **所有挂载必须为只读**：若 `read_only: false`（或 `readOnly: false`），应拒绝并返回 `SR-POL-103`。
- 后续若要支持有限可写挂载，必须先在 `plan/` 中新增可证明的权限模型字段（additive），再放开该限制。

## 2. I-EV-001 新增事件类型

- `mount.validated`
- `mount.rejected`
- `mount.applied`

## 3. I-RP-001 additive 字段

```json
{
  "mountAudit": {
    "requested": 2,
    "accepted": 1,
    "rejected": 1,
    "reasons": ["path_outside_allowlist"]
  }
}
```

## 4. 错误码扩展

- `SR-POL-101`：挂载路径不在白名单
- `SR-POL-102`：挂载目标路径非法
- `SR-POL-103`：挂载策略组合冲突
- `SR-RUN-101`：挂载应用失败
