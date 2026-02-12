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

- `mode=none` 时不得提供 `egress` 规则；若 `network.egress` 非空应返回 `SR-POL-201`（`path=network.egress`）。
- `mode=allowlist` 时必须提供至少一条规则。
- 仅允许显式定义的协议/目标/端口组合。
- M3 定稿细则（详见 `plan/M3/NETWORK_CLARIFICATIONS.md`）：
  - `protocol` 仅允许 `tcp|udp`（IPv4-only）。
  - `port` 必填，范围 `1..=65535`。
  - `host` 与 `cidr` 必须二选一；`host` 在运行期解析（A 记录）。

## 2. I-CP-001 networkPlan（M3 起非空）

```json
{
  "networkPlan": {
    "tap": {"name": "sr-tap-<runId>"},
    "nft": {
      "table": "safe_run",
      "chains": ["forward"],
      "rules": []
    }
  }
}
```

说明：

- 当 `network.mode=none`：`networkPlan` 必须为 `null`（保持 M0-M2 口径与快照兼容）。
- 当 `network.mode=allowlist`：`networkPlan` 必须为非空对象（详见 `plan/M3/NETWORK_CLARIFICATIONS.md`）。

## 3. I-EV-001 新增网络事件

- `network.plan.generated`
- `network.rule.applied`
- `network.rule.hit`
- `network.rule.released`
- `network.rule.cleanup_failed`

`network.rule.hit` payload（M3 Stage 5 定稿，计数器采样口径）：

```json
{
  "tap": "sr-tap-sr-20260212-001",
  "table": "safe_run",
  "chain": "forward",
  "protocol": "tcp",
  "target": "1.1.1.1/32",
  "port": 443,
  "allowedHits": 23,
  "blockedHits": 7
}
```

补充约束：

- 事件粒度：单条已应用规则。
- 仅当 `allowedHits + blockedHits > 0` 时写入事件。
- 事件写入受 `evidencePlan.events` gating 控制。

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

`mode=none` 输出口径（M3 Stage 5 定稿）：

```json
{
  "networkAudit": {
    "mode": "none",
    "rulesTotal": 0,
    "allowedHits": 0,
    "blockedHits": 0
  }
}
```

## 5. 错误码扩展

- `SR-POL-201`：网络规则缺失或非法
- `SR-CMP-201`：networkPlan 生成失败
- `SR-RUN-201`：网络规则应用失败
- `SR-RUN-202`：网络规则回收失败
