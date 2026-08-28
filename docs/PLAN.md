# Helix-Tentacle 开发导航牌（PLAN）

> **版本**：v1.2（P1→P2 流转，2026-08-28）
> **状态**：🚧 P2 觅食 + 沙箱（T1 待启动）
> **分支**：rs
> **所属方法论**：DNA 自生长方法论 v2.0（PLAN 动态流转闭环）
> **规则**：本文件只含当前阶段 + 下一阶段预览 + 阶段总览地图。完成阶段 → GROWTH.md。总行数 ≤150，超出触发历史迁移。

---

## 1. 当前阶段：P2 — 觅食 + 沙箱（修正2/3 落地）

> **状态**：🚧 规划中，T1 待用户确认后启动。

### 1.1 目标（基于白皮书 v3.4 + 四修正）

| 任务 | 内容 | 入口 | 状态 |
|---|---|---|---|
| T1 | forager.rs：渐进式觅食评估器（纯统计 Jaccard/TF-IDF，0 Token，多维停止条件） | 白皮书 §3.4/T07 | ⏳ 待启动 |
| T2 | 已见熵布隆过滤器（修正2，可选 feature，对接 Callosum 导出） | 四修正2 | ⏳ |
| T3 | WASM 沙箱（wasmtime，epoch_deadline 超时，WASI 能力配置） | 白皮书 §6.3/T05 | ⏳ |
| T4 | JS 协程沙箱（修正3，QuickJS + 固定 worker 池 + 协变中断，ARM 端侧） | 四修正3/白皮书 §6.3 | ⏳ |

### 1.2 代码真相源（P2 调研结论）

- **tentacle-tools**：当前为空 crate骨架。forager.rs 需新建，依赖 core 的 Manifest/ForagingConfig
- **tentacle-wasm**：当前为空 crate骨架。需引入 wasmtime（可选 feature，避免默认依赖膨胀）
- **tentacle-js**：当前为空 crate骨架。需引入 quickjs（或 rquickjs），固定 worker 池大小与 CPU 核心数绑定
- **布隆过滤器**：需选择纯 Rust 实现（如 `bloomfilter` crate），作为可选 feature，core 不硬编码
- **觅食评估器**：白皮书 §3.4 已有完整 Rust 伪代码，ForagingEvaluator 结构 + evaluate_gain 方法 + ForageDecision/ForageStopReason 枚举

### 1.3 四修正硬性验收（P2 落地修正2/3）

| 修正 | P1 状态 | P2 计划 |
|---|---|---|
| 1 凭证标签流转 | ✅ T2+T4 完成 | — |
| 2 已见熵布隆 | ⏳ 字段已定义 | T2 实现（forager 接收 seen_entropy_bloom） |
| 3 异步协程沙箱 | ⏳ crate 骨架已建 | T4 实现（JS worker 池 + 协变中断） |
| 4 动态共识适配 | ✅ T2+T4 完成 | — |

### 1.4 入口 ADR

- **ADR-0001**：Tentacle Rust 重构 + 四修正 + 方法论迁移（Active，已覆盖 P2 技术选型大方向，无需额外 ADR）

### 1.5 待用户审查的决策点

| # | 决策点 | 建议 | 状态 |
|---|---|---|---|
| D1 | WASM 运行时选型 | wasmtime（功能全，epoch_deadline 原生支持）vs wasmi（纯 Rust，更轻量）。建议：wasmtime 作为默认 feature，wasmi 作为可选 | 待确认 |
| D2 | JS 引擎选型 | rquickjs（异步支持好，生态成熟）vs quickjs（更底层）。建议：rquickjs | 待确认 |
| D3 | 布隆过滤器 crate | bloomfilter（纯 Rust，无外部依赖）vs 自研。建议：bloomfilter crate，可选 feature | 待确认 |
| D4 | P2 T 拆分粒度 | T1 forager → T2 布隆 → T3 WASM → T4 JS（串行，每 T 验证后再进下一个） | 待确认 |

### 1.6 验收标准

- T1：forager 信息增益评估测试通过（已知输入 → 预期 ForageDecision），中文 N-Gram 分词兼容
- T2：布隆过滤器可选 feature，forager 接收 seen_entropy_bloom 后重复 token 增益为 0
- T3：WASM 沙箱加载 .wasm 执行，epoch_deadline 超时强制终止，WASI 能力越权拒绝
- T4：JS 沙箱 worker 池固定大小，协变中断超时终止，ARM 端侧 10 并发不 OOM
- `cargo test --workspace` 全绿 + 0 warning

### 1.7 下一阶段预览：P3 — 完整工具集 + 插件热插拔 + 性能优化

- 内置工具集（targeted_scraper 等，对接 forager）
- 插件热插拔（文件系统监听，Manifest 即时注册）
- gRPC/MCP/STDIO 传输层
- 性能基准测试（ARM 端侧）

---

## 2. 阶段总览（地图，不展开）

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | Python 早期版本（哲学参考，main 分支保留） | ✅ 历史 |
| P1 | Rust 重构：核心骨架 + HTTP 传输层（T1-T4） | ✅ 2026-08-28（37 tests） |
| **P2** | **觅食 + 沙箱（修正2/3 落地，T1-T4）** | **🚧 进行中** |
| P3 | 完整工具集 + 插件热插拔 + 更多传输层 + 性能优化 | ⏳ 预览 |

---

## 3. 活跃决策与契约指针（不展开）

| 项 | 指针 |
|---|---|
| 四修正 | ADR-0001（修正1✅/4✅，修正2/3 待 P2） |
| 白皮书 | `docs/SPEC.md` + `docs/spec/*.md`（5 分卷） |
| 觅食设计 | 白皮书 §3.4（ForagingEvaluator 完整伪代码） |
| 沙箱设计 | 白皮书 §6.3（WASM epoch_deadline + JS 协变中断） |
| 生态对齐 | Anaphase-Helix gRPC 契约 + Helix-Mind 认知工艺 + Callosum 布隆导出 |
| Tuck 边界 | 明文凭证永不在 Tentacle 内存，Tuck 物理边缘注入 |

---

## 4. 文档生态 SOP（DNA v2.0）

PLAN 是导航牌不是历史档案；阶段收尾时（收尾 SLA：24h）完成记录追加 GROWTH.md 并从 PLAN 移除；GROWTH ≤3 条超则归档；PLAN ≤150 行超则触发历史迁移。提交信息必须包含 ADR 关联 `(ADR-NNNN §Tx)`。详见 `docs/DNA.md`「文档生态 SOP」和 `docs/RNA.md`「加载协议」。
