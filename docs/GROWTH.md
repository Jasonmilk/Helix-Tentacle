# Helix-Tentacle 生长记录
> **版本**：v1.3
> **日期**：2026-08-30
> **规则**：仅保留最近 3 条记录，超则归档至 `docs/archive/growth/`
> **归档策略**：历史随仓库版本化，永不删除

## 记录 1：P3 完成——工具集成 + WASI 细化 + worker 池 + 首个内置工具 + 传输层集成（2026-08-29）
**变异类型**：P3 阶段完成
**背景**：
- P3 目标：WASI 细化 + worker 池优化 + 工具执行引擎集成 + 首个内置工具 targeted_scraper + 传输层集成测试
- 按 T1→T2→T3→T4→T5 串行执行，每 T 验证后再进下一个
- 方法论闭环：ADR 命名规范修正（0001-rust-rebuild.md → ADR-0001-rust-rebuild.md）
**关键决策**：
1. **T1 WASI 细化**：接入 wasmtime-wasi v26 preview1。stdout 用 MemoryOutputPipe 捕获到内存（可 clone，执行后 contents() 获取）；文件系统用 preopened_dir 按 manifest.permissions.filesystem 配置（支持 read:/write:/rw: 前缀）；未声明目录无法访问，越权即拒绝
2. **T2 worker 池**：JS 沙箱从"每次独立线程"升级为"固定 worker 池 + 多 context 单线程协程调度"。worker 数量默认与 CPU 核心数绑定，防止 ARM 端侧过饱和。每个 worker 持有一个 QuickJS Runtime，多 Context 共享 Runtime 但隔离执行
3. **T3 工具执行引擎**：WasmTool/JsTool（包装沙箱，实现 Tool trait，持有共享沙箱 Arc）+ PluginLoader（扫描→校验→加载→注册完整链路，按扩展名选择沙箱，双重完整性校验）。WasmOutput.exit_code 不判定成功/失败（WASM 返回值不一定是退出码），执行成功总是 ok=true
4. **T4 targeted_scraper**：首个内置工具，HTTP 客户端（IdentityHttpClient，X-Identity-Label 头）+ 渐进式觅食（ForagingEvaluator）+ HTML 解析（regex，script/style 独立正则避免反向引用）+ 同步阻塞（内部 tokio runtime）。布隆过滤器接口保留，完整序列化待 Callosum 接口明确
5. **T5 传输层集成测试**：7 个端到端测试全绿。覆盖 HTTP+WasmTool、HTTP+JsTool、PluginLoader 完整链路、SSE 流式、凭证标签不泄露、未注册工具 404、说明书索引渐进披露。gRPC/MCP 传输层仍为空壳，后续阶段实现
**方法论修正**：
- ADR 命名规范：RNA.md 决策拦截铁律增加完整命名规范（ADR-<4位编号>-<标题>.md），现有文件重命名为 ADR-0001-rust-rebuild.md
**验收**：
- T1: 10 passed (runtime)，WASI stdout 捕获 + 文件系统权限验证
- T2: 14 passed (runtime)，worker 池 + 多 context 并发验证
- T3: 21 passed (runtime)，WasmTool/JsTool/PluginLoader 单元测试
- T4: 20 passed (scraper)，extract_text + scraper 基础测试
- T5: 7 passed (integration)，端到端传输层集成测试
- 全 feature 编译：通过（runtime + scraper + bloom）
**状态**：✅ P3 完成，进入 P4（生态对齐 + gRPC/MCP 传输层实现 + CI-144 接入）
---
## 记录 2：P4 完成——生态对齐 + gRPC/MCP 传输层 + 插件热插拔 + 全传输层集成（2026-08-29）
**变异类型**：P4 阶段完成
**背景**：
- P4 目标：gRPC 传输层 + MCP 传输层 + 生态对齐（Anaphase-Helix 契约 + CI-144 语义）+ 插件热插拔 + 全传输层集成测试
- 按 T1→T2→T3→T4→T5 串行执行，每 T 验证后再进下一个
- 方法论闭环：ADR-0001 追加 P4 决策（6-12），PLAN.md 实时更新，提交信息持续关联 ADR
**关键决策**：
1. **T1 gRPC 传输层**：tonic + proto 定义（ListManifests/GetManifest/ExecuteTool/ExecuteStream）+ 服务端骨架。proto 契约与 Anaphase-Helix 对齐，identity_labels/trace_id/seen_entropy_bloom 字段完整传递
2. **T2 MCP 传输层**：手动实现 JSON-RPC 2.0 over stdio（rmcp 的宏是编译时静态注册，不适合动态工具注册）。tools/list + tools/call 端点，_meta 扩展字段传递 identity_labels，inputSchema 从 Manifest parameters_schema 自动映射
3. **T3 生态对齐**：identity_labels 扩展字段（HTTP/gRPC/MCP 三传输层统一）+ 参数 Schema 校验（MCP tools/call 前校验 arguments）+ contract.md 更新（三传输层统一凭证流转规范）。CI-144 v2.0 PAL 冻结后自然接入，不阻塞 P4
4. **T4 插件热插拔**：PluginWatcher + notify crate（文件系统事件监听）+ 可选 `hot-reload` feature（默认不启用，保持 Tentacle 轻量）。新增/移除插件时，说明书索引实时更新，无需重启
5. **T5 全传输层集成测试**：新建 `tentacle-integration-tests` crate（publish=false，仅测试用），11 个测试覆盖 HTTP/gRPC/MCP 三传输层端到端 + 跨传输层一致性。关键修复：axum 版本对齐（0.7）、ExecutionRequest.tool 必填字段、manifest 索引响应格式（Vec<ManifestIndex> 数组）、MCP 可见性（handle_message/JsonRpcResponse 改为 pub）
**方法论闭环**：
- ADR-0001 追加决策 6-12（gRPC tonic、MCP 手动实现、_meta 扩展字段、Schema 校验、CI-144 对齐、插件热插拔、三传输层统一凭证流转）
- PLAN.md 实时更新（P4 进度表 + 阶段总览地图）
- 提交信息持续关联 ADR（`(ADR-0001 §Tx)`）
- GROWTH.md 追加 P4 记录，P2 记录归档至 `docs/archive/growth/2026-08-29-p2-foraging-sandbox.md`
**验收**：
- T1: 8 passed (gRPC)，manifest 索引 + execute + 未找到 NotFound
- T2: 8 passed (MCP)，tools/list + tools/call + _meta 扩展字段
- T3: 17 passed (MCP Schema 校验) + contract.md 更新
- T4: 32 passed (core，含 hot-reload feature)
- T5: 11 passed (integration)，HTTP/gRPC/MCP 端到端 + 跨传输层一致性
- P4 合计：76+ tests
**状态**：✅ P4 完成，进入 P5（性能优化 + 生产就绪）
---
## 记录 3：P5 阶段性进展——T1 性能基准 + T2 资源限制 + T3 可观测性（2026-08-30）
**变异类型**：P5 阶段 T1-T3 完成（T4-T5 待启动，阶段性记录）
**背景**：
- P5 目标：性能基准测试 + 资源限制 + 可观测性 + 部署文档 + STDIO 传输层
- 按 T1→T2→T3→T4→T5 串行执行，每 T 验证后再进下一个
- 方法论闭环：PLAN.md 实时更新状态，GROWTH.md 记录阶段性生长，提交信息持续关联 ADR-0001
**关键决策**：
1. **T1 性能基准框架**：选择 criterion（Rust 生态主流，支持异步基准 + 统计分析）。新建 `crates/tentacle-benchmarks/` crate（publish=false），覆盖核心层基线（direct execute + registry lookup）、HTTP 传输层（single + 10/50/100 并发）、gRPC 传输层（single）、MCP 传输层（tools_call + tools_list）。MockTool 用 SecurityLevel::Normal 避免共识审批阻塞基准测试。运行方式：`cargo bench --package tentacle-benchmarks`
2. **T2 统一资源限制**：建立 ResourceLimiter trait（极致解耦，与具体沙箱实现分离）。五维配额：内存/CPU时间/文件描述符/超时/输出大小。四种预设：default（50MB+5s+64FD+10s+1MB）、lightweight（16MB+1s+16FD+3s+256KB）、heavyweight（256MB+30s+256FD+60s+10MB）、unlimited（无限制）。AtomicResourceLimiter 用原子计数器实现，线程安全，内存峰值用 compare_exchange_weak 无锁更新。WASM 沙箱新增 `execute_with_quota()` + 输出大小检查；JS 沙箱新增 `execute_with_quota()` + worker 内存限制取 quota.memory_limit_bytes。向后兼容：`execute()` 接口不变
3. **T3 可观测性**：轻量 InMemoryMetrics（不依赖外部 metrics crate，按需加载，无运行时开销）。MetricsCollector trait 支持 Counter/Gauge/Histogram 三种指标类型，Prometheus 文本格式导出（含 HELP/TYPE 注释）。13 个标准指标常量：工具执行（executions_total/executions_failed_total/execution_duration_seconds）、工具状态（tools_active/tools_registered）、沙箱资源（memory_used_bytes/memory_peak_bytes/cpu_time_used_ms/fd_used）、传输层（requests_total/requests_failed_total/request_duration_seconds）。ToolExecutionMetrics 是 RAII 风格记录器，自动记录开始时间，`record_success()`/`record_failure()` 自动记录计数器和耗时直方图。record_resource_usage() 是 ResourceLimiter→Metrics 桥接函数。HTTP 传输层 AppState 新增 metrics 字段 + GET /metrics 端点（Content-Type: text/plain; version=0.0.4）+ execute_tool 自动记录成功/失败指标
**方法论闭环**：
- PLAN.md 实时更新（P5 进度表 T1-T3 标记 ✅，T4-T5 标记 ⏳，决策点 D1-D5 全部确认）
- GROWTH.md 追加 P5 阶段性记录（本记录）
- 提交信息持续关联 ADR（`(ADR-0001 §P5-Tx)`）
- RNA.md v1.3 方法论完整生效（PLAN 导航牌 + GROWTH 生长记录 + RNA 加载协议三层闭环）
**验收**：
- T1: tentacle-benchmarks crate 建立，criterion 框架集成，6 组基准测试（core_execute/core_registry/http_execute/http_concurrency/grpc_execute/mcp_protocol）
- T2: 14 passed (resource)，ResourceQuota/ResourceUsage/ResourceLimiter/AtomicResourceLimiter 单元测试
- T3: 12 passed (metrics)，Counter/Gauge/Histogram/导出格式/RAII记录器/资源桥接 单元测试
- P5 T1-T3 合计：26 新增测试，全 workspace 96 tests
- `cargo check --workspace` 全绿（含 runtime feature）
**状态**：🚧 P5 T1-T3 完成，T4（部署文档）待启动，T5（STDIO 传输层）待启动
---
