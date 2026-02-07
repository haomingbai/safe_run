# M1 接口设计

## 1. I-RN-001 RunnerControl（首次启用）

请求模型：

```json
{
  "compileBundle": {"...": "..."},
  "runtimeContext": {
    "workdir": "/var/lib/safe-run/runs/<runId>",
    "timeoutSec": 300
  }
}
```

响应模型：

```json
{
  "runId": "sr-20260206-001",
  "state": "finished",
  "artifacts": {
    "log": "...",
    "report": "run_report.json"
  }
}
```

## 2. I-EV-001 EvidenceEvent（M1 事件类型）

- `run.prepared`
- `vm.started`
- `resource.sampled`
- `vm.exited`
- `run.cleaned`
- `run.failed`

事件必须包含：`timestamp/runId/type/payload/hashPrev/hashSelf`。

## 3. I-RP-001 RunReport v1（M1 字段子集）

```json
{
  "schemaVersion": "safe-run.report/v1",
  "runId": "...",
  "startedAt": "...",
  "finishedAt": "...",
  "exitCode": 0,
  "artifacts": {
    "kernelHash": "sha256:...",
    "rootfsHash": "sha256:...",
    "policyHash": "sha256:...",
    "commandHash": "sha256:..."
  },
  "policySummary": {"network": "none", "mounts": 0},
  "resourceUsage": {"cpu": "...", "memory": "..."},
  "events": [],
  "integrity": {"digest": "sha256:..."}
}
```

### 3.1 Hash 口径（M1）

- `policyHash`：对 `PolicySpec` 进行 JSON 规范化后计算 SHA-256。
- `commandHash`：对 `{"command": <runtime.command>, "args": <runtime.args>}` 进行 JSON 规范化后计算 SHA-256。
- `integrity.digest`：对整份 `RunReport` 进行 JSON 规范化后计算 SHA-256，**计算前需将 `integrity.digest` 置空字符串**。

JSON 规范化规则遵循全局基线（字段排序、无空白字符、数组保持顺序）。

## 4. 错误码扩展

- `SR-RUN-001`：Runner 初始化失败
- `SR-RUN-002`：VM 启动失败
- `SR-RUN-003`：执行超时
- `SR-EVD-001`：事件写入失败
- `SR-EVD-002`：报告生成失败
