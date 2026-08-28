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
## 记录 2：P1 完成——Rust 核心骨架 + HTTP 传输层（2026-08-28）
**变异类型**：P1 阶段完成
**背景**：
- P1 目标：基于 DNA 方法论构建 Rust 版 Tentacle 核心骨架 + HTTP 传输层
- 按白皮书 v3.4 crates 结构搭建，四修正为硬性验收
- 方法论缺口：RNA.md/DEPRECATE.md 缺失，PLAN.md 未升级为导航牌格式
**关键决策**：
1. **T1 workspace 骨架**：9 crates（core/http/tools/transport-http/transport-mcp/transport-grpc/wasm/js/tentacle-bin），workspace Cargo.toml 共享配置
2. **T2 core 类型**：Tool trait/Manifest/ToolRegistry/Redactor/ConsensusHook，无网络依赖。`ExecutionRequest.identity_labels`（无明文 credentials，修正1），`ConsensusMode{Standalone,Helix,Mcp}`（修正4）
3. **T3 完整性校验**：SHA-256（sha2 crate）+ 插件目录扫描（walkdir）+ ScanReport 白盒可观测 + 渐进披露索引（ManifestIndex 只暴露 name/desc/version/security_level）
4. **T4 HTTP 传输层**：axum 0.7 四端点（manifest 索引/完整说明书/execute/execute_stream SSE）。Critical 级工具走 ConsensusHook 审批，输出自动脱敏。`IdentityHttpClient` 出网强制 `X-Identity-Label` 头（修正1落地）
5. **方法论补全**：RNA.md（三层加载 + PLAN 必读 + 凭证红线铁律）、DEPRECATE.md（DEP-001 Python 实现 + DEP-002 明文凭证）、PLAN.md 升级为导航牌格式（参照 Helix-Mind）
**四修正进度**：修正1✅（凭证标签流转）、修正4✅（动态共识适配）、修正2⏳（布隆过滤器待 P2）、修正3⏳（协程沙箱待 P2）
**验收**：`cargo test --workspace` → 37 passed, 0 failed, 0 warning
**状态**：✅ P1 完成，进入 P2（觅食 + 沙箱 + 更多传输层）
---
## 记录 3：预留
*（按 DNA v2.0 SOP，新记录追加至此，旧记录自动归档）*
