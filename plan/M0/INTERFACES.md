# M0 接口设计

## 1. I-PL-001 PolicySpec（M0 生效子集）

```yaml
apiVersion: policy.safe-run.dev/v1alpha1
metadata:
  name: demo-job
runtime:
  command: /bin/echo
  args: ["hello"]
resources:
  cpu:
    max: "100000 100000"
  memory:
    max: 256Mi
network:
  mode: none
mounts: []
audit:
  level: basic
```

M0 约束：

- `network.mode` 仅允许 `none`。
- `mounts` 可为空；若非空仅做格式校验，不做安全判定（M2 强化）。

## 2. I-VA-001 ValidationResult

```json
{
  "valid": true,
  "errors": [],
  "warnings": [],
  "normalizedPolicy": {"...": "..."}
}
```

校验覆盖：

- 必填字段完整性。
- 资源上限格式合法性。
- 默认拒绝语义（无显式放行即拒绝）。

## 3. I-CP-001 CompileBundle（dry-run）

```json
{
  "firecrackerConfig": {"machine-config": {}, "boot-source": {}, "drives": []},
  "jailerPlan": {"enabled": true, "ops": []},
  "cgroupPlan": {"enabled": true, "ops": []},
  "networkPlan": null,
  "evidencePlan": {"enabled": true, "events": ["compile"]}
}
```

说明：

- M0 仅生成计划，不执行任何 host 变更。
- `networkPlan` 固定为 `null`，与 M3 扩展兼容。

## 4. 错误码映射

- `SR-POL-001`：缺少必填字段
- `SR-POL-002`：字段格式错误
- `SR-POL-003`：策略语义冲突
- `SR-CMP-001`：编译模板缺失
- `SR-CMP-002`：编译输出不完整
