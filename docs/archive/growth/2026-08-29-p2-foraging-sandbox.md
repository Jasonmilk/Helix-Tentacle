# P2 完成——觅食 + 沙箱（四修正全部兑现）

> **日期**：2026-08-29
> **归档自**：GROWTH.md 记录 1
> **变异类型**：P2 阶段完成

## 背景
- P2 目标：渐进式觅食评估器 + 已见熵布隆过滤器 + WASM 沙箱 + JS 协程沙箱
- 四修正剩余 2/3 在 P2 落地（修正2 布隆过滤器、修正3 协程沙箱）
- 按 T1→T2→T3→T4 串行执行，每 T 验证后再进下一个

## 关键决策
1. **T1 forager**：`ForagingEvaluator` 纯统计信息增益评估（Jaccard/N-Gram），0 Token。多维停止条件（LowInformationGain/MaxPagesReached/ZeroGainStreak）。中文 2-Gram 滑窗分词（零依赖）
2. **T2 布隆过滤器**：可选 `bloom` feature（bloomfilter crate），`seen_entropy_bloom: Option<Bloom<String>>`。命中布隆 → 增益计 0（全局记忆已覆盖）。Callosum 侧暂不实现（Callosum 待 Rust 重构），Tentacle 侧先定义接口
3. **T3 WASM 沙箱**：wasmtime v26 + epoch_deadline 超时（10ms 递增线程）。T3 简化为纯 WASM（不链接 WASI），符合"如无必要勿增实体"。wasmtime-wasi v26 API 复杂（WasiP1Ctx 无公开构造），WASI stdout/文件系统留到 P3
4. **T4 JS 协程沙箱**：rquickjs v0.5 + 独立线程 + 协变中断（set_interrupt_handler + AtomicBool）+ 内存限制（默认 50MB）。危险 API 移除（rquickjs 默认无 require/import/文件/网络）。关键修复：在 context.with 闭包内直接通过 channel 发送结果（闭包返回 ()），避免 rquickjs Exception(Value) 生命周期问题导致错误场景挂起

## 四修正进度
修正1✅ 修正2✅ 修正3✅ 修正4✅（全部兑现！）

## 验收
- T1: 10 passed（forager 单元测试）
- T2: 14 passed (bloom) / 11 passed (无 bloom)，向后兼容
- T3: 7 passed (runtime) / 0 warning，纯 WASM 零外部访问验证
- T4: 10 passed (runtime) / 0 warning，协变中断 + 内存限制 + 危险 API 移除验证
- 全 workspace: 51 passed, 0 failed, 0 warning

## 状态
✅ P2 完成，四修正全部兑现，进入 P3（工具集成 + WASI 细化 + worker 池优化 + 首个内置工具）
