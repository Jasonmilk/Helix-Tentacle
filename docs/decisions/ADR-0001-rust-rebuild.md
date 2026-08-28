# ADR-0001：Rust 重构 + 生态对齐（凭证标签流转等四修正）

## 状态

**Active**（2026-08-28，已审查通过）

## 问题

1. Tentacle `rs` 分支为早期 Python 版（明文 cookie 加载 + 本地觅食），与生态对齐方向相反
2. 身份明文传递与 Tuck 零信任冲突（安全红线）
3. 渐进式觅食无全局记忆感知（能耗浪费）
4. JS 沙箱 OS 线程开销在 ARM 端侧不可行（性能）
5. 共识钩子独立运行降级 503（独立体验）

## 决策

1. **清理 Python 残留**：物理移除 `cli/`/`cookies/`/`domains/`/`tentacle/`/`tests/`/`pyproject.toml`，main 分支保留 Python 版为哲学历史
2. **Apache 2.0**：协议从 MIT 改为 Apache 2.0
3. **DNA 方法论先行**：五件套（VISION/DNA/GROWTH/PLAN/decisions）建立后再编码
4. **四修正（硬性验收）**：
   - **修正 1 凭证标签流转**（🔴 安全红线）：`ExecutionRequest.credentials` → `identity_labels`（标签如 `weibo_session_1`）；Tentacle 出网强制 `X-Identity-Label` 头，Tuck（127.0.0.1:9000）边缘注入真实凭证。独立模式（无 Tuck）下凭证由调用者本地提供（用户自担），Helix 生态模式用标签流转
   - **修正 2 已见熵布隆过滤器**（🟡 节能）：`forager.rs` 接收 `seen_entropy_bloom: Option<BloomFilter>`（Callosum 导出）；布隆假阳性可配置（如 1%）；依赖 Callosum 渐进（先本地 seen_entropy，布隆可选 feature）
   - **修正 3 异步协程沙箱**（🟢 性能）：JS 沙箱固定 worker 池（绑定物理核）+ QuickJS 多 context 协程调度，协变中断降级为协程级协作终止
   - **修正 4 动态共识适配**（🟡 体验）：`ConsensusMode{Standalone/Helix/MCP}`——Standalone 终端确认（`eprintln!` + stdin `yes`）；Helix gRPC 桥接 CAP；MCP 回调。独立模式不返回 503。与 Anaphase HITL（执行闸）职责不重叠：HITL 管编排层，共识钩子管工具层 Critical
5. **对齐链**：Tentacle 对齐 Anaphase（gRPC + 凭证标签 + 共识 Helix 模式）→ Anaphase 对齐 Mind

## P4 追加决策（2026-08-29，生态对齐 + 传输层实现 + 插件热插拔）

6. **gRPC 传输层（T1）**：使用 tonic 框架（与 Anaphase-Helix 一致）。proto 独立定义（`TentacleService`：ListManifests/GetManifest/ExecuteTool/ExecuteToolStream），与 HTTP 端点语义对齐。同步阻塞执行（`tokio::task::spawn_blocking` 执行 `Tool::execute`）。`ExecuteToolStream` 简化为一次性输出 + done=true，完整流式后续优化。
7. **MCP 传输层（T2）**：手动实现 JSON-RPC 2.0 over stdio（而非 rmcp 框架）。原因：rmcp 的 `#[tool_router]` 宏是编译时静态注册，不适合 Tentacle 的动态工具注册；MCP 协议本身很简单，手动实现更轻量，更符合极致解耦/极致节能哲学。支持 `initialize`/`notifications/initialized`/`tools/list`/`tools/call`，协议版本 `2024-11-05`。
8. **MCP 扩展字段（T3）**：利用 MCP 协议标准的 `_meta` 字段传递生态特定信息，不破坏协议兼容性。`_meta.identityLabels`（凭证标签流转，四修正1）、`_meta.traceId`（分布式追踪）、`_meta.seenEntropyBloom`（已见熵布隆过滤器，四修正2）。
9. **MCP 参数 Schema 校验（T3）**：轻量级校验（`required` 字段 + 类型匹配：string/number/integer/boolean/object/array，integer 是 number 的子类型）。完整 JSON Schema 校验（enum/pattern/minimum 等）后续引入 `jsonschema` crate。
10. **CI-144 语义对齐（T3）**：Tentacle 的 gRPC 服务语义与 CI-144 各层对应（INTENT-7=tool+params，CAPABILITY-13=security_level+permissions，BIND-19=二进制/JSON双模，INTENT-7-SECURE=identity_labels+Tuck）。CI-144 v2.0 PAL（物理锚定层）待冻结后自然接入，不阻塞 P4。
11. **插件热插拔（T4）**：使用 notify crate 的 RecommendedWatcher，跨平台，递归监听 `plugins/` 目录。只监听 `.manifest.json` 文件变化（新增/修改→注册/更新，删除→注销）。作为可选 feature `hot-reload`，默认不启用，保持 Tentacle 轻量。执行体（.wasm/.js）的热加载后续阶段实现。
12. **三传输层统一凭证标签流转**：HTTP（请求体 `identity_labels`）、gRPC（`ExecuteToolRequest.identity_labels` map）、MCP（`_meta.identityLabels`）三种传输层中的传递方式统一 documented。

## 白皮书升级

- `docs/vision/tentacle-whitepaper-v3.4.md`：在 v3.3.2 基础上纳入四修正（T05/3.3 凭证标签、T07/3.4 布隆、T06/6.3 协程、6.7 共识适配）+ 独立/Helix 双轨语义

## 回滚阈值

若 Rust 重构导致与 Anaphase gRPC 契约不兼容，或四修正验收失败产生安全回归，可回滚至清理前状态重新评估。

## 关联

- Anaphase-Helix ADR-0001（契约对齐，P10a-P11b）
- Helix-Mind ADR-0010/0020/0021
- Tentacle 工程白皮书 v3.4
