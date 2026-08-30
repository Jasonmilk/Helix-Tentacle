# Helix-Tentacle 开发导航牌（PLAN）

> **版本**：v4.1（P5 T1-T3 完成，T4 待启动，2026-08-30）
> **状态**：🚧 P5 性能优化 + 生产就绪（T1-T3 ✅，T4 ⏳）
> **上一阶段**：P4 ✅ 生态对齐 + gRPC/MCP 传输层 + 插件热插拔（2026-08-29，76+ tests）
> **分支**：rs
> **所属方法论**：phyt-DNA v1.0（PLAN 动态流转闭环，方法论锚点项目 https://github.com/Jasonmilk/phyt-DNA）
> **规则**：本文件只含当前阶段 + 下一阶段预览 + 阶段总览地图。完成阶段 → GROWTH.md。总行数 ≤150，超出触发历史迁移。

---

## 1. 当前阶段：P5 — 性能优化 + 生产就绪

> **状态**：🚧 T1-T3 完成，T4 待启动（部署文档）。
> **前置依赖**：CI-144 v2.0 已冻结（PFP-xCF14 + SAP-xCF14），Tuck 重构已完成，P5 可独立推进。

### 1.1 目标（基于白皮书 v3.4 + 生产就绪需求）

| 任务 | 内容 | 入口 | 状态 |
|---|---|---|---|
| T1 | 性能基准测试：criterion + HTTP/gRPC/MCP 三传输层延迟/吞吐/并发对比 | 白皮书 §性能 / 生产就绪 | ✅ |
| T2 | 资源限制：ResourceLimiter trait + 内存/CPU/FD/超时/输出五维配额 + WASM/JS沙箱集成 | 白皮书 §6.3 沙箱安全 | ✅ |
| T3 | 可观测性：MetricsCollector trait + InMemoryMetrics + HTTP /metrics端点 + 工具执行指标自动记录 | 白皮书 §白盒可观测 | ✅ |
| T4 | 生产部署文档：Docker/K8s/systemd，配置最佳实践 | 生产就绪 | ⏳ |
| T5 | STDIO 传输层实现：零配置本地使用（echo JSON → 执行 → 输出结果） | 白皮书 §3.2 STDIO 模式 | ⏳ |

### 1.2 代码真相源（P5 T1-T3 完成，T4-T5 待启动）

- **性能基准（T1 ✅）**：`crates/tentacle-benchmarks/`，criterion 框架，覆盖核心层/HTTP/gRPC/MCP。详见 GROWTH.md 记录 3
- **资源限制（T2 ✅）**：`crates/tentacle-core/src/resource.rs`，ResourceLimiter trait + 五维配额 + WASM/JS集成。详见 GROWTH.md 记录 3
- **可观测性（T3 ✅）**：`crates/tentacle-core/src/metrics.rs`，MetricsCollector trait + InMemoryMetrics + HTTP /metrics端点。详见 GROWTH.md 记录 3
- **部署文档（T4 ⏳）**：待编写 Dockerfile/K8s/systemd + 配置最佳实践
- **STDIO 传输层（T5 ⏳）**：`crates/tentacle/` 二进制入口待实现，echo JSON → 执行 → 输出结果

### 1.3 四修正状态（全部兑现，P5 持续维护）

| 修正 | 状态 | 落地位置 |
|---|---|---|
| 1 凭证标签流转 | ✅ | P1-T2 core + P1-T4 transport-http + P4-T1 gRPC + P4-T2 MCP |
| 2 已见熵布隆过滤器 | ✅ | P2-T2（可选 bloom feature） |
| 3 异步协程沙箱 | ✅ | P2-T4 + P3-T2 worker 池 |
| 4 动态共识适配 | ✅ | P1-T2 core + P1-T4 transport-http + P4-T2 MCP |

### 1.4 入口 ADR

- **ADR-0001**：Tentacle Rust 重构 + 四修正 + 方法论迁移（Active，已覆盖 P1-P5 技术选型大方向）
- **P5 决策记录**：T1 criterion 框架选型、T2 ResourceLimiter trait 设计、T3 MetricsCollector trait + InMemoryMetrics 轻量实现（均已追加至 ADR-0001）

### 1.5 已确认决策点（P5 D1-D5 全部确认）

| # | 决策点 | 决议 | 状态 |
|---|---|---|---|
| D1 | P5 T 拆分粒度 | T1→T2→T3→T4→T5 串行，每 T 验证后再进下一个 | ✅ 已确认 |
| D2 | 性能基准框架 | criterion（Rust 生态主流，支持异步基准 + 统计分析） | ✅ 已确认 |
| D3 | 可观测性方案 | 轻量 InMemoryMetrics（不依赖外部 crate），Prometheus 格式导出，未来可接入 OpenTelemetry | ✅ 已确认 |
| D4 | STDIO 传输层 | 复用 core + 现有传输层逻辑，独立二进制入口 | ✅ 已确认 |
| D5 | CI-144 v2.0 接入时机 | PAL 已冻结，Tuck 重构已完成，P5 可独立推进，CI-144 接入放 P6 | ✅ 已确认 |

### 1.6 P5 进度（T1-T3 完成）

| 任务 | 内容 | 状态 | 测试 |
|---|---|---|---|
| T1 | 性能基准测试（criterion + 三传输层对比） | ✅ 完成 | - |
| T2 | 资源限制（ResourceLimiter trait + 五维配额 + WASM/JS集成） | ✅ 完成 | 14 |
| T3 | 可观测性（MetricsCollector + /metrics端点 + 自动指标记录） | ✅ 完成 | 12 |
| T4 | 生产部署文档（Docker/K8s/systemd） | ⏳ 待启动 | - |
| T5 | STDIO 传输层实现 | ⏳ 待启动 | - |

### 1.7 验收标准

- T1：✅ 性能基准测试框架建立，HTTP/gRPC/MCP 三传输层延迟/吞吐/并发基准可复现
- T2：✅ ResourceLimiter trait 建立，五维配额可配置，WASM/JS 沙箱集成，越权即拒绝
- T3：✅ Prometheus metrics 端点可用（/metrics），工具执行自动记录成功/失败计数器和耗时直方图
- T4：Dockerfile + K8s manifest + systemd service 可用，配置最佳实践文档完整
- T5：STDIO 模式可用（echo JSON → 执行 → 输出结果），零配置本地使用
- `cargo test --workspace` 全绿 + 0 warning

### 1.8 下一阶段预览：P6 — 生态全组件联调 + CI-144 v2.0 接入

- 与 Helix 生态全组件端到端联调（Mind + Anaphase + Tuck + Cellrix + Callosum）
- CI-144 v2.0（PFP-xCF14 + SAP-xCF14）接入：4 字节固定偏移 PFP 头部，Modality/Risk-Level/Override-Flag
- Tuck 重构对接：凭证标签流转完整闭环（Tentacle → Tuck → 公网）
- Cellrix 观测对接：工具执行状态实时展示
- 生产环境灰度发布

---

## 2. 阶段总览（地图，不展开）

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | Python 早期版本（哲学参考，main 分支保留） | ✅ 历史 |
| P1 | Rust 重构：核心骨架 + HTTP 传输层（T1-T4） | ✅ 2026-08-28（37 tests） |
| P2 | 觅食 + 沙箱（修正2/3 落地，T1-T4） | ✅ 2026-08-29（51 tests，四修正全部兑现） |
| P3 | 工具集成 + WASI 细化 + worker 池 + 首个内置工具 | ✅ 2026-08-29（72 tests，P3 收官） |
| P4 | 生态对齐 + gRPC/MCP 传输层 + 插件热插拔 | ✅ 2026-08-29（76+ tests，P4 收官） |
| **P5** | **性能优化 + 生产就绪（基准测试/资源限制/可观测性/部署文档/STDIO）** | **🚧 T1-T3 完成，T4 待启动（96 tests）** |
| P6 | 生态全组件联调 + CI-144 v2.0 接入 | ⏳ 预览 |

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
| 传输层 | P1-T4 HTTP + P4-T1 gRPC + P4-T2 MCP（三传输层平等） |
| 全传输层集成 | P4-T5（11 tests，跨传输层一致性验证） |
| 性能基准 | P5-T1（tentacle-benchmarks crate + criterion） |
| 资源限制 | P5-T2（ResourceLimiter trait + 五维配额 + WASM/JS集成） |
| 可观测性 | P5-T3（MetricsCollector trait + InMemoryMetrics + HTTP /metrics端点） |
| 生态对齐 | Anaphase-Helix gRPC 契约 + Helix-Mind 认知工艺 + Callosum 布隆导出 |
| Tuck 边界 | 明文凭证永不在 Tentacle 内存，Tuck 物理边缘注入 |
| CI-144 v2.0 | PFP-xCF14 + SAP-xCF14 已冻结，P6 接入（不阻塞 P5） |

---

## 4. 文档生态 SOP（DNA v2.0）

PLAN 是导航牌不是历史档案；阶段收尾时（收尾 SLA：24h）完成记录追加 GROWTH.md 并从 PLAN 移除；GROWTH ≤3 条超则归档；PLAN ≤150 行超则触发历史迁移。提交信息必须包含 ADR 关联 `(ADR-NNNN §Tx)`。详见 `docs/DNA.md`「文档生态 SOP」和 `docs/RNA.md`「加载协议」。
