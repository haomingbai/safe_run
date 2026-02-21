# archive_integrity

该目录用于记录 M4 迭代 B（归档与完整性）的测试入口与验收命令。

- 主要自动化测试文件：`crates/sr-evidence/tests/archive_integrity.rs`
- 执行命令：`cargo test -p sr-evidence --test archive_integrity`

覆盖场景（共 6 条）：

1. 归档写入成功并返回 `archive`/`verification` 元信息。
2. 归档写入成功并可从落盘文件读取报告。
3. 归档索引读取成功并包含写入的 bundle。
4. 归档索引可追加多条 bundle 记录。
5. 归档根路径异常时返回 `SR-OPS-301`。
6. 索引写入路径异常时返回 `SR-OPS-301`。
