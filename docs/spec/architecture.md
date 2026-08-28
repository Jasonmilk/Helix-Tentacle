# 架构卷（spec/architecture.md）
> **来源**：Tentacle 工程白皮书 v3.4 第二部分提炼
> **性质**：Tentacle 的物理架构——crates 结构、架构法则与关键机制

## 一、多 Crate 解耦，极小化核心（T01）

```
crates/
├── tentacle-core          # Tool trait, Manifest, ToolRegistry, 安全等级, 脱敏管道, ConsensusHook trait
├── tentacle-http          # HTTP 客户端：TLS 指纹模拟、反检测、X-Identity-Label 出网头
├── tentacle-tools         # 内置工具集：定向爬取（渐进觅食+布隆）、文件操作、文本提取等
├── tentacle-transport-http # HTTP 传输层（axum）：RESTful + SSE
├── tentacle-transport-mcp  # MCP 传输层（rmcp）：stdio/HTTP/SSE
├── tentacle-transport-grpc # gRPC 传输层（tonic）：供 Anaphase 等
├── tentacle-wasm          # WASM 插件运行时（wasmtime），可选 feature
├── tentacle-js            # JS 协程沙箱（QuickJS + worker 池），可选 feature
└── tentacle               # 二进制入口，按需组装
```

**核心原则**：`tentacle-core` 纯逻辑层，无网络依赖，<500KB。网络具体实现经 Rust Trait 依赖注入，Core 不硬编码。

## 二、架构法则（T01-T07）

| # | 法则 | 核心 |
|---|---|---|
| T01 | 多 Crate 解耦 | 极小化核心，依赖注入 |
| T02 | 声明与执行分离 | Manifest（声明）+ wasm/js（执行体），SHA-256 绑定 |
| T03 | 按需加载，执行即焚 | 懒加载 → 执行 → 立即卸载 |
| T04 | 乐高式组装 | Atom / Chain（DAG）/ Template |
| T05 | 凭证标签流转 | `identity_labels` 出网，Tuck 边缘注入 |
| T06 | 协程沙箱安全 | worker 池 + QuickJS 协程协作终止 |
| T07 | 自适应觅食 + 已见熵 | 信息增益 + 布隆碰撞 → 增益 0 |

## 三、核心机制

### 3.1 声明与执行分离（T02）
```
plugins/
├── targeted_scraper.manifest.json    # 静态声明（说明书+权限+SHA-256）
└── targeted_scraper.wasm             # 执行体（仅被调用时加载）
```
注册时校验 SHA-256，不一致拒绝注册 + 告警。

### 3.2 按需加载，执行即焚（T03）
1. 执行请求 → 索引匹配 → 校验 Manifest → 懒加载运行时
2. 执行 → 返回 `ToolOutput` / 流式 `Stream<Item=OutputChunk>`
3. 返回后立即卸载运行时，释放所有资源

### 3.3 渐进式觅食评估器（T07）
```rust
pub struct ForagingEvaluator {
    target_tokens: HashSet<String>,
    seen_entropy: HashMap<String, u32>,        // 本次会话已见
    seen_entropy_bloom: Option<BloomFilter>,   // 全局已见熵（Callosum 导出）
    threshold_delta: f64,
    pages_fetched: u32,
    max_pages_per_session: u32,
}
```
- 布隆碰撞 → 增益 0（全局记忆已覆盖）
- novel_target_tokens / total_target_tokens < threshold → `ForageDecision::Stop`
- 终止不视为错误 → 200 OK + `stop_reason` 元数据 / SSE 正常关闭

### 3.4 协程沙箱（T06）
- JS：固定 worker 池（默认绑定物理核）+ QuickJS 多 context 单线程协程调度
- `set_interrupt_handler` + `AtomicBool` + `CancellationToken` → 协程级协作终止
- `JS_SetMemoryLimit` 限内存；移除 require/import 和文件/网络 API
- WASM：wasmtime + `epoch_deadline`；命令：默认编译期禁用

## 四、降级策略

| 场景 | 行为 |
|---|---|
| 插件目录不可用 | 索引为空，仅内置工具 |
| WASM 未编译 | WASM 插件跳过，不影响其他 |
| 传输层端口冲突 | 仅该传输层不可用 |
| 共识服务不可达 | Helix 模式拒绝 Critical（503）；Standalone 终端确认 |
| 安全策略缺失 | 默认严格策略（deny_all） |
| Manifest 校验失败 | 拒绝该工具，不影响已注册 |

---

*《架构卷》完。*
