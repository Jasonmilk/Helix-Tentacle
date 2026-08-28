# Helix-Tentacle 生长记录
> **版本**：v1.1
> **日期**：2026-08-29
> **规则**：仅保留最近 3 条记录，超则归档至 `docs/archive/growth/`
> **归档策略**：历史随仓库版本化，永不删除

## 记录 1：P2 完成——觅食 + 沙箱（四修正全部兑现）（2026-08-29）
**变异类型**：P2 阶段完成
**背景**：
- P2 目标：渐进式觅食评估器 + 已见熵布隆过滤器 + WASM 沙箱 + JS 协程沙箱
- 四修正剩余 2/3 在 P2 落地（修正2 布隆过滤器、修正3 协程沙箱）
- 按 T1→T2→T3→T4 串行执行，每 T 验证后再进下一个
**关键决策**：
1. **T1 forager**：`ForagingEvaluator` 纯统计信息增益评估（Jaccard/N-Gram），0 Token。多维停止条件（LowInformationGain/MaxPagesReached/ZeroGainStreak）。中文 2-Gram 滑窗分词（零依赖）
2. **T2 布隆过滤器**：可选 `bloom` feature（bloomfilter crate），`seen_entropy_bloom: Option<Bloom<String>>`。命中布隆 → 增益计 0（全局记忆已覆盖）。Callosum 侧暂不实现（Callosum 待 Rust 重构），Tentacle 侧先定义接口
3. **T3 WASM 沙箱**：wasmtime v26 + epoch_deadline 超时（10ms 递增线程）。T3 简化为纯 WASM（不链接 WASI），符合"如无必要勿增实体"。wasmtime-wasi v26 API 复杂（WasiP1Ctx 无公开构造），WASI stdout/文件系统留到 P3
4. **T4 JS 协程沙箱**：rquickjs v0.5 + 独立线程 + 协变中断（set_interrupt_handler + AtomicBool）+ 内存限制（默认 50MB）。危险 API 移除（rquickjs 默认无 require/import/文件/网络）。关键修复：在 context.with 闭包内直接通过 channel 发送结果（闭包返回 ()），避免 rquickjs Exception(Value) 生命周期问题导致错误场景挂起
**四修正进度**：修正1✅ 修正2✅ 修正3✅ 修正4✅（全部兑现！）
**验收**：
- T1: 10 passed（forager 单元测试）
- T2: 14 passed (bloom) / 11 passed (无 bloom)，向后兼容
- T3: 7 passed (runtime) / 0 warning，纯 WASM 零外部访问验证
- T4: 10 passed (runtime) / 0 warning，协变中断 + 内存限制 + 危险 API 移除验证
- 全 workspace: 51 passed, 0 failed, 0 warning
**状态**：✅ P2 完成，四修正全部兑现，进入 P3（工具集成 + WASI 细化 + worker 池优化 + 首个内置工具）
---
## 记录 2：P3 完成——工具集成 + WASI 细化 + worker 池 + 首个内置工具 + 传输层集成（2026-08-29）
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
## 记录 3：预留
*（按 DNA v2.0 SOP，新记录追加至此，旧记录自动归档）*
