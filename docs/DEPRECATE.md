# 凋亡清单（Helix-Tentacle）

> **所属方法论**：DNA 自生长方法论 v2.0
> **规则**：每条必须有明确死期。已安葬项移入 `docs/archive/deprecated/`。退役期间标记为 `deprecated`，新代码不得调用。

---

## DEP-001: 早期 Python 实现（main 分支）

- **原因**：Tentacle 早期以 Python 原型验证设计哲学（渐进式觅食、插件化、身份流转）。方法论迁移至 Rust（rs 分支）后，Python 原型仅作为哲学历史参考保留在 main 分支，不再维护
- **替代**：rs 分支 Rust 重构（`crates/tentacle-core/`、`crates/tentacle-transport-http/`、`crates/tentacle-tools/` 等）。Python 版的设计哲学已吸收到白皮书 v3.4 和 DNA 原则中
- **截止**：2026-12-31（main 分支保留为只读历史参考，不接受新功能 PR）
- **状态**：⏳ 已退役（main 分支只读），Rust 版（rs 分支）为活跃开发线

## DEP-002: Python 版明文凭证传递模式

- **原因**：早期 Python 版在请求体中传递明文 Cookie/Token。四修正审查确认此模式存在安全漏洞（凭证在到达 Tuck 前已在内存中暴露），与 Helix 生态零信任哲学冲突
- **替代**：Rust 版采用凭证标签流转（`identity_label`）——Tentacle 只传递无意义标签，明文凭证由 Tuck 在物理边缘（127.0.0.1:9000）注入。`ExecutionRequest.identity_labels` 字段 + `IdentityHttpClient` 出网 `X-Identity-Label` 头
- **截止**：立即（Rust 版从 T1 起即遵循，无迁移期）
- **状态**：✅ 已安葬（Rust 版从未实现明文凭证模式）

---

## 退役流程

1. **公告**：功能确定退役后，在此文件添加条目（名称、原因、替代方案、退役版本、截止日期）
2. **标记**：代码中添加 `#[deprecated]` 或文档注释，新代码不得调用
3. **保留**：保留至少一个版本周期，给调用方迁移时间
4. **安葬**：迁移期结束后，删除代码，将此条目移入 `docs/archive/deprecated/YYYY-MM-DD-<名称>.md`

## 历史退役

| 日期 | 功能 | 原因 | 替代方案 | 归档 |
|---|---|---|---|---|
| — | — | — | — | — |
