# M3 网络 allowlist 规则澄清（供实现与后续阶段复用）

## 1. 目的

本文件用于消除 M3 阶段关于 `network.mode=allowlist` 的歧义点，确保测试、实现与后续阶段（M4）扩展不冲突。

适用范围：M3（网络白名单控制）。

## 2. 阶段边界与默认拒绝

- M0-M2：仅允许 `network.mode=none`，`CompileBundle.networkPlan` 必须保持 `null`（见 `plan/INTERFACE_BASELINE.md`）。
- M3：允许 `network.mode=allowlist` 与 `CompileBundle.networkPlan` 实际生效（见 `plan/M3/OVERVIEW.md`）。
- 仍坚持 deny-by-default：未显式配置 allowlist 时，应保持“无网”安全姿态（见 `plan/M3/OVERVIEW.md`）。

## 3. PolicySpec.network（M3 扩展字段与约束）

### 3.1 字段结构（additive）

在既有 `network.mode` 基础上，M3 增量引入 `network.egress[]`：

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

说明：

- `network.mode`：`none|allowlist`（字段已存在，语义在 M3 启用 allowlist）。
- `network.egress[]`：M3 新增（additive），用于描述 egress allowlist 规则集合。

### 3.2 allowlist 规则语义（M3 定稿）

1. `mode=none` 时不得提供任何 `network.egress[]` 规则（即必须为空或省略）。
   - 若检测到非空 `network.egress[]`：`SR-POL-201`，`path=network.egress`。
2. `mode=allowlist` 时必须提供至少一条规则：`network.egress` 非空数组。
   - 违反时：`SR-POL-201`，`path=network.egress`。
3. 协议集合：仅允许 `tcp|udp`（M3 不引入其它协议）。
4. 端口：`port` 必填，范围 `1..=65535`。
5. 目标二选一：每条规则必须且只能提供 `host` 或 `cidr` 之一：
   - `host`：域名（运行期解析）。
   - `cidr`：IPv4 CIDR（例如 `1.2.3.4/32`）。
6. IPv4-only：M3 仅支持 IPv4；IPv6 作为后续阶段扩展项（见 `TODO.md` / `M3_STAGE_PLAN.md`）。

校验错误统一映射为：`SR-POL-201`，并将 `path` 精确指向 `network.egress[i].<field>`。

### 3.3 运行期域名解析（M3 定稿）

当规则使用 `host` 时：

- 解析时机：**运行期**（runner apply 阶段），不得在 compile 阶段解析（保持编译输出确定性）。
- 解析范围：仅解析 A 记录（IPv4）。
- 多地址语义：若解析出多个 IPv4 地址，应按“同一条规则展开为多条等价目标”处理（实现可在 networkPlan 中保留原始 host，并在 apply 阶段展开为多条 nft 规则）。
- 解析失败：应视为运行失败并返回 `SR-RUN-201`（错误路径建议：`launch.network.dns` 或 `launch.network.apply`，需与实现保持一致）。

## 4. CompileBundle.networkPlan（M3 定稿）

### 4.1 none 模式语义（确认）

- 当 `network.mode=none`：`CompileBundle.networkPlan` 必须为 `null`（保持 M0-M2 快照与兼容口径）。

### 4.2 allowlist 模式语义（确认）

- 当 `network.mode=allowlist`：`CompileBundle.networkPlan` 必须为非空结构化对象（不得为 `null`）。

### 4.3 最小结构（M3）

```json
{
  "networkPlan": {
    "tap": { "name": "sr-tap-<runId>" },
    "nft": {
      "table": "safe_run",
      "chains": ["forward"],
      "rules": []
    }
  }
}
```

约束：

- `tap.name` 使用占位符 `<runId>`；runner 在运行期用 `PreparedRun.run_id` 替换，以确保资源名与 runId 强绑定且避免跨运行污染。
- `nft.chains`：M3 以 `forward` 为主链（见 5.1）。
- 确定性：`networkPlan` 不得包含时间戳/随机数/runId 等非确定性数据；同一 `normalizedPolicy` 的 compile 输出必须一致。

## 5. nftables/TAP 技术选型的关键澄清

### 5.1 链选择（确认）

为避免与实际数据路径不一致，M3 默认以 `forward` 链作为 egress allowlist 过滤主入口（替代示例中的 `output`）。

文档与实现需保持一致：

- `plan/M3/INTERFACES.md` 的示例应使用 `chains:["forward"]`。
- 若后续发现目标环境必须兼容其它 hook 点（例如 output/ingress），应先更新 `plan/` 再改实现。

### 5.2 “真实可出网闭环”作为可选项（确认）

由于不同运行环境对网络能力限制差异很大（沙箱/非沙箱、是否具备 root、是否允许创建 TAP/nft），M3 将“真实可出网闭环”定义为可选项：

- M3 P0/P1 目标：完成策略校验、networkPlan 编译、runner 生命周期 apply/release、证据事件与报告聚合的**可测试闭环**（mock/recording 测试为主）。
- 可选项：在具备权限的 Linux 主机上完成真实 TAP + nft + NAT/路由 + guest IP 配置的集成验证。
- guest IP 配置方式：优先考虑通过 kernel boot args 提供更灵活的定制（具体机制在实现前需在 `plan/` 定稿）。

## 6. 错误码映射（M3 定稿）

见 `plan/M3/INTERFACES.md`：

- `SR-POL-201`：网络规则缺失或非法（validate 阶段）。
- `SR-CMP-201`：networkPlan 生成失败（compile 阶段）。
- `SR-RUN-201`：网络规则应用失败（run 阶段）。
- `SR-RUN-202`：网络规则回收失败（cleanup 阶段）。

## 7. Stage 5 定稿：`network.rule.hit` 与 `networkAudit` 口径

### 7.1 `network.rule.hit` payload schema（M3 定稿）

M3 采用“计数器采样”语义，不采集逐包事件。runner 在网络生命周期采样阶段写入 `network.rule.hit` 事件，payload 结构如下：

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

约束：

- `allowedHits`、`blockedHits` 为非负整数（`u64` 口径）。
- 事件粒度为“单条已应用规则”，一条规则对应一条计数事件。
- 仅当 `allowedHits + blockedHits > 0` 时写事件（避免零命中噪声）。
- 事件仍受 `CompileBundle.evidencePlan.events` gating：未声明 `network.rule.hit` 时不得写该事件。

### 7.2 `networkAudit` 聚合口径（M3 定稿）

`RunReport.networkAudit` 聚合规则如下：

1. `mode`：优先取事件流中的 `network.plan.generated.mode`，缺失时回退到 `PolicySpec.network.mode`。
2. `rulesTotal`：优先取 `network.plan.generated.rulesTotal`；缺失时回退到 `network.rule.applied` 事件条数；再缺失时回退到 `PolicySpec.network.egress` 条数。
3. `allowedHits` / `blockedHits`：对 `network.rule.hit` 事件中的 `allowedHits` / `blockedHits` 求和。

host 多 IP 展开口径：

- 对于 `host` 解析出的每个 IPv4 目标（`/32`）视为独立规则目标；
- `networkAudit` 以全局求和口径聚合，不做去重折叠。

### 7.3 `mode=none` 的 `networkAudit` 输出策略（M3 定稿）

`networkAudit` 字段在 `RunReport` 中始终存在（additive 且稳定）：

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

### 7.4 allowlist 默认证据事件集（M3 定稿）

为确保 M3“网络规则下发/命中/回收可审计”目标可默认生效：

- 当 `network.mode=allowlist`，`CompileBundle.evidencePlan.events` 默认应包含：
  - `network.plan.generated`
  - `network.rule.applied`
  - `network.rule.hit`
  - `network.rule.released`
  - `network.rule.cleanup_failed`
- 当 `network.mode=none`，保持 M0-M2 默认证据事件集，不强制新增 network 事件。
