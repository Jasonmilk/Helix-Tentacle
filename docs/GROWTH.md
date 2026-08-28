# Helix-Tentacle 生长记录
> **版本**：v1.0
> **日期**：2026-08-28
> **规则**：仅保留最近 3 条记录，超则归档至 `docs/archive/growth/`
> **归档策略**：历史随仓库版本化，永不删除

## 记录 1：方法论迁移 + Rust 重构启动（2026-08-28）
**变异类型**：方法论迁移 + 重构启动
**背景**：
- Tentacle `rs` 分支从 `main`（早期 Python 版）创建，需清理重构
- 用户指令：先完成 DNA 自生长方法论，再基于方法论构建 Rust 版
- 对齐链：Helix-Tentacle 对齐 Anaphase-Helix，Anaphase-Helix 对齐 Helix-Mind
**关键决策**：
1. **清理**：物理移除 `cli/`、`cookies/`（含 `google.txt` 凭证样例）、`domains/`、`tentacle/`、`tests/`、`pyproject.toml`、`README.zh-CN.md`——早期 Python 残留与生态对齐方向相反（明文 cookie 加载 + 本地觅食）
2. **协议**：MIT → **Apache 2.0**
3. **main 保留**：main 分支保留 Python 版作为哲学历史参考（历史永不删除，按需加载）
4. **DNA 五件套**：VISION/DNA/GROWTH/PLAN/decisions/0001 建立（机制复用 DNA v2.0，内容按 Tentacle 哲学独立编写）
5. **四修正为硬性验收**：凭证标签流转（Tuck）/ 已见熵布隆过滤器（Callosum）/ 异步协程沙箱（ARM）/ 动态共识适配（Standalone/Helix/MCP）
**状态**：✅ 方法论已建立，Rust 重构待启动
---
## 记录 2：预留
*（按 DNA v2.0 SOP，新记录追加至此，旧记录自动归档）*
