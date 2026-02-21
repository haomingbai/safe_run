# TODO（M4 阶段：测试驱动交付与合规化）

> 阶段范围：仅 M4（交付打磨、报告校验、归档检索、发布与验收），不新增高风险底层能力。
>
> 执行方式：严格 TDD（Red → Green → Refactor），每个任务必须先写失败测试，再做最小实现。

## 0. M4 硬约束（执行前检查）

- [ ] 仅做 additive 变更；不删除/重命名既有接口字段（`schemaVersion` 保持 `safe-run.report/v1`）。
- [ ] `I-VF-001` 必须落地：`safe-run report verify <run_report.json>`。
- [ ] `I-RP-001` 仅新增 `archive`、`verification` 字段，旧报告可读。
- [ ] 不新增虚拟化后端或其他高风险底层能力。

## 1. TDD 总流程门禁（所有任务通用）

- [ ] **Red**：先新增/修改测试并确认失败（至少 1 个明确断言失败）。
- [ ] **Green**：仅实现使当前失败测试通过的最小代码。
- [ ] **Refactor**：重构后重新跑同组测试，结果保持全绿。
- [ ] 每个任务提交说明中必须包含：
  - 失败测试名称（Red 证据）
  - 通过命令与结果摘要（Green 证据）
  - 重构影响面（Refactor 证据）

## 2. 迭代 A：`report verify` 核心能力（`sr-evidence` + `sr-cli`）

### A1. 先写测试（Red）

- [x] 新建/完善测试集：`tests/report_verify_compat`。
- [x] 最低测试用例数：**8**（全部可自动化）。
  - [x] 有效报告通过：2
  - [x] schema 不匹配：2
  - [x] artifact hash 不匹配：2
  - [x] event chain 断裂：2
- [x] 失败路径必须分别断言错误码：`SR-EVD-301`、`SR-EVD-302`、`SR-EVD-303`。

### A2. 最小实现（Green）

- [x] 实现校验器最小闭环：`schema`、`artifact_hash`、`event_chain` 三项检查。
- [x] CLI 命令打通：`safe-run report verify ./run_report.json`。
- [x] 成功返回必须包含：`valid=true` 且 `checks` 至少含 3 项（名称固定：`schema`/`artifact_hash`/`event_chain`）。

### A3. 重构与回归（Refactor）

- [x] 抽离校验逻辑到独立模块（不改变 CLI 行为语义）。
- [x] 回归命令：`cargo test -p sr-evidence`、`cargo test -p sr-cli report`。
- [x] 验收阈值：上述命令 **0 failed**。

## 3. 迭代 B：归档与完整性（`sr-evidence`）

### B1. 先写测试（Red）

- [x] 新建/完善测试集：`tests/archive_integrity`。
- [x] 最低测试用例数：**6**。
  - [x] 归档写入成功：2
  - [x] 归档读取/索引成功：2
  - [x] 归档写入失败（权限/路径）触发：2
- [x] 写入失败路径必须断言错误码：`SR-OPS-301`。

### B2. 最小实现（Green）

- [x] 实现归档元信息：`archive.bundleId`、`archive.storedAt`、`archive.retention`。
- [x] 实现验证元信息：`verification.algorithm`、`verification.verifiedAt`、`verification.result`。
- [x] 默认算法固定 `sha256`；输出格式保持结构化 JSON。

### B3. 重构与回归（Refactor）

- [x] 索引与归档写入逻辑解耦，保证单元可测。
- [x] 回归命令：`cargo test -p sr-evidence archive`。
- [x] 验收阈值：**100% 通过（0 failed, 0 ignored in this suite）**。

## 4. 迭代 C：兼容性与回归防护（M1-M3 报告向后兼容）

### C1. 先写测试（Red）

- [ ] 增加兼容样本：至少 **3** 份（分别代表 M1/M2/M3 输出特征）。
- [ ] 为“缺失 M4 新字段”场景编写失败前置测试（验证器初始应失败或不兼容）。

### C2. 最小实现（Green）

- [ ] 调整读取与校验流程：旧报告缺失 `archive`/`verification` 时按可选处理。
- [ ] 保持旧字段语义不变，不引入破坏性迁移。

### C3. 重构与回归（Refactor）

- [ ] 回归命令：`cargo test -p sr-evidence report_verify_compat`。
- [ ] 验收阈值：
  - [ ] 兼容样本通过率 **100%**。
  - [ ] 新增兼容测试无 flaky（同一命令连续执行 3 次结果一致）。

## 5. 迭代 D：CLI 交付面与运维可用性（`sr-cli` + `sr-ops`）

### D1. 先写测试（Red）

- [ ] CLI 集成测试最少 **6** 条：
  - [ ] `report verify` 成功输出结构：2
  - [ ] 错误输入/文件缺失：2
  - [ ] 校验失败时错误码透传：2

### D2. 最小实现（Green）

- [ ] 完成 `safe-run report verify` 用户入口与错误码映射。
- [ ] （可选）`safe-run report inspect` 仅在不影响主链时纳入；若纳入，需同样先测后写。
- [ ] 新增/更新运维脚本：发布前自检命令集（可一键执行）。

### D3. 重构与回归（Refactor）

- [ ] 回归命令：`cargo test -p sr-cli`。
- [ ] 验收阈值：CLI 相关测试 **0 failed**，且错误码覆盖率达到 **4/4**（`301/302/303` + `SR-OPS-301`）。

## 6. 迭代 E：发布烟测与文档交付（`sr-ops` + 文档模块）

### E1. 先写测试（Red）

- [ ] 建立 `tests/e2e_release_smoke` 最少 **5** 条脚本化检查：
  - [ ] 干净环境命令可执行
  - [ ] 报告可校验
  - [ ] 归档可写入并可查询
  - [ ] 失败路径可定位
  - [ ] 清理后无残留

### E2. 最小实现（Green）

- [ ] 完成文档交付：`docs/README.md`、`docs/DESIGN.md`、`docs/SECURITY.md`、`docs/OPERATIONS.md`。
- [ ] 文档命令均可在目标环境复现（按脚本实际跑通）。

### E3. 重构与回归（Refactor）

- [ ] 发布前总回归：`cargo test` + `tests/e2e_release_smoke`。
- [ ] 验收阈值：
  - [ ] Rust 测试：**0 failed**。
  - [ ] 烟测脚本：**5/5 通过**。
  - [ ] 关键命令文档一致性抽检：**10 条命令 10/10 可复现**。

## 7. M4 最终验收（严格量化 DoD）

- [ ] 功能 DoD：
  - [ ] `safe-run report verify` 可用，且三类校验均生效。
  - [ ] `run_report.json` 支持离线校验。
- [ ] 兼容 DoD：
  - [ ] M1-M3 样本兼容率 **100%**。
  - [ ] 接口变更审查记录为 additive（0 个破坏性变更）。
- [ ] 质量 DoD：
  - [ ] M4 新增测试总数 **>= 25**。
  - [ ] M4 相关测试通过率 **100%**。
  - [ ] M4 关键套件连续回归 3 轮，无 flaky。
- [ ] 交付 DoD：
  - [ ] 文档四件套齐备并通过命令抽检。
  - [ ] 发布烟测通过，归档与检索链路可演示。

## 8. 执行记录（每次迭代更新）

- [x] 迭代日期：2026-02-21
- [x] 迭代范围（A/B/C/D/E）：A, B
- [x] Red 失败测试：`cargo test -p sr-evidence report_verify_compat` 初次失败（`SR_EVD_301/302/303` 常量与 `verify_report` 未实现）。
- [x] Green 通过证据：`cargo test -p sr-evidence report_verify_compat`（8 passed, 0 failed）。
- [x] Refactor 说明：新增 `crates/sr-evidence/src/verifier.rs`，将报告校验逻辑从 CLI 抽离为独立模块并由 CLI 复用。
- [x] Red 失败测试：`cargo test -p sr-evidence archive_integrity` 初次失败（`archive_report/load_archive_index/load_archived_report` 未实现）。
- [x] Green 通过证据：`cargo test -p sr-evidence --test archive_integrity`（6 passed, 0 failed, 0 ignored）。
- [x] Refactor 说明：新增 `crates/sr-evidence/src/archiver.rs` 与 `crates/sr-evidence/src/index.rs`，将“归档写入”与“索引维护”解耦。
- [x] 风险与遗留项：迭代A/B完成；C-E（兼容样本、运维脚本、文档交付）仍待后续迭代。

## 9. 依据文档（事实来源）

- `plan/OVERVIEW.md`
- `plan/INTERFACE_BASELINE.md`
- `plan/M4/OVERVIEW.md`
- `plan/M4/INTERFACES.md`
- `plan/M4/MODULES.md`
- `plan/M4/ARCHITECTURE.md`
- `plan/M4/REFERENCES.md`
