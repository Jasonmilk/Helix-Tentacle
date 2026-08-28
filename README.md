# Helix-Tentacle — 通用战术武器系统（Rust 版）

> **rs 分支** ｜ **Apache 2.0** ｜ **DNA 自生长方法论 v2.0 治理**
> **性质**：独立的、自带说明书的、多传输层的通用工具执行引擎（无状态纯净版）
> **当前阶段**：P4 ✅ 完成（生态对齐 + gRPC/MCP 传输层 + 插件热插拔），P5 🚧 预览中（性能优化 + 生产就绪）

## 定位

Tentacle 是一把"刀"——独立、可组合、自带说明书（Manifest）。它接收标准化的调用请求，在安全沙箱中执行工具，返回结构化结果，然后忘记一切。**Tentacle 不是 Helix 的双手，而是全 AI Agent 社区的公共武器库。**

- **通用**：不绑定任何 Agent 框架，通过 HTTP/MCP/gRPC/STDIO 平等服务所有调用者
- **极简**：启动 <2MB 内存，工具按需加载、用完即焚
- **自带说明书**：每个工具携带完整 Manifest（声明与执行体分离，SHA-256 密码学绑定）
- **身份流转**：凭证标签流转（`identity_label`），不存储任何明文凭证
- **渐进式觅食**：信息增益评估器 + 已见熵布隆过滤器，信息饱和即止
- **沙箱安全**：完整性校验/权限声明/运行时隔离/自动脱敏/孤儿沙箱防御
- **共识可选**：动态共识适配（Standalone 终端确认 / Helix CAP / MCP 回调）

## 分支说明

| 分支 | 内容 | 状态 |
|---|---|---|
| **`rs`**（当前） | Rust 重构版，DNA 方法论治理 | **P4 完成，P5 预览中**（76+ tests） |
| **`main`** | 早期 Python 版 | **保留为哲学历史参考**（历史永不删除，按需加载） |

## 阶段总览

| 阶段 | 内容 | 状态 | 测试 |
|---|---|---|---|
| P0 | Python 早期版本（哲学参考） | ✅ 历史 | - |
| P1 | Rust 重构：核心骨架 + HTTP 传输层 | ✅ 2026-08-28 | 37 tests |
| P2 | 觅食 + 沙箱（四修正全部兑现） | ✅ 2026-08-29 | 51 tests |
| P3 | 工具集成 + WASI 细化 + worker 池 + 首个内置工具 | ✅ 2026-08-29 | 72 tests |
| **P4** | **生态对齐 + gRPC/MCP 传输层 + 插件热插拔** | **✅ 2026-08-29** | **76+ tests** |
| P5 | 性能优化 + 生产就绪 | 🚧 预览中 | - |

## 传输层支持

| 传输层 | 状态 | 端点 | 适用场景 |
|---|---|---|---|
| **HTTP/REST + SSE** | ✅ 完成 | `GET /v1/manifest`、`GET /v1/tools/{name}/manifest`、`POST /v1/tools/{name}/execute`、`POST /v1/tools/{name}/execute_stream` | 任何 HTTP 客户端、远程部署、云服务 |
| **gRPC** | ✅ 完成 | `ListManifests`、`GetManifest`、`ExecuteTool`、`ExecuteStream` | Anaphase-Helix 内部生态、高性能二进制通信 |
| **MCP** | ✅ 完成 | `tools/list`、`tools/call`（JSON-RPC 2.0 over stdio/HTTP/SSE） | Claude Desktop、Codex 等 MCP 客户端 |
| **STDIO** | ⏳ P5 | 零配置本地使用 | 调试、脚本集成、极简部署 |

## 核心能力

| 能力 | 状态 | 说明 |
|---|---|---|
| 工具说明书（Manifest） | ✅ | 声明与执行体分离，SHA-256 完整性校验，渐进披露索引 |
| 凭证标签流转 | ✅ | `identity_labels` 三传输层统一，明文凭证永不在 Tentacle 内存（Tuck 物理边缘注入） |
| 渐进式觅食 | ✅ | `ForagingEvaluator` 纯统计信息增益评估（0 Token），多维停止条件，中文 2-Gram 分词 |
| 已见熵布隆过滤器 | ✅ | 可选 `bloom` feature，对接 Callosum，跨会话免重复采集 |
| WASM 沙箱 | ✅ | wasmtime v26 + epoch_deadline 超时 + WASI preview1（stdout 捕获 + 文件系统权限） |
| JS 协程沙箱 | ✅ | rquickjs + worker 池（与 CPU 核心数绑定）+ 协变中断 + 内存限制 |
| 插件热插拔 | ✅ | 可选 `hot-reload` feature（默认不启用），文件系统监听，索引实时更新 |
| 动态共识适配 | ✅ | Standalone 终端确认 / Helix CAP / MCP 回调三模式 |
| 全传输层集成测试 | ✅ | 11 个端到端测试，HTTP/gRPC/MCP 跨传输层一致性验证 |

## 生态对齐链

```
Helix-Mind ← Anaphase-Helix ← Helix-Tentacle
（记忆/认知）  （编排/执行）    （工具/武器）
```

Tentacle 对齐 Anaphase（gRPC 契约 + 凭证标签流转 + 共识 Helix 模式），Anaphase 对齐 Mind（已完成）。对齐链是重构的硬约束。

**CI-144 v2.0（PAL）**：审查通过、等待冻结，不阻塞当前阶段，冻结后自然接入生态手套路由。

## 四修正硬性验收（生态对齐）

1. **凭证标签流转**：对接 Tuck，内存 grep 不出明文 Cookie/Token ✅
2. **已见熵布隆过滤器**：对接 Callosum，跨会话免重复采集 ✅
3. **异步协程沙箱**：ARM 端侧 worker 池，10 并发 JS 工具不 OOM/CPU 过载 ✅
4. **动态共识适配层**：Standalone/Helix/MCP 三模式，独立模式不返回 503 ✅

## 快速开始

```bash
# 克隆仓库
git clone https://github.com/Jasonmilk/Helix-Tentacle.git
cd Helix-Tentacle
git checkout rs

# 运行测试
cargo test --workspace

# 运行全传输层集成测试
cargo test -p tentacle-integration-tests

# 构建（按需启用 feature）
cargo build --release --features runtime,scraper,bloom
```

## 文档导航

| 文档 | 路径 | 说明 |
|---|---|---|
| 愿景索引 | `docs/VISION.md` | 根索引（原子原则 + 导航 + 生态位置） |
| 知识本体（完整叙事） | `docs/SPEC.md` | "一粒种子的自白"，项目存在的理由 |
| 哲学 / 架构 / 契约 / 安全分卷 | `docs/spec/` | 5 个分卷详细规格 |
| 不可变原则 | `docs/DNA.md` | 8 条公理 + 自生长流程 |
| AI 协作铁律 + 三层加载协议 | `docs/RNA.md` | PLAN.md 必读声明 + 决策拦截铁律 |
| 已退役实现 | `docs/DEPRECATE.md` | Python 早期实现等退役记录 |
| 生长记录 | `docs/GROWTH.md` | 最近 3 条，超则归档 |
| 当前阶段导航 | `docs/PLAN.md` | 只含当前阶段 + 下一阶段预览 |
| 决策记录 | `docs/decisions/` | ADR-0001 起，Active 后不可覆写 |
| 工程白皮书 v3.4 | `docs/vision/tentacle-whitepaper-v3.4.md` | 完整架构设计 |

## 方法论治理

本项目遵循 **DNA 自生长方法论 v2.0**：

- **N 层（叙事）**：VISION.md + SPEC.md + spec/ 分卷
- **D 层（决策）**：ADR 系列（Draft/Active 两态，Active 后不可覆写）
- **A 层（架构）**：代码 + 契约（crates/ + proto/）
- **文档生命周期**：PLAN ≤150 行、GROWTH ≤3 条、归档永不删除
- **提交规范**：提交信息必须包含 ADR 关联 `(ADR-NNNN §Tx)`

---

*Helix-Tentacle（rs）Rust 重构版。早期 Python 版保留于 main 分支作为哲学历史参考。Apache 2.0。*
