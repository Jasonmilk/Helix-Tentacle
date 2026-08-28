# Helix-Tentacle 开发导航牌（PLAN）

> **版本**：v3.0（P4 启动，gRPC/MCP 传输层 + 生态对齐，2026-08-29）
> **状态**：🚧 P4 生态对齐 + gRPC/MCP 传输层实现（T1⏳ gRPC 传输层，T2⏳ MCP 传输层，T3⏳ 生态对齐，T4⏳ 插件热插拔，T5⏳ 全传输层集成测试）
> **上一阶段**：P3 ✅ 工具集成全部完成（2026-08-29）
> **分支**：rs
> **所属方法论**：DNA 自生长方法论 v2.0（PLAN 动态流转闭环）
> **规则**：本文件只含当前阶段 + 下一阶段预览 + 阶段总览地图。完成阶段 → GROWTH.md。总行数 ≤150，超出触发历史迁移。

---

## 1. 当前阶段：P4 — 生态对齐 + gRPC/MCP 传输层实现

> **状态**：🚧 T1 待启动（gRPC 传输层基础实现）。

### 1.1 目标（基于白皮书 v3.4 + 生态对齐需求）

| 任务 | 内容 | 入口 | 状态 |
|---|---|---|---|
| T1 | gRPC 传输层：tonic + proto 定义 + 服务端骨架（manifest 索引 + execute 端点），对接 Anaphase-Helix | 白皮书 §3.2 / 生态对齐 | ⏳ |
| T2 | MCP 传输层：rmcp + stdio/HTTP/SSE（tools/list + tools/call），对接 Claude Desktop 等通用生态 | 白皮书 §3.2 / 通用生态 | ⏳ |
| T3 | 生态对齐：Anaphase-Helix gRPC 契约对齐 + CI-144 语义对齐 + 凭证标签流转验证 | 生态对齐需求 | ⏳ |
| T4 | 插件热插拔：文件系统监听（notify crate）+ Manifest 即时注册/注销 + 渐进披露索引实时更新 | 白皮书 §3.6 | ⏳ |
| T5 | 全传输层集成测试：HTTP/gRPC/MCP 三传输层端到端测试 + 跨传输层一致性验证 | P1-T4 + P4-T1/T2 | ⏳ |

### 1.2 代码真相源（P3 完成状态 + P4 调研）

- **gRPC 传输层（T1 ⏳）**：当前 `crates/tentacle-transport-grpc/` 为空壳（无依赖无实现）。需基于 tonic 实现，proto 定义参考白皮书 §3.2（manifest 索引 + execute + execute_stream）。Anaphase-Helix 侧 gRPC 契约需对齐（CI-144 语义）
- **MCP 传输层（T2 ⏳）**：当前 `crates/tentacle-transport-mcp/` 为空壳。需基于 rmcp 实现，支持 stdio/HTTP/SSE 三种传输。MCP 协议自动将工具说明书映射为 tools/list 和 tools/call
- **生态对齐（T3 ⏳）**：Anaphase-Helix 通过 gRPC 调用 Tentacle，需对齐 proto 契约。CI-144 v2.0（PAL）仍处于"审查通过、等待冻结"状态，不阻塞 P4，v2.0 冻结后自然接入
- **插件热插拔（T4 ⏳）**：白皮书 §3.6 要求文件系统监听自动检测新增/移除插件。需引入 notify crate，监听 plugins/ 目录，新插件 Manifest 即时出现在说明书索引中
- **全传输层集成（T5 ⏳）**：HTTP（P1-T4 已实现）+ gRPC（T1）+ MCP（T2）三传输层共享同一个 ToolRegistry，无状态冲突。需验证跨传输层调用同一工具的一致性

### 1.3 四修正状态（全部兑现，P4 持续维护）

| 修正 | 状态 | 落地位置 |
|---|---|---|
| 1 凭证标签流转 | ✅ | P1-T2 core + P1-T4 transport-http + P4-T1 gRPC |
| 2 已见熵布隆过滤器 | ✅ | P2-T2（可选 bloom feature） |
| 3 异步协程沙箱 | ✅ | P2-T4 + P3-T2 worker 池 |
| 4 动态共识适配 | ✅ | P1-T2 core + P1-T4 transport-http + P4-T2 MCP |

### 1.4 入口 ADR

- **ADR-0001**：Tentacle Rust 重构 + 四修正 + 方法论迁移（Active，已覆盖 P1-P3 技术选型大方向）
- **P4 新增 ADR 候选**：gRPC/MCP 传输层契约设计（如与 Anaphase-Helix 契约有重大偏离，需新建 ADR-0002）

### 1.5 已确认决策点（P4 D1-D5 待确认）

| # | 决策点 | 决议 | 状态 |
|---|---|---|---|
| D1 | P4 T 拆分粒度 | T1→T2→T3→T4→T5 串行，每 T 验证后再进下一个 | ⏳ 待确认 |
| D2 | gRPC 框架选型 | tonic（Rust 生态主流，与 Anaphase-Helix 一致） | ⏳ 待确认 |
| D3 | MCP 框架选型 | rmcp（Rust 原生 MCP 实现） | ⏳ 待确认 |
| D4 | 插件热插拔实现 | notify crate（文件系统事件监听） | ⏳ 待确认 |
| D5 | CI-144 接入时机 | v2.0 PAL 冻结后自然接入，不阻塞 P4 | ⏳ 待确认 |

### 1.6 验收标准

- T1：gRPC 传输层服务端启动，manifest 索引 + execute 端点可用，proto 契约与 Anaphase-Helix 对齐
- T2：MCP 传输层 stdio/HTTP/SSE 三种传输可用，tools/list + tools/call 端点正常
- T3：Anaphase-Helix 通过 gRPC 调用 Tentacle 工具，凭证标签流转正确，CI-144 语义对齐
- T4：plugins/ 目录新增/移除插件时，说明书索引实时更新，无需重启
- T5：HTTP/gRPC/MCP 三传输层调用同一工具结果一致，跨传输层无状态冲突
- `cargo test --workspace` 全绿 + 0 warning

### 1.7 下一阶段预览：P5 — 性能优化 + 生产就绪

- 性能基准测试（ARM 端侧，100 并发延迟/吞吐）
- 资源限制（内存/CPU/文件描述符配额）
- 生产部署文档（Docker/K8s/systemd）
- 可观测性（Prometheus metrics + OpenTelemetry tracing）
- 与 Helix 生态全组件端到端联调（Mind + Anaphase + Tuck + Cellrix）

---

## 2. 阶段总览（地图，不展开）

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | Python 早期版本（哲学参考，main 分支保留） | ✅ 历史 |
| P1 | Rust 重构：核心骨架 + HTTP 传输层（T1-T4） | ✅ 2026-08-28（37 tests） |
| P2 | 觅食 + 沙箱（修正2/3 落地，T1-T4） | ✅ 2026-08-29（51 tests，四修正全部兑现） |
| P3 | 工具集成 + WASI 细化 + worker 池 + 首个内置工具 | ✅ 2026-08-29（72 tests，P3 收官） |
| **P4** | **生态对齐 + gRPC/MCP 传输层 + 插件热插拔** | **🚧 进行中** |
| P5 | 性能优化 + 生产就绪（基准测试/可观测性/部署文档） | ⏳ 预览 |
| P4 | 生态对齐 + 性能优化 + 更多传输层（gRPC/MCP） | ⏳ 预览 |

---

## 3. 活跃决策与契约指针（不展开）

| 项 | 指针 |
|---|---|
| 四修正 | ADR-0001（全部✅，P3-T2 升级 worker 池） |
| 白皮书 | `docs/SPEC.md` + `docs/spec/*.md`（5 分卷） |
| 觅食设计 | 白皮书 §3.4（ForagingEvaluator，P2-T1 已实现） |
| 沙箱设计 | 白皮书 §6.3（WASM epoch_deadline + JS 协变中断，P2-T3/T4 已实现） |
| WASI 细化 | P3-T1（wasmtime-wasi v26 preview1 API） |
| worker 池 | P3-T2（固定 worker 池 + 多 context 单线程协程调度） |
| 工具执行引擎 | P3-T3（Manifest 完整性校验 + 插件加载执行） |
| targeted_scraper | P3-T4（HTTP 客户端 + 渐进式觅食 + 布隆过滤器） |
| 生态对齐 | Anaphase-Helix gRPC 契约 + Helix-Mind 认知工艺 + Callosum 布隆导出 |
| Tuck 边界 | 明文凭证永不在 Tentacle 内存，Tuck 物理边缘注入 |

---

## 4. 文档生态 SOP（DNA v2.0）

PLAN 是导航牌不是历史档案；阶段收尾时（收尾 SLA：24h）完成记录追加 GROWTH.md 并从 PLAN 移除；GROWTH ≤3 条超则归档；PLAN ≤150 行超则触发历史迁移。提交信息必须包含 ADR 关联 `(ADR-NNNN §Tx)`。详见 `docs/DNA.md`「文档生态 SOP」和 `docs/RNA.md`「加载协议」。
