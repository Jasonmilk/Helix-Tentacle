# Helix-Tentacle — 通用战术武器系统（Rust 版）

> **rs 分支** ｜ **Apache 2.0** ｜ **DNA 自生长方法论 v2.0 治理**
> **性质**：独立的、自带说明书的、多传输层的通用工具执行引擎（无状态纯净版）

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
| **`rs`**（当前） | Rust 重构版，DNA 方法论治理 | 重构进行中 |
| **`main`** | 早期 Python 版 | **保留为哲学历史参考**（历史永不删除，按需加载） |

## 生态对齐链

```
Helix-Mind ← Anaphase-Helix ← Helix-Tentacle
（记忆/认知）  （编排/执行）    （工具/武器）
```

Tentacle 对齐 Anaphase（gRPC 契约 + 凭证标签流转 + 共识 Helix 模式），Anaphase 对齐 Mind（已完成）。对齐链是重构的硬约束。

## 四修正硬性验收（生态对齐）

1. **凭证标签流转**：对接 Tuck，内存 grep 不出明文 Cookie/Token
2. **已见熵布隆过滤器**：对接 Callosum，跨会话免重复采集
3. **异步协程沙箱**：ARM 端侧 worker 池，10 并发 JS 工具不 OOM/CPU 过载
4. **动态共识适配层**：Standalone/Helix/MCP 三模式，独立模式不返回 503

## 文档导航

| 文档 | 路径 |
|---|---|
| 愿景 | `docs/VISION.md` |
| 不可变原则 | `docs/DNA.md` |
| 生长记录 | `docs/GROWTH.md` |
| 当前阶段导航 | `docs/PLAN.md` |
| 决策记录 | `docs/decisions/` |
| 工程白皮书 v3.4 | `docs/vision/tentacle-whitepaper-v3.4.md` |

---

*Helix-Tentacle（rs）Rust 重构版。早期 Python 版保留于 main 分支作为哲学历史参考。Apache 2.0。*
