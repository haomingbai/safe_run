# M1 完成检查单

本检查单用于确认 M1 交付满足 `plan/` 约束，并记录真实运行验证结果与阻塞项。

## 1. 文档依据

- `plan/OVERVIEW.md`
- `plan/INTERFACE_BASELINE.md`
- `plan/M1/OVERVIEW.md`
- `plan/M1/MODULES.md`
- `plan/M1/INTERFACES.md`
- `plan/M1/ARCHITECTURE.md`
- `plan/M1/REFERENCES.md`

## 2. 阶段边界（M1）

- [x] 支持最小执行链：`validate -> compile -> run -> report`
- [x] `network.mode` 固定 `none`，未实现 `allowlist` 生效
- [x] 保持 `I-PL-001`、`I-VA-001`、`I-CP-001` 兼容（仅增量变化）
- [x] 未引入 M2 路径强化和 M3 网络白名单执行能力

## 3. 接口对齐

- [x] `I-RN-001`：`RunnerControlRequest/Response` 包含 `runId/state/artifacts/eventStream`
- [x] `I-EV-001`：事件包含 `timestamp/runId/stage/type/payload/hashPrev/hashSelf`
- [x] `I-RP-001`：`schemaVersion=safe-run.report/v1`，包含 M1 最小字段子集
- [x] Hash 口径符合全局规范：`policyHash`、`commandHash`、`integrity.digest`

## 4. 错误码覆盖

- [x] `SR-RUN-001`：prepare/monitor/cleanup 初始化与状态错误
- [x] `SR-RUN-002`：启动失败
- [x] `SR-RUN-003`：超时
- [x] `SR-EVD-001`：事件写入失败
- [x] `SR-EVD-002`：报告与 hash 生成失败

## 5. 自动化测试记录

- [x] 执行命令：`cargo test`
- [x] 结果：全部通过（`sr-cli/sr-policy/sr-compiler/sr-runner/sr-evidence` 与集成测试）

## 6. 真实运行验证记录

- [x] 已执行预检失败验证：`cargo run -p sr-cli -- run --policy examples/m1_network_none.yaml`（默认环境无 `jailer`，返回 `SR-RUN-002`，`path=launch.preflight.jailer`）
- [x] 已执行手工命令：`PATH="/tmp:$PATH" cargo run -p sr-cli -- run --policy examples/m1_network_none.yaml`（沙箱外）
- [x] 已记录产物路径：`/tmp/safe-run/runs/sr-1770459110-232306729/artifacts/run_report.json`
- [x] 已记录事件路径：`/tmp/safe-run/runs/sr-1770459110-232306729/artifacts/events.jsonl`
- [x] 状态达到 `finished`（`runId=sr-1770459110-232306729`，`state=finished`，`exitCode=0`）

修复后验证要点：

- `sr-runner` 已显式传递 `--api-sock /tmp/safe-run/runs/<runId>/artifacts/firecracker.socket`
- `sr-runner` 在 launch 前增加 `jailer/firecracker` 可执行文件预检，缺失时返回标准错误码
- 本机默认环境仍无真实 `jailer`，真实链路验证使用 `/tmp/jailer` 兼容脚本 + `/usr/bin/firecracker`

## 7. 结论

- [x] Context 8 的文档、测试、边界核对和验收记录已完成
- [x] M1 DoD 已满足（含真实链路 `state=finished` 验证）

## 8. 后续修复入口

后续修复任务已在 `TODO.md` 新增 `Context 8 后续修复任务（真实执行阻塞）`。

## 9. M1-FIX10 验收补齐记录（2026-02-07）

- [x] 已复现并覆盖回归路径：`prepare -> cleanup -> build_report`（`sr-cli` 单测 `report_build_succeeds_after_cleanup`）。
- [x] 已选定并实现修复方案：`cleanup` 保留 `firecracker-config.json`，仅清理临时运行文件（`runtime-context.json` / `vm.pid` / `api socket`）。
- [x] 已验证报告链路：cleanup 后仍可生成 `run_report.json`，不影响 `I-RP-001` 字段语义。
- [x] 已验证 hash 可复算：`integrity.digest`、`policyHash`、`commandHash` 相关测试通过。
- [x] 本次验证命令：`cargo test -p sr-cli -p sr-runner -p sr-evidence`（全部通过）。

## 10. M1-FIX11 异常退出错误码修复记录（2026-02-07）

- [x] 已确认问题：非零退出路径原实现仅设置 `RunState::Failed`，CLI 未返回 `SR-RUN-*`。
- [x] `sr-runner` 已在异常退出时写入 `run.failed` 事件并附 `errorCode=SR-RUN-001`。
- [x] `sr-cli run` 已在报告生成后按失败状态返回 `SR-RUN-001` 标准错误结构。
- [x] 验证命令：`cargo test -p sr-cli -p sr-runner`（全部通过）。
