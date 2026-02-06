# M1 模块划分

## 1. sr-runner

- 子模块：`prepare`, `launch`, `monitor`, `cleanup`。
- 输出：运行状态 + 事件。

## 2. sr-evidence

- 子模块：`event_writer`, `hashing`, `report_builder`。
- 输出：`run_report.json`。

## 3. sr-compiler（M1 扩展）

- 从 M0 dry-run 升级为可执行配置输出。
- 保持 `CompileBundle` 字段与语义不变。

## 4. sr-cli

- 新增 `run` 子命令。
- 保留 `validate` 与 `compile --dry-run`。

## 5. 集成测试

- `tests/run_smoke`
- `tests/run_failure_paths`
- `tests/report_schema_v1`
