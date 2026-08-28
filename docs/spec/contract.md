# 契约卷（spec/contract.md）
> **来源**：Tentacle 工程白皮书 v3.4 第三/四/五部分提炼
> **性质**：Tentacle 对外契约——传输层契约、凭证标签流转、身份流转流程
> **更新**：v1.1（P4-T3 生态对齐：三传输层契约细化 + MCP 扩展字段 + 参数 Schema 校验）

## 一、多传输层契约（T3）

同一套工具经三种传输层平等暴露，共享同一 `ToolRegistry`，无状态冲突。

| 传输层 | 端点/协议 | 适用 | identity_labels 传递 |
|---|---|---|---|
| **HTTP**（axum） | `GET /v1/manifest`（索引）、`GET /v1/tools/{name}/manifest`、`POST /v1/tools/{name}/execute`、`POST /v1/tools/{name}/execute_stream`（SSE） | 任何语言/平台远程调用 | 请求体 `identity_labels` 字段 |
| **gRPC**（tonic） | `TentacleService`：`ListManifests`/`GetManifest`/`ExecuteTool`/`ExecuteToolStream` | Anaphase 等内部生态 | `ExecuteToolRequest.identity_labels` map |
| **MCP**（JSON-RPC 2.0） | stdio：`initialize`/`tools/list`/`tools/call` | Claude Desktop、Codex 等通用生态 | `tools/call` 的 `_meta.identityLabels` 扩展字段 |

**HTTP 细节**：Bearer Token 认证、token bucket 限流、CORS、OpenAPI/Swagger、无状态（不维持会话，不存储凭证）。

**gRPC 细节**：proto 定义在 `crates/tentacle-transport-grpc/proto/tentacle.proto`，与 HTTP 端点语义对齐。`ExecuteToolStream` 简化为一次性输出 + `done=true`，完整流式在后续优化。

**MCP 细节**：手动实现 JSON-RPC 2.0 over stdio（而非 rmcp 框架），原因：rmcp 的 `#[tool_router]` 宏是编译时静态注册，不适合 Tentacle 的动态工具注册。MCP 协议版本 `2024-11-05`，支持 `tools` capability。

### 1.1 MCP 扩展字段（_meta）

MCP 协议标准的 `tools/call` 请求支持 `_meta` 扩展字段，Tentacle 利用此字段传递生态特定信息：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "targeted_scraper",
    "arguments": { "query": "test" },
    "_meta": {
      "identityLabels": {
        "weibo": "weibo_session_1"
      },
      "traceId": "trace-abc-123",
      "seenEntropyBloom": "<base64-encoded-bloom-filter>"
    }
  }
}
```

| 扩展字段 | 类型 | 说明 |
|---|---|---|
| `_meta.identityLabels` | map<string, string> | 凭证标签流转（四修正1），无意义标签，Tuck 物理注入明文凭证 |
| `_meta.traceId` | string | 分布式追踪 ID，透传至 ToolOutput |
| `_meta.seenEntropyBloom` | string | 已见熵布隆过滤器（四修正2），来自 Callosum，避免重复采集 |

### 1.2 参数 Schema 校验

所有传输层在执行工具前，校验 `arguments`/`params` 是否符合 Manifest 的 `parameters_schema`（JSON Schema 2020-12）。

**轻量级校验**（当前实现）：
- `required` 字段是否存在
- 字段类型是否匹配（`string`/`number`/`integer`/`boolean`/`object`/`array`）
- `integer` 是 `number` 的子类型

**校验失败**：返回 `isError: true`，错误信息包含字段名和期望类型。

**完整 JSON Schema 校验**（后续优化）：引入 `jsonschema` crate，支持 `enum`/`pattern`/`minimum`/`maximum`/`items` 等完整约束。

## 二、凭证标签流转契约（T05，四修正 1）

### 2.1 请求契约
```json
POST /v1/tools/execute
{
  "tool": "targeted_scraper",
  "params": { ... },
  "identity_labels": { "weibo": "weibo_user_session_1" }
}
```
**禁止**明文 `credentials` 字段。Tentacle 出网强制携带 `X-Identity-Label` 头。

> 三传输层统一流转流程见 §3.3。

### 2.2 宿主代签
沙箱内 `host_sign(payload)` → 宿主验证链：
1. `Manifest.requires_external_signer == true`
2. `payload.target_domain ∈ allowed_signing_domains`
3. 频率 < `rate_limit_per_minute`
→ 代理外部 KMS/MPC → 签名返回沙箱。**私钥永不进入 Tentacle 进程。**

## 三、Anaphase 生态适配契约

Tentacle 对齐 Anaphase（gRPC）：说明书自动转 CAP 快照；共识 Helix 模式桥接 CAP 服务。

**对齐链**：Helix-Tentacle → Anaphase-Helix → Helix-Mind。

### 3.1 gRPC 契约对齐

Tentacle 的 gRPC 服务（`TentacleService`）与 Anaphase-Helix 的 gRPC 客户端契约对齐：

| Tentacle RPC | Anaphase 调用场景 | 语义 |
|---|---|---|
| `ListManifests` | Anaphase 启动时扫描可用工具 | 渐进披露，只返回名称+描述+版本+安全等级 |
| `GetManifest` | Anaphase 决定调用某工具前获取完整说明书 | 含 parameters_schema，用于参数校验 |
| `ExecuteTool` | Anaphase 编排执行工具 | 一次性返回结果，含 stop_reason（觅食终止） |
| `ExecuteToolStream` | Anaphase 需要流式输出（如渐进式觅食） | 流式返回结果 chunk |

### 3.2 CI-144 语义对齐

Tentacle 的 gRPC 传输层语义与 CI-144（生态血液语言）对齐：

| CI-144 层 | Tentacle 对应 | 说明 |
|---|---|---|
| **INTENT-7**（意图语义） | `ExecuteToolRequest.tool` + `params` | 工具名即意图动词，params 即意图参数 |
| **CAPABILITY-13**（能力认证） | Manifest `security_level` + `permissions` | 工具能力声明，Anaphase 据此决定是否需要 HITL |
| **BIND-19**（传输绑定） | gRPC 二进制帧（生产环境）/ JSON（调试） | 双模通信，二进制极致节能，JSON 白盒可观测 |
| **INTENT-7-SECURE**（安全层） | `identity_labels` + Tuck 物理注入 | 凭证标签流转，明文凭证永不进入 Tentacle 内存 |

**CI-144 v2.0 PAL（物理锚定层）**：待冻结后自然接入，Tentacle 的 gRPC 帧头将携带 PAL 元数据（Modality/Risk-Level/Override-Flag），Anaphase 和 Tuck 可在不解析载荷的情况下读取元数据。

### 3.3 凭证标签流转（三传输层统一）

所有传输层统一使用 `identity_labels`（无意义标签），禁止明文凭证：

| 传输层 | 字段位置 | 示例 |
|---|---|---|
| HTTP | 请求体 `identity_labels` | `{"identity_labels": {"weibo": "weibo_session_1"}}` |
| gRPC | `ExecuteToolRequest.identity_labels` map | `identity_labels: {"weibo": "weibo_session_1"}` |
| MCP | `tools/call` 的 `_meta.identityLabels` | `"_meta": {"identityLabels": {"weibo": "weibo_session_1"}}` |

**统一流转流程**：
1. 调用者提供 `identity_labels`（无意义标签）
2. Tentacle 验证标签与 Manifest `requires_identity` 匹配
3. 沙箱出网请求强制 `X-Identity-Label` 头
4. **Helix 模式**：Tuck（127.0.0.1:9000）拦截标签 → 物理保密柜注入真实凭证 → 发公网
5. **独立模式**：本地沙箱注入调用者提供凭证（用户自担）
6. 审计日志脱敏敏感字段

## 四、标准错误码

| 错误码 | 含义 |
|---|---|
| `503 Consensus Unavailable` | 外部共识不可达（仅 Helix 模式拒绝 Critical） |
| `403 Identity Required` | 需要身份凭证但未提供标签 |
| `403 Permission Denied` | 越权被沙箱拦截 |
| `403 Integrity Mismatch` | Manifest 哈希不匹配 |
| `408 Execution Timeout` | 工具超时被强制终止 |
| `429 Rate Limited` | 调用超限流 |
| `400 Invalid Manifest` | Manifest 格式错误 |

> **觅食终止**（`LowInformationGain`/`MaxPagesReached`/`ZeroGainStreak`）不视为错误——单步 200 OK + `stop_reason` 元数据；流式正常关闭 SSE 连接。

---

*《契约卷》完。*
