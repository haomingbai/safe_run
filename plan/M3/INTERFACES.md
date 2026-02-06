# M3 接口设计

## 1. I-PL-001 network 扩展（字段兼容）

示例：

```yaml
network:
  mode: allowlist
  egress:
    - protocol: tcp
      host: api.example.com
      port: 443
    - protocol: udp
      cidr: 1.1.1.1/32
      port: 53
```

约束：

- `mode=allowlist` 时必须提供至少一条规则。
- 仅允许显式定义的协议/目标/端口组合。

## 2. I-CP-001 networkPlan（M3 起非空）

```json
{
  "networkPlan": {
    "tap": {"name": "sr-tap-<runId>"},
    "nft": {
      "table": "safe_run",
      "chains": ["output"],
      "rules": []
    }
  }
}
```

## 3. I-EV-001 新增网络事件

- `network.plan.generated`
- `network.rule.applied`
- `network.rule.hit`
- `network.rule.released`
- `network.rule.cleanup_failed`

## 4. I-RP-001 additive 字段

```json
{
  "networkAudit": {
    "mode": "allowlist",
    "rulesTotal": 4,
    "allowedHits": 23,
    "blockedHits": 7
  }
}
```

## 5. 错误码扩展

- `SR-POL-201`：网络规则缺失或非法
- `SR-CMP-201`：networkPlan 生成失败
- `SR-RUN-201`：网络规则应用失败
- `SR-RUN-202`：网络规则回收失败
