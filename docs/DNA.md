# Helix-Tentacle DNA — 不可变原则与自生长流程
> **版本**：v1.0
> **日期**：2026-08-28
> **继承自**：DNA 自生长方法论 v2.0（机制复用）、Helix-Tentacle VISION.md v1.0（哲学内容源）、工程白皮书 v3.4
> **性质**：Tentacle 的不可变原则与如何生长。修改 DNA 等于修改身份，旧身份的信用不会转移。

## 一、不可变原则（10 条公理）

见 `docs/VISION.md` 原子原则表（10 条）。本文件不再重复原则内容，而是定义**工程映射**与**防腐化铁律**。

**工程映射（关键 4 条，对应四修正硬性验收）**：

| 原则 | 工程映射 | 验收标准 |
|---|---|---|
| 4 零信任凭证：标签流转 | `tentacle-core` 用 `identity_labels: HashMap<String,String>` 替代明文 credentials；`tentacle-http` 出网强制加 `X-Identity-Label` 头 | **内存 grep 不出明文 Cookie/Token**；Tuck 按标签替换 |
| 6 已见熵布隆过滤器 | `tentacle-tools/forager.rs` 接收 `seen_entropy_bloom: Option<BloomFilter>`（Callosum 导出） | 两次会话爬同一关键词，第二次不重复采集已覆盖信息 |
| 7 异步协程沙箱 | JS 沙箱用固定 worker 池 + 协程协作终止，不 `std::thread::spawn` | ARM 端侧 10 并发 JS 调用不 OOM/CPU 过载 |
| 8 动态共识适配 | `ConsensusMode { Standalone, Helix, MCP }`；Standalone 终端确认，Helix gRPC 桥接 CAP，MCP 回调 | Claude Desktop 调 Critical 工具弹确认，不返回 503 |

## 二、分层自纠偏系统（N/D/A）

| 层级 | 名称 | 形式 | 作用 |
|---|---|---|---|
| **N 层** | 叙事层（愿景） | `VISION.md` + `docs/vision/tentacle-whitepaper-v3.4.md` | 顶层叙事，所有决策最终裁判 |
| **D 层** | 决策层（ADR） | `docs/decisions/XXXX-*.md` | 记录架构决策的"为什么"与"放弃了什么" |
| **A 层** | 架构层（代码） | `crates/*/src/` | 物理实现最终形态 |

## 三、文档生态 SOP（DNA v2.0）

| 文档 | 职责 | 规则 |
|---|---|---|
| **PLAN.md** | 当前阶段导航 + 下一阶段预览 | ≤150 行，超出触发历史迁移 |
| **GROWTH.md** | 已完成阶段生长记录 | ≤3 条，超则归档至 `docs/archive/growth/` |
| **ADR** | 决策记录 | 两态（Draft/Active），Active 后不可覆写，仅可 Superseded |
| **归档** | 历史记录 | 随仓库版本化，永不删除 |

## 四、防腐化铁律（5 条）

| # | 铁律 | 说明 |
|---|---|---|
| 1 | **版本以 spec/代码为源真相** | README/门面标注必须对齐，防版本漂移 |
| 2 | **契约冻结不可静默修改** | 扩展走 Append-Only / reserved 预留 |
| 3 | **变更先 ADR（D 层冻结）→ 改代码 → 同步门面** | 决策先于代码 |
| 4 | **生长记录保留近 3 条，超则归档** | 历史永不删除，按需加载 |
| 5 | **提交前必须人工确认** | 无自动提交 |

## 五、与生态的关系

- **对齐链**：Tentacle 对齐 Anaphase → Anaphase 对齐 Mind（Helix-Mind 为记忆/认知真相源）
- **四修正**是 Helix 生态接入的硬性验收（凭证标签/布隆/协程/共识），不改变 Tentacle 通用核心
- **main 分支**保留早期 Python 版为哲学历史参考，rs 分支为当前重构

## 六、一句话总结

> **Tentacle DNA 是武器的基因锁：通用核心（多传输层/自带说明书）+ 零信任凭证标签流转 + 沙箱即契约。四修正（凭证标签/已见熵/协程沙箱/动态共识）是生态对齐的硬性验收，不改变其独立于全 AI Agent 社区的定位。**

---

*《Helix-Tentacle DNA.md》v1.0 完。*
