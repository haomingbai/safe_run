# M4 接口设计

## 1. I-VF-001 ReportVerify（首次正式发布）

命令：

```bash
safe-run report verify ./run_report.json
```

返回示例：

```json
{
  "valid": true,
  "checks": [
    {"name": "schema", "ok": true},
    {"name": "artifact_hash", "ok": true},
    {"name": "event_chain", "ok": true}
  ]
}
```

## 2. I-RP-001 additive 字段（M4）

```json
{
  "archive": {
    "bundleId": "bundle-...",
    "storedAt": "...",
    "retention": "180d"
  },
  "verification": {
    "algorithm": "sha256",
    "verifiedAt": "...",
    "result": "pass"
  }
}
```

## 3. 兼容性声明

- `schemaVersion` 仍为 `safe-run.report/v1`。
- M1-M3 生成的报告可被 M4 验证器读取（缺失新字段时按可选处理）。

## 4. 错误码扩展

- `SR-EVD-301`：报告 schema 不匹配
- `SR-EVD-302`：artifact hash 校验失败
- `SR-EVD-303`：事件链完整性校验失败
- `SR-OPS-301`：归档写入失败
