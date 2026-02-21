# report_verify_compat

本目录用于承载 M4 迭代 A 的 `report verify` 兼容性测试集说明。

当前自动化测试入口位于：

- `crates/sr-evidence/tests/report_verify_compat.rs`

覆盖用例（8）：

- 有效报告通过：2
- schema 不匹配：2
- artifact hash 不匹配：2
- event chain 断裂：2
