# M2 接口设计

## 1. I-PL-001 mounts 语义增强（字段不变）

示例：

```yaml
mounts:
  - hostPath: /opt/safe-run/data
    guestPath: /data
    readOnly: true
```

新增强约束：

- `hostPath` 必须 canonicalize 后匹配白名单前缀。
- 禁止挂载宿主敏感路径（如 `/proc`, `/sys`, `/dev` 默认拒绝）。
- `guestPath` 必须位于允许命名空间，禁止覆盖关键系统路径。

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
