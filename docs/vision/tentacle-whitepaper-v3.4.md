# Tentacle 工程白皮书 v3.4（Rust 重构·无状态纯净版·生态对齐定案）
**通用战术武器系统 — 自带说明书、按需组装、多传输层、凭证标签流转、渐进觅食、协程沙箱、动态共识**
**版本**：v3.4（2026-08-28）
**状态**：重构定案 + 生态对齐定案（四修正纳入）
**继承**：v3.3.2（含 409 移除/字段命名/中文分词微调）+ Gemini 生态审计四修正 + 用户执行方案
**治理**：DNA 自生长方法论 v2.0 ｜ Apache 2.0

## 卷首语：武器就是武器

一把好刀，不需要知道握刀的人是谁，也不需要替握刀的人保管钥匙。它只需要有自己的说明书——重量、重心、刃角、保养方式——任何人拿起来都能用。

Tentacle 就是这把刀。它是一个独立的、可组合的、自带说明书的通用工具执行引擎。它不依赖任何特定 AI Agent 框架。它只做一件事：**接收标准化的调用请求，在安全的沙箱中执行工具，返回结构化结果，然后忘记一切。**

Tentacle 不存储任何调用者的身份凭证——**它连明文凭证都不接触**。身份以无意义的标签（`identity_label`）流转，真实凭证由最边缘的 Tuck 物理闸门注入。网络采集工具拥有自适应渐进式觅食能力，信息增益低于阈值即主动终止。你可以通过 HTTP/MCP/gRPC/STDIO 调用它。**传输层是自由的，工具是通用的，安全是内建的，凭证是标签流转的，觅食是渐进且感知全局的，沙箱是协程级的，共识是动态适配的。**

---

## 第一部分：设计公理

Tentacle 遵循 Helix 生态公理，延伸出以下专属公理（v3.4 修订后）：

### 公理 T1：不预载，不应求则不动
启动只加载核心骨架（工具注册器 + 轻量说明书索引），不实例化任何工具引擎。工具仅在执行请求到达时懒加载，执行完毕后立即卸载。

### 公理 T2：自带说明书，谁都能用
每个工具携带完整 Manifest（名称/描述/参数 Schema/示例/触发条件/安全等级/所需权限/所需身份类型）。渐进披露：只对外提供说明书索引，发现适用时才请求完整说明书。

### 公理 T3：多传输层，多协议共生
核心与传输层彻底解耦。同一套工具可同时经 HTTP/JSON-RPC（RESTful + SSE）、MCP、gRPC、STDIO 暴露。

### 公理 T4：凭证标签流转，零信任
**v3.4 修订（四修正 1）**：Tentacle 不存储、**不接触**任何明文凭证。调用者提供 `identity_label`（无意义标签），Tentacle 出网强制携带 `X-Identity-Label` 头，真实凭证由 Tuck（127.0.0.1:9000）在物理边缘注入。宿主代签私钥永不在 Tentacle 进程内出现。**独立模式（无 Tuck）**：凭证由调用者在本地沙箱提供（用户自担风险），Helix 生态模式必须标签流转。

### 公理 T5：沙箱即契约，安全可验证
每个工具执行在独立沙箱：WASM（wasmtime）、JS（QuickJS 协程沙箱）、命令（受限用户，默认编译期禁用）。工具必须声明权限，越权即拒绝。

### 公理 T6：自适应觅食，信息饱和即止
**v3.4 修订（四修正 2）**：网络采集工具基于本地统计算法 + **已见熵布隆过滤器**（`seen_entropy_bloom`，Callosum 导出）动态评估信息增益。布隆碰撞 → 增益归 0（全局记忆已覆盖）。增益低于阈值即主动终止，输出阶段性成果并销毁沙箱。

---

## 第二部分：架构法则

### 法则 T01：多 Crate 解耦，极小化核心
```
crates/
├── tentacle-core          # Tool trait, Manifest, ToolRegistry, 安全等级, 脱敏管道, ConsensusHook trait
├── tentacle-http          # HTTP 客户端：TLS 指纹模拟、反检测、X-Identity-Label 出网头
├── tentacle-tools         # 内置工具集：定向爬取（含渐进觅食+布隆）、文件操作、文本提取等
├── tentacle-transport-http # HTTP 传输层（axum）：RESTful + SSE
├── tentacle-transport-mcp  # MCP 传输层（rmcp）：stdio/HTTP/SSE
├── tentacle-transport-grpc # gRPC 传输层（tonic）：供 Anaphase 等
├── tentacle-wasm          # WASM 插件运行时（wasmtime），可选 feature
├── tentacle-js            # JS 协程沙箱（QuickJS + worker 池），可选 feature
└── tentacle               # 二进制入口，按需组装
```
`tentacle-core` 纯逻辑层，无网络依赖，<500KB。网络具体实现（HTTP 共识钩子、SPIFFE 令牌、Tuck 出网）经 Rust Trait 依赖注入。

### 法则 T02：声明与执行分离，渐进披露
`plugins/tool.manifest.json`（静态声明）+ `plugins/tool.wasm/.js`（执行体）。Manifest 含执行体 SHA-256；注册时校验，不一致拒绝注册 + 告警。

### 法则 T03：按需加载，执行即焚
执行请求 → 索引匹配 → 校验 Manifest → 懒加载运行时 → 执行 → 返回 → **立即卸载**。

### 法则 T04：乐高式组装，形态自由
原子工具（Atom）/ 工具链（Chain，DAG 组合）/ 工具模板（Template，预设配置）。说明书驱动，无需改代码。

### 法则 T05：凭证标签流转，用完即焚
**v3.4 修订（四修正 1）**：
```json
POST /v1/tools/execute
{
  "tool": "targeted_scraper",
  "params": { ... },
  "identity_labels": { "weibo": "weibo_user_session_1" }
}
```
Tentacle 出网强制加 `X-Identity-Label` 头，**不注入明文凭证**。Tuck 拦截识别标签 → 物理保密柜注入真实凭证 → 发公网。独立模式：本地沙箱提供凭证（用户自担）。

**宿主代签验证链**：Manifest `requires_external_signer == true` → `payload.target_domain ∈ allowed_signing_domains` → 频率 < `rate_limit_per_minute` → 代理外部 KMS/MPC。私钥永不进入进程。

### 法则 T06：协程沙箱安全
**v3.4 修订（四修正 3）**：
- **JS**：固定大小 worker 池（`worker_pool_size` 默认绑定物理核）+ QuickJS 多 context 单线程协程调度。`set_interrupt_handler` 周期性检查与 `CancellationToken` 联动的 `AtomicBool`，超时/流 Drop → 协程级协作终止（C 引擎内部终止异常 → 优雅解退回收）。`JS_SetMemoryLimit` 限内存，移除 require/import 和文件/网络 API。**不在端侧开 OS 线程**。
- **WASM**：wasmtime 沙箱，按 Manifest 权限配置 WASI，`epoch_deadline` 超时强制终止。
- **命令**：默认编译期禁用（`no-native-commands` feature flag），启用时 `nobody` 用户 + ulimit + 可选 chroot。

### 法则 T07：自适应觅食 + 已见熵过滤器
**v3.4 修订（四修正 2）**：网络采集工具内建渐进式觅食评估器，每步评估信息增益：
- 本地 token 词频/Jaccard 相似度（本次会话已见）
- **布隆过滤器碰撞**（`seen_entropy_bloom`，Callosum 导出——全局记忆已覆盖 → 增益 0）
- 增益 < 阈值 → `ForageDecision::Stop` → 输出阶段性成果 → 销毁沙箱（**不视为错误，200 OK 或正常关闭流**）

---

## 第三部分：核心设计

### 3.1 工具说明书（Manifest）规范
（继承 v3.3.2，字段含 `max_pages_per_session`、`foraging_config`，完整性 `integrity.sha256`）

### 3.2 传输层设计
（继承 v3.3.2）HTTP/MCP/gRPC/STDIO 四传输层平等可替换，共享同一 `ToolRegistry`。HTTP：`GET /v1/manifest`（索引）、`GET /v1/tools/{name}/manifest`、`POST /v1/tools/{name}/execute`、`POST /v1/tools/{name}/execute_stream`（SSE，觅食终止正常关闭连接）。

### 3.3 凭证标签流转流程（v3.4 修订）
1. 调用者提供 `identity_labels`（标签，无明文凭证）
2. Tentacle 验证标签与 Manifest `requires_identity` 匹配
3. 沙箱出网请求强制携带 `X-Identity-Label` 头
4. **Helix 模式**：Tuck（127.0.0.1:9000）拦截标签 → 物理保密柜注入真实凭证 → 发公网
5. **独立模式**：本地沙箱注入调用者提供凭证（用户自担）
6. 审计日志脱敏敏感字段
**宿主代签**：沙箱内 `host_sign(payload)` → 宿主验证链 → 外部 KMS/MPC → 私钥永不进入进程。

### 3.4 渐进式觅食评估器（v3.4 修订，含布隆）
```rust
pub struct ForagingEvaluator {
    target_tokens: HashSet<String>,
    seen_entropy: HashMap<String, u32>,
    /// v3.4：全局已见熵布隆过滤器（Callosum 导出，可选）
    seen_entropy_bloom: Option<BloomFilter>,
    threshold_delta: f64,
    pages_fetched: u32,
    max_pages_per_session: u32,
}
// 评估：布隆碰撞 → 增益 0（全局记忆已覆盖）；本地已见 → 不计；
// novel_target_tokens / total_target_tokens < threshold → Stop
```
> *中文分词：字符级 N-Gram（如 2-Gram）滑窗，零依赖鲁棒。*
**布隆假阳性**：可配置（默认 1%），假阳性导致"漏爬"是可接受的节能权衡（极致节能优先）。

### 3.5 乐高式组装 / 3.6 插件热插拔
（继承 v3.3.2）工具链 TOML DAG 定义；插件热插拔 + 完整性校验。

---

## 第四部分：生态位与关系

（继承 v3.3.2）Tentacle 是自包含通用工具服务器。Helix 是其中一个用户。
- **对齐链**：Tentacle 对齐 Anaphase（gRPC + 凭证标签 + 共识 Helix 模式）→ Anaphase 对齐 Mind
- **CAP 协同**：Critical 级操作动态共识（Standalone 终端确认 / Helix CAP / MCP 回调）
- **语义发现**：默认只输出说明书索引，调用方负责语义检索；极简 BM25 仅作可选 CLI 调试 feature（`--features discover`）

---

## 第五部分：HTTP 能力深度说明
（继承 v3.3.2）远程部署、Bearer Token 认证、无状态、SSE 流式、CORS、OpenAPI、token bucket 限流。

---

## 第六部分：安全设计

### 6.1 完整性校验 / 6.2 权限声明
（继承 v3.3.2）SHA-256 绑定；`permissions{network/filesystem/execute/max_memory_mb/max_cpu_time_ms}`。

### 6.3 协程沙箱执行（v3.4 修订）
见法则 T06。JS 用 worker 池 + 协程级协作终止（ARM 端侧）。

### 6.4 宿主代签安全
（继承 v3.3.2）验证链：requires_external_signer → allowed_signing_domains → rate_limit。私钥永不进入进程。

### 6.5 审计日志与脱敏
（继承 v3.3.2）自动脱敏：Manifest `sensitive_fields` + 通用敏感关键词（cookie/token/api_key/authorization/private_key）。脱敏默认开启不可绕过。

### 6.6 流式响应的孤儿沙箱防御
（继承 v3.3.2）Stream Drop → CancellationToken → AtomicBool → 协程终止/WASM epoch 到期/命令 SIGKILL → 审计。

### 6.7 动态共识适配层（v3.4 修订）
```rust
pub enum ConsensusMode {
    Standalone,   // CLI/独立模式 → 终端物理确认
    Helix,        // Anaphase 生态 → gRPC 桥接 CAP 服务
    MCP,          // MCP 模式 → MCP 协议回调确认
}
```
- **Standalone**：`eprintln!("[Tentacle] Critical operation requires approval. Type 'yes': ")` → stdin 等待 `yes`
- **Helix**：gRPC 调用 CAP 服务获取人类确认密码学证明
- **MCP**：通过 MCP sampling/自定义工具回调
- 共识不可达：Helix 模式降级拒绝 Critical（503）；Standalone 永不 503（本地确认）
- **职责边界**：与 Anaphase HITL（执行闸，编排层）不重叠——HITL 管编排层，共识钩子管工具层 Critical

### 6.8 安全策略引擎 / 6.9 标准错误码
（继承 v3.3.2）全局策略（deny_all 默认）；错误码表（**409 已移除**——觅食终止归 200 正常完成）。

---

## 第七部分：配置与降级

（继承 v3.3.2 + v3.4 新增）
```toml
[tentacle]
http_enabled = true
http_listen = "0.0.0.0:8080"
grpc_enabled = false
grpc_listen = "0.0.0.0:50062"
security_policy = "strict"
max_execution_time_ms = 30000

[consensus]
mode = "standalone"   # standalone / helix / mcp（v3.4）
helix_cap_endpoint = ""

[js_sandbox]          # v3.4
worker_pool_size = 2  # 默认绑定物理核

[foraging]
default_threshold_delta = 0.15
default_max_pages_per_session = 50
bloom_false_positive = 0.01  # v3.4：布隆假阳性率
bloom_enabled = true         # v3.4：依赖 Callosum 导出

[tuck]                # v3.4：凭证标签流转
enabled = false       # Helix 模式 true
proxy = "127.0.0.1:9000"
```

---

## 第八部分：总结

Tentacle 是一套**独立的、自带说明书的、多传输层的、凭证标签流转的、渐进式觅食（感知全局）的、乐高式可组装的、协程沙箱安全的、动态共识适配的通用工具执行引擎**。

- **通用**：HTTP/MCP/gRPC/STDIO 平等服务所有调用者
- **极简**：启动 <2MB 内存，按需加载，用完即焚
- **零信任**：凭证标签流转，明文凭证永不进入内存，Tuck 边缘注入
- **节能**：渐进式觅食 + 已见熵布隆，全局记忆感知，避免重复采集
- **性能**：协程沙箱 + worker 池，ARM 端侧可跑
- **独立**：动态共识适配，Standalone 模式终端确认，不返回 503

**Tentacle 不是 Helix 的双手，而是全 AI Agent 社区的公共武器库。它携带说明书，流转标签（身份），智能觅食（信息采集），严守安全，动态共识（可选信任锚），随时待命。**

---

*本白皮书随 Tentacle Rust 版同步演进。任何架构变更必须通过哲学审查与跨组件对齐（DNA 方法论 D 层）。*
