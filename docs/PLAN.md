# Helix-Tentacle 开发导航牌（PLAN）

> **版本**：v1.1（导航牌化，2026-08-28）
> **状态**：🚧 P1 Rust 重构（T4 完成，待提交）
> **分支**：rs
> **所属方法论**：DNA 自生长方法论 v2.0（PLAN 动态流转闭环）
> **规则**：本文件只含当前阶段 + 下一阶段预览 + 阶段总览地图。完成阶段 → GROWTH.md。总行数 ≤150，超出触发历史迁移。

---

## 1. 当前阶段：P1 — Rust 重构（核心骨架 + HTTP 传输层）

> **状态**：🚧 T4 代码完成（37 tests 全绿），待方法论补全后提交。

### 1.1 目标（基于白皮书 v3.4 代码真相源调研）

| 任务 | 内容 | 代码现状 | 状态 |
|---|---|---|---|
| T1 | workspace 骨架：9 crates（core/http/tools/transport-http/transport-mcp/transport-grpc/wasm/js/tentacle-bin） | ✅ 已建 | ✅ |
| T2 | tentacle-core 类型：Tool trait/Manifest/ToolRegistry/Redactor/ConsensusHook | ✅ 26 tests | ✅ |
| T3 | Manifest 完整性校验：SHA-256 + 插件目录扫描 + ScanReport + 渐进披露索引 | ✅ 含于 core 26 tests | ✅ |
| T4 | HTTP 传输层：axum 4 端点 + IdentityHttpClient 出网头（修正1） | ✅ 11 tests | ✅ 待提交 |

### 1.2 代码真相源（P1 调研结论）

- **core**：纯逻辑层，无网络依赖。`ExecutionRequest.identity_labels`（无明文 credentials），`ConsensusMode{Standalone,Helix,Mcp}`，`Redactor` 默认开启不可绕过
- **transport-http**：axum 0.7，4 端点。Critical 级工具走 ConsensusHook 审批，输出自动脱敏。SSE 流式简化为一次性输出（完整流式待 P2 工具实现）
- **tentacle-http**：reqwest 客户端，出网强制 `X-Identity-Label: key=value` 头。无标签时不添加
- **依赖**：core 用 sha2/hex/walkdir（纯 Rust）；transport 用 axum/tokio/tower/reqwest

### 1.3 四修正硬性验收（贯穿 P1-P2）

| 修正 | P1 状态 | P2 计划 |
|---|---|---|
| 1 凭证标签流转 | ✅ T2+T4 完成 | — |
| 2 已见熵布隆 | ⏳ 字段已定义 | P2 forager 实现 |
| 3 异步协程沙箱 | ⏳ crate 骨架已建 | P2 tentacle-js 实现 |
| 4 动态共识适配 | ✅ T2+T4 完成 | — |

### 1.4 入口 ADR

- **ADR-0001**：Tentacle Rust 重构 + 四修正 + 方法论迁移（Active）

### 1.5 验收标准

- ✅ `cargo test --workspace` 全绿 + 0 warning（37 passed）
- ✅ tentacle-core 无明文凭证字段（grep 检查）
- ✅ Manifest SHA-256 完整性校验测试通过
- ✅ HTTP 传输层：说明书索引 + 执行 + SSE 流式
- ✅ identity_label 出网头验证
- ⏳ 方法论闭环：RNA.md/DEPRECATE.md 补全，GROWTH 记录，提交信息含 ADR 关联

### 1.6 下一阶段预览：P2 — 觅食 + 沙箱 + 更多传输

- 渐进式觅食评估器（forager.rs）+ 已见熵布隆（修正2）
- JS 协程沙箱（修正3）+ WASM（wasmtime）
- gRPC/MCP/STDIO 传输层
- 内置工具集（targeted_scraper 等）

---

## 2. 阶段总览（地图，不展开）

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | Python 早期版本（哲学参考，main 分支保留） | ✅ 历史 |
| **P1** | **Rust 重构：核心骨架 + HTTP 传输层（T1-T4）** | **🚧 进行中** |
| P2 | 觅食 + 沙箱 + 更多传输层（修正2/3 落地） | ⏳ 预览 |
| P3 | 完整工具集 + 插件热插拔 + 性能优化 | ⏳ 预览 |

---

## 3. 活跃决策与契约指针（不展开）

| 项 | 指针 |
|---|---|
| 四修正 | ADR-0001（凭证标签/布隆/协程沙箱/共识适配） |
| 白皮书 | `docs/SPEC.md` + `docs/spec/*.md`（5 分卷） |
| 生态对齐 | Anaphase-Helix gRPC 契约 + Helix-Mind 认知工艺 |
| CI-144 | 内部优先 gRPC（CI-144 语义），外部 HTTP/MCP/STDIO 通用 |
| Tuck 边界 | 明文凭证永不在 Tentacle 内存，Tuck 物理边缘注入 |

---

## 4. 文档生态 SOP（DNA v2.0）

PLAN 是导航牌不是历史档案；阶段收尾时（收尾 SLA：24h）完成记录追加 GROWTH.md 并从 PLAN 移除；GROWTH ≤3 条超则归档；PLAN ≤150 行超则触发历史迁移。提交信息必须包含 ADR 关联 `(ADR-NNNN §Tx)`。详见 `docs/DNA.md`「文档生态 SOP」和 `docs/RNA.md`「加载协议」。
