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

## 7. 待确认项（写代码前必须先定稿）

以下点不在本次确认范围内，写测试/实现前需先在 `plan/` 补齐并确认：

1. `network.rule.hit` 的 payload schema（逐包事件 vs 计数器采样、字段名与聚合口径）。
2. `networkAudit` 在 `mode=none` 时的输出策略（字段是否总是存在、默认值口径）。
3. allowlist “命中/拦截”的精确定义与规则优先级（尤其是 host 多 IP 展开后的计数聚合方式）。
