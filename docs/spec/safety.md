# 安全卷（spec/safety.md）
> **来源**：Tentacle 工程白皮书 v3.4 第六部分提炼
> **性质**：Tentacle 的安全设计——内建、可配置、对所有传输层生效

## 一、安全层总览

安全内建，不依赖任何外部系统，任何调用者均可受益。

| 层 | 机制 | 强度 |
|---|---|---|
| 完整性校验 | Manifest SHA-256 ↔ 执行体哈希 | 声明与执行密码学绑定 |
| 权限声明 | Manifest `permissions` 越权拒绝 | 工具级 |
| 运行时沙箱 | WASM（wasmtime）/ JS（QuickJS 协程）/ 命令（受限） | 隔离级 |
| 宿主代签 | `requires_external_signer` 验证链 | 高敏操作 |
| 审计脱敏 | Schema 感知脱敏管道，默认开启不可绕过 | 日志级 |
| 孤儿沙箱防御 | Stream Drop → 强制终止 | 流式级 |
| 安全策略引擎 | 全局策略（默认 deny_all） | 策略级 |
| 动态共识 | Standalone/Helix/MCP 三模式 | Critical 级 |

## 二、关键安全机制

### 2.1 完整性校验
注册工具时计算实际执行体 SHA-256，与 Manifest `integrity.hash` 比对。不一致 → 拒绝注册 + 告警。

### 2.2 权限声明
```json
{
  "permissions": {
    "network": ["outbound"],
    "filesystem": ["read:/tmp", "write:/tmp/tentacle"],
    "execute": false,
    "max_memory_mb": 50,
    "max_cpu_time_ms": 30000
  }
}
```

### 2.3 协程沙箱（四修正 3）
- **JS**：固定 worker 池（绑定物理核）+ QuickJS 多 context 协程调度；`set_interrupt_handler` + `AtomicBool` + `CancellationToken` → 协程级协作终止；`JS_SetMemoryLimit` 限内存；移除 require/import 和文件/网络 API。**不在端侧开 OS 线程**
- **WASM**：wasmtime，按权限配置 WASI，`epoch_deadline` 超时强制终止
- **命令**：默认编译期禁用（`no-native-commands`），启用时 `nobody` 用户 + ulimit + 可选 chroot

### 2.4 宿主代签验证链
沙箱工具 `host_sign(payload)` → Manifest `requires_external_signer` → `target_domain ∈ allowed_signing_domains` → 频率限流 → 外部 KMS/MPC 代签。**私钥永不进入进程。**

### 2.5 审计日志与脱敏
记录：时间/调用者/工具名/参数（脱敏）/凭证（脱敏）/时长/结果/资源。
脱敏管道：Manifest `sensitive_fields` + 通用敏感关键词（cookie/token/api_key/authorization/private_key）。**默认开启、不可绕过。**

### 2.6 孤儿沙箱防御
客户端断开 → `SandboxStream::drop()` → `CancellationToken.cancel()` + `AtomicBool` 翻转 → 沙箱强制终止（协程/epoch/SIGKILL）→ 审计。

### 2.7 动态共识适配（四修正 4）
```rust
pub enum ConsensusMode {
    Standalone,   // CLI → 终端物理确认
    Helix,        // Anaphase 生态 → gRPC 桥接 CAP
    MCP,          // MCP → MCP 协议回调确认
}
```
- Standalone：`eprintln!("[Tentacle] Critical operation requires approval. Type 'yes': ")` → stdin `yes`
- Helix：gRPC 调用 CAP 服务
- MCP：MCP sampling/自定义工具回调
- 共识不可达：Helix 模式拒绝 Critical（503）；**Standalone 永不 503**
- **职责边界**：与 Anaphase HITL（编排层执行闸）不重叠——HITL 管编排层，共识钩子管工具层 Critical

### 2.8 安全策略引擎
```toml
[security]
default_policy = "deny_all"
allowed_network_domains = ["api.weibo.com", "www.zhihu.com"]
allow_file_write = false
require_consensus_for = ["critical"]
```

## 三、零信任凭证（安全红线）

Tentacle 内存中**永无明文 Cookie/Token**：
- 请求契约只接受 `identity_labels`（拒绝明文 credentials 字段）
- 出网强制 `X-Identity-Label` 头
- 真实凭证由 Tuck 在物理边缘注入（Helix 模式）
- 独立模式本地自担（用户知情）

**验收**：`grep -r "cookie\|token\|api_key" tentacle-core/` 源码审计 + 运行时内存扫描，均不得出现明文凭证值。

---

*《安全卷》完。*
