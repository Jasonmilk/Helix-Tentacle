# Helix-Tentacle 开发导航牌（PLAN）

> **版本**：v1.5（P3-T2 完成，T3 待启动，2026-08-29）
> **状态**：🚧 P3 工具集成（T1✅ WASI 细化，T2✅ worker 池，T3⏳ 工具执行引擎）
> **分支**：rs
> **所属方法论**：DNA 自生长方法论 v2.0（PLAN 动态流转闭环）
> **规则**：本文件只含当前阶段 + 下一阶段预览 + 阶段总览地图。完成阶段 → GROWTH.md。总行数 ≤150，超出触发历史迁移。

---

## 1. 当前阶段：P3 — 工具集成 + WASI 细化 + worker 池优化

> **状态**：🚧 T1 已完成（WASI 细化），T2 待用户确认后启动（worker 池优化）。

### 1.1 目标（基于白皮书 v3.4 + P2 遗留细化）

| 任务 | 内容 | 入口 | 状态 |
|---|---|---|---|
| T1 | WASI 细化：WASM 沙箱接入 wasmtime-wasi（stdout 捕获 + 文件系统权限按 Manifest 配置） | P2-T3 遗留 | ✅ 完成（d095766，10 tests） |
| T2 | worker 池优化：JS 沙箱从"每次独立线程"升级为"固定 worker 池 + 多 context 单线程协程调度" | P2-T4 遗留/白皮书 §6.3 | ✅ 完成（14 tests，0.16s） |
| T3 | 工具执行引擎集成：Manifest 完整性校验 + 插件目录扫描 + 渐进披露索引与执行层对接（WASM/JS 插件加载执行） | P1-T3 + 白皮书 §3.2 | ⏳ |
| T4 | 首个内置工具 `targeted_scraper`：定向爬取 + 渐进式觅食 + 布隆过滤器端到端验证 | 白皮书 §3.4/附录 A | ⏳ |
| T5 | 传输层集成测试：HTTP/MCP/gRPC 传输层调用工具执行引擎的完整链路测试 | P1-T4 + 白皮书 §3.2 | ⏳ |

### 1.2 代码真相源（P3 调研结论 + T1 完成状态）

- **WASI 细化（T1 ✅ 已完成）**：已接入 wasmtime-wasi v26 preview1。stdout 用 MemoryOutputPipe 捕获到内存（可 clone，执行后 contents() 获取）；文件系统用 preopened_dir 按 manifest.permissions.filesystem 配置（支持 read:/write:/rw: 前缀）；未声明目录无法访问，越权即拒绝。Store data = WasiP1Ctx，preview1::add_to_linker_sync 链接 WASI 函数
- **worker 池（T2 ⏳ 待启动）**：P2-T4 已实现 JS 沙箱（rquickjs + 独立线程 + 协变中断），但每次执行创建新线程。白皮书 §6.3 要求"固定 worker 池（默认绑定物理核）+ QuickJS 多 context 单线程协程调度"，需在 P3-T2 升级
- **工具执行引擎（T3 ⏳）**：P1-T3 已实现 Manifest 完整性校验 + 插件目录扫描 + 渐进披露索引，但未与执行层（WASM/JS 沙箱）对接。P3-T3 需实现 `ToolExecutor`：根据 Manifest 类型加载 WASM/JS 执行体，调用沙箱执行
- **targeted_scraper（T4 ⏳）**：白皮书 §3.4 已有完整设计，ForagingEvaluator 已在 P2-T1 实现。P3-T4 需实现 HTTP 客户端（tentacle-http crate）+ 渐进式觅食集成 + 布隆过滤器对接
- **传输层集成（T5 ⏳）**：P1-T4 已实现 HTTP 传输层（axum 四端点），但 execute 端点未对接真实工具执行引擎。P3-T5 需实现端到端链路

### 1.3 四修正状态（全部兑现）

| 修正 | 状态 | 落地位置 |
|---|---|---|
| 1 凭证标签流转 | ✅ | P1-T2 core + P1-T4 transport-http |
| 2 已见熵布隆过滤器 | ✅ | P2-T2（可选 bloom feature） |
| 3 异步协程沙箱 | ✅ | P2-T4（JS 协变中断 + 独立线程），P3-T2 升级为 worker 池 |
| 4 动态共识适配 | ✅ | P1-T2 core + P1-T4 transport-http |

### 1.4 入口 ADR

- **ADR-0001**：Tentacle Rust 重构 + 四修正 + 方法论迁移（Active，已覆盖 P3 技术选型大方向，无需额外 ADR）

### 1.5 已确认决策点（D1-D4 全部通过）

| # | 决策点 | 决议 | 状态 |
|---|---|---|---|
| D1 | P3 T 拆分粒度 | T1→T2→T3→T4→T5 串行，每 T 验证后再进下一个 | ✅ 已确认 |
| D2 | WASI 实现方式 | wasmtime-wasi v26 preview1 API（stdout 捕获 + preopened_dir 按 Manifest permissions 配置） | ✅ 已确认（T1 已落地） |
| D3 | worker 池大小 | 默认与物理核数绑定（`std::thread::available_parallelism`），可配置 | ✅ 已确认（T2 落地） |
| D4 | targeted_scraper HTTP 客户端 | tentacle-http crate（已存在骨架），支持 TLS 指纹模拟 + 反检测策略 | ✅ 已确认（T4 落地） |

### 1.6 验收标准

- T1：WASM 沙箱接入 wasmtime-wasi，stdout 捕获测试通过，文件系统权限按 Manifest 配置（越权拒绝）
- T2：JS worker 池固定大小，多 context 单线程协程调度，10 并发不 OOM，协变中断超时终止
- T3：ToolExecutor 加载 WASM/JS 插件执行，Manifest 完整性校验通过，渐进披露索引与执行层对接
- T4：targeted_scraper 端到端验证，渐进式觅食正常终止，布隆过滤器防重复采集
- T5：HTTP 传输层 execute 端点调用真实工具执行引擎，完整链路测试通过
- `cargo test --workspace` 全绿 + 0 warning

### 1.7 下一阶段预览：P4 — 生态对齐 + 性能优化 + 更多传输层

- gRPC 传输层实现（tonic，对接 Anaphase-Helix）
- MCP 传输层实现（rmcp，对接 Claude Desktop 等通用生态）
- 性能基准测试（ARM 端侧，100 并发延迟/吞吐）
- 插件热插拔（文件系统监听，Manifest 即时注册/注销）
- 与 Anaphase-Helix 端到端联调（gRPC 契约对齐）

---

## 2. 阶段总览（地图，不展开）

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | Python 早期版本（哲学参考，main 分支保留） | ✅ 历史 |
| P1 | Rust 重构：核心骨架 + HTTP 传输层（T1-T4） | ✅ 2026-08-28（37 tests） |
| P2 | 觅食 + 沙箱（修正2/3 落地，T1-T4） | ✅ 2026-08-29（51 tests，四修正全部兑现） |
| **P3** | **工具集成 + WASI 细化 + worker 池优化 + 首个内置工具** | **🚧 进行中** |
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
