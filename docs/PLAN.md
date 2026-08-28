# Helix-Tentacle PLAN — 当前阶段导航

> **DNA 方法论 v2.0** ｜ PLAN.md 是导航牌，不是历史档案（≤150 行）。完成记录进 GROWTH.md。

## 当前阶段：P1 — Rust 重构（核心骨架）

**目标**：基于 DNA 方法论构建 Rust 版 Tentacle。按白皮书 v3.4 crates 结构搭建，四修正为硬性验收。

### 任务清单

| # | 任务 | 内容 | 入口 |
|---|---|---|---|
| T1 | workspace 骨架 | crates 结构：core / tools / transport-http / transport-mcp / transport-grpc / tentacle（二进制） | ADR-0001 |
| T2 | tentacle-core | Tool trait / Manifest / ToolRegistry / 安全等级 / 脱敏管道（无网络依赖，<500KB） | 白皮书 T01 |
| T3 | Manifest + 完整性校验 | 声明与执行体分离，SHA-256 绑定，渐进披露（说明书索引） | 白皮书 T02 |
| T4 | 传输层 HTTP | axum RESTful + SSE 流式；`identity_label` 出网头（修正 1） | 白皮书 T03/3.2 |

### 四修正硬性验收（贯穿）

| 修正 | 验收标准 |
|---|---|
| 1 凭证标签流转 | `tentacle-core` 无明文 credentials 字段；`tentacle-http` 出网强制 `X-Identity-Label`；内存 grep 不出明文 Cookie/Token |
| 2 已见熵布隆过滤器 | `forager.rs` 接收 `seen_entropy_bloom: Option<BloomFilter>`（Callosum 导出，可选 feature） |
| 3 异步协程沙箱 | JS 沙箱固定 worker 池 + 协程协作终止（ARM 端侧） |
| 4 动态共识适配 | `ConsensusMode{Standalone/Helix/MCP}`；Standalone 终端确认不返回 503 |

### 技术前提

- 白皮书 v3.4（`docs/vision/tentacle-whitepaper-v3.4.md`）为设计定案
- Anaphase gRPC 契约（tentacle proto 已在 anaphase-helix 同步）
- Rust workspace（tokio/axum/tonic/serde/petgraph）

### 风险与注意事项

- **勿增实体**：P1 只做核心骨架 + HTTP 传输，MCP/gRPC 传输层、WASM/JS 沙箱渐进（勿一次铺开）
- **依赖注入**：HTTP 共识钩子/布隆过滤器通过 Trait 注入，Core 不硬编码
- **四修正优先**：凭证标签流转是安全红线，T1-T4 落地即遵循

### 验收标准

- `cargo check --workspace` 通过（P1 结束时）
- tentacle-core 无明文凭证字段（修正 1 静态检查）
- Manifest 完整性校验（SHA-256）测试通过
- HTTP 传输层：说明书索引 + 执行 + SSE 流式
- 方法论闭环：ADR 追加、GROWTH 记录、spec 同步

## 下一阶段预览：P2 — 觅食 + 沙箱 + 更多传输

- 渐进式觅食评估器 + 已见熵布隆（修正 2）
- JS 协程沙箱（修正 3）+ WASM（wasmtime）
- 动态共识适配（修正 4）+ gRPC/MCP/STDIO 传输层

---

*Helix-Tentacle PLAN v1.0（Rust 重构启动，2026-08-28）*
