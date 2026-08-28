# 契约卷（spec/contract.md）
> **来源**：Tentacle 工程白皮书 v3.4 第三/四/五部分提炼
> **性质**：Tentacle 对外契约——传输层契约、凭证标签流转、身份流转流程

## 一、多传输层契约（T3）

同一套工具经四种传输层平等暴露，共享同一 `ToolRegistry`，无状态冲突。

| 传输层 | 端点/协议 | 适用 |
|---|---|---|
| **HTTP**（axum） | `GET /v1/manifest`（索引）、`GET /v1/tools/{name}/manifest`、`POST /v1/tools/{name}/execute`、`POST /v1/tools/{name}/execute_stream`（SSE） | 任何语言/平台远程调用 |
| **MCP**（rmcp） | stdio/HTTP(SSE)/WebSocket；Manifest → `tools/list`/`tools/call` | Claude Desktop、Codex 等 |
| **gRPC**（tonic） | 高性能二进制 + stream | Anaphase 等内部生态 |
| **STDIO** | `echo '{"tool":...}' \| tentacle stdio` | 本地调试、脚本、极简部署 |

**HTTP 细节**：Bearer Token 认证、token bucket 限流、CORS、OpenAPI/Swagger、无状态（不维持会话，不存储凭证）。

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

### 2.2 流转流程
1. 调用者提供 `identity_labels`（无意义标签）
2. Tentacle 验证标签与 Manifest `requires_identity` 匹配
3. 沙箱出网请求强制 `X-Identity-Label` 头
4. **Helix 模式**：Tuck（127.0.0.1:9000）拦截标签 → 物理保密柜注入真实凭证 → 发公网
5. **独立模式**：本地沙箱注入调用者提供凭证（用户自担）
6. 审计日志脱敏敏感字段

### 2.3 宿主代签
沙箱内 `host_sign(payload)` → 宿主验证链：
1. `Manifest.requires_external_signer == true`
2. `payload.target_domain ∈ allowed_signing_domains`
3. 频率 < `rate_limit_per_minute`
→ 代理外部 KMS/MPC → 签名返回沙箱。**私钥永不进入 Tentacle 进程。**

## 三、Anaphase 生态适配契约

Tentacle 对齐 Anaphase（gRPC）：说明书自动转 CAP 快照；共识 Helix 模式桥接 CAP 服务。

**对齐链**：Helix-Tentacle → Anaphase-Helix → Helix-Mind。

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
