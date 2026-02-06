# M4 模块划分

## 1. sr-evidence

- 新增：`verifier.rs`, `archiver.rs`, `index.rs`。
- 职责：报告校验、归档、检索。

## 2. sr-cli

- 新增：`report verify`, `report inspect`（可选）。
- 职责：为审计与运维提供稳定命令入口。

## 3. sr-ops

- 子模块：`packaging`, `deploy_checks`, `demo_runner`。
- 职责：降低部署与验收门槛。

## 4. 文档模块

- `docs/README.md`
- `docs/DESIGN.md`
- `docs/SECURITY.md`
- `docs/OPERATIONS.md`

## 5. 测试模块

- `tests/report_verify_compat`
- `tests/archive_integrity`
- `tests/e2e_release_smoke`
