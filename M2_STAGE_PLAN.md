# M2 过渡评估与 Stage 拆分（测试先行）

> 目标：在 M1 执行闭环基础上，强化挂载与路径安全（canonicalize + 白名单前缀 + deny-by-default + 组合约束），并将挂载决策纳入证据链与报告。
>
> 依据文档（唯一事实来源）：
>
> - plan/OVERVIEW.md
> - plan/INTERFACE_BASELINE.md
> - plan/M2/OVERVIEW.md
> - plan/M2/MODULES.md
> - plan/M2/INTERFACES.md
> - plan/M2/ARCHITECTURE.md
> - plan/M2/REFERENCES.md

## 全局硬约束（M2 仍必须满足）

- 接口兼容：仅允许 additive change；禁止删除/重命名已发布字段。
- M0-M2 网络：`network.mode` 仅允许 `none`；`CompileBundle.networkPlan` 必须保持 `null`。
- `PolicySpec.runtime.args` 与 `PolicySpec.mounts` 必须显式存在（可为空数组但不可缺失）。
- 所有挂载决策必须走 `sr-policy` 统一策略引擎，禁止绕过（plan/M2/OVERVIEW.md）。

## 需要确认的设计缺口（不确认前不应“臆造”实现）

> 以下点在 plan/M2 里提到“白名单前缀 / 允许命名空间 / 组合约束”，但未给出可执行的具体参数。

1. **宿主路径白名单前缀列表**：白名单来自哪里？白名单需要来自配置文件.
2. **guestPath 允许命名空间**：允许哪些前缀？禁止覆盖哪些关键系统路径？请根据Unix系统规范, 在项目中编写一份规约, 确保只能覆盖数据和非重要路径 (例如/opt, /media, /home 等等, 但是需要提供修改能力, 用户可以通过配置绕开禁令).
3. **“风险组合”定义**：目前 `PolicySpec` 无“权限等级”字段；组合约束需要明确以哪些策略字段作为条件。可以新增相应字段, 但是HASH相应规范必须作出调整(明确字段在计算SHA时的位置等信息).
4. **字段命名不一致**：plan/M2/INTERFACES.md 示例用 `hostPath/guestPath/readOnly`，当前实现/示例使用 `source/target/read_only`。将字段名映射写在文档当中.

建议处理方式（保持兼容 + 不破坏既有样例）：

- 先在 `sr-policy::Mount` 上通过 serde `alias` 同时接受两套字段名（additive）。
- 白名单/命名空间/组合约束参数：建议在实现 Stage 1 前先由你确认一版“可执行默认值”（或同意以 `sr-policy` 内常量方式落地）。

---

## Stage 划分（每个 Stage 目标：一次上下文内完成）

### Stage 1：接口对齐与错误码脚手架（测试先行）

**先写代表性测试**

- `sr-policy`：解析 `hostPath/guestPath/readOnly` 样例应成功，并规范化为内部结构。
- `sr-policy`：新增无效用例 YAML（不涉及真实 mount）覆盖：
  - `hostPath`/`source` 为空
  - `guestPath`/`target` 非绝对路径

**实现内容**

- `sr-policy::Mount` 增加 serde `alias`：`hostPath->source`，`guestPath->target`，`readOnly->read_only`。
- 在 `sr-common` 增加错误码常量：`SR_POL_101/102/103`、`SR_RUN_101`（plan/M2/INTERFACES.md）。

**验收标准（可编译可执行）**

- `cargo test -p sr-policy` 通过。
- `cargo test -p sr-common`（如存在）通过。

### Stage 2：PathSecurityEngine（canonicalize + 白名单前缀）

**先写代表性测试**

- 使用临时目录构造：
  - allowlisted 目录下真实路径通过。
  - allowlisted 目录下的 symlink 指向非白名单目录时被拒绝。
  - `..` 穿越在 canonicalize 后落到白名单外时被拒绝。

**实现内容**（plan/M2/ARCHITECTURE.md）

- `crates/sr-policy/src/path_security.rs`：实现 `PathSecurityEngine`，对 `mount.source` 进行 canonicalize 并做“白名单前缀匹配”。
- 在 `validate_policy()` 中调用该引擎；失败映射为 `SR-POL-101`。

**验收标准**

- `cargo test -p sr-policy` 通过，且新增的逃逸/软链用例覆盖生效。

### Stage 3：MountConstraints（敏感宿主路径 + guestPath 命名空间）

**先写代表性测试**

- `source` canonicalize 后位于 `/proc`、`/sys`、`/dev`（或你确认的敏感集）必须拒绝。
- `target` 指向关键系统路径（或不在允许命名空间）必须拒绝。

**实现内容**

- `crates/sr-policy/src/mount_constraints.rs`：
  - 宿主敏感路径 denylist。
  - `target` 命名空间规则（允许/禁止前缀）。
- 错误码映射：
  - `SR-POL-102`：target 不合法/命名空间违规。

**验收标准**

- `cargo test -p sr-policy` 通过。
- 新增的策略无效样例（YAML）回归通过。

### Stage 4：编译阶段 MountPlanBuilder（可回滚计划 + 快照测试）

**先写代表性测试**

- `sr-compiler`：给定包含 mounts 的 `PolicySpec`，编译输出包含“挂载计划”信息（形式需与你确认：作为 `jailerPlan.ops` 的结构化 op，或作为 CompileBundle 的新增字段）。
- 快照测试：mount plan 变更能被检测。

**实现内容**（plan/M2/MODULES.md）

- `crates/sr-compiler/src/mount_plan.rs`：实现 `MountPlanBuilder`。
- 将 mount plan 注入编译输出（并保持 `networkPlan=null`）。

**验收标准**

- `cargo test -p sr-compiler` 通过。
- `CompileBundle` 仍满足 `ensure_bundle_complete()`。

### Stage 5：Runner 挂载执行与失败回滚（尽量不依赖 root）

**先写代表性测试**

- 计划执行顺序/回滚顺序的纯逻辑测试（无需真实 mount）。
- 若需要真实 mount：以 `#[ignore]` + root-only 的方式增加最小集成测试。

**实现内容**（plan/M2/MODULES.md）

- `crates/sr-runner/src/mount_executor.rs`：按计划应用挂载。
- `crates/sr-runner/src/rollback.rs`：失败时按逆序回滚，保证清理。
- 失败映射：`SR-RUN-101`（plan/M2/INTERFACES.md）。

**验收标准**

- `cargo test -p sr-runner` 通过。
- （可选手工）root 环境执行被 `#[ignore]` 的 mount 集成测试。

### Stage 6：证据链与报告 mountAudit（additive）

**先写代表性测试**

- 构造 events.jsonl（含 `mount.validated/rejected/applied`）输入，断言报告 JSON 包含 `mountAudit` 且计数与 reasons 正确。

**实现内容**

- `sr-evidence` 增加挂载审计聚合器，将结果注入 `RunReport` 的 additive 字段 `mountAudit`。
- `I-EV-001` 新事件类型：`mount.validated/mount.rejected/mount.applied`。

**验收标准**

- `cargo test -p sr-evidence` 通过。
- `cargo test` 全通过（全仓回归）。

---

## M2 总体验收标准（可落实、可验证）

- 策略层：路径逃逸（`..` / symlink / 非白名单）稳定拦截，并返回 `SR-POL-101/102/103` 中正确错误码。
- 编译层：挂载计划输出可回滚（有序 + 可逆），且快照测试能感知规则变更。
- 执行层：挂载失败能回滚并返回 `SR-RUN-101`。
- 证据层：`run_report.json` additive 增加 `mountAudit`，并能从事件流重建计数与 reasons。
- 约束回归：`network.mode` 仍固定 `none`，`CompileBundle.networkPlan` 仍为 `null`。
