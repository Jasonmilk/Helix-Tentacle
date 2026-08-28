# ADR-0001：Rust 重构 + 生态对齐（凭证标签流转等四修正）

## 状态

**Active**（2026-08-28，已审查通过）

## 问题

1. Tentacle `rs` 分支为早期 Python 版（明文 cookie 加载 + 本地觅食），与生态对齐方向相反
2. 身份明文传递与 Tuck 零信任冲突（安全红线）
3. 渐进式觅食无全局记忆感知（能耗浪费）
4. JS 沙箱 OS 线程开销在 ARM 端侧不可行（性能）
5. 共识钩子独立运行降级 503（独立体验）

## 决策

1. **清理 Python 残留**：物理移除 `cli/`/`cookies/`/`domains/`/`tentacle/`/`tests/`/`pyproject.toml`，main 分支保留 Python 版为哲学历史
2. **Apache 2.0**：协议从 MIT 改为 Apache 2.0
3. **DNA 方法论先行**：五件套（VISION/DNA/GROWTH/PLAN/decisions）建立后再编码
4. **四修正（硬性验收）**：
   - **修正 1 凭证标签流转**（🔴 安全红线）：`ExecutionRequest.credentials` → `identity_labels`（标签如 `weibo_session_1`）；Tentacle 出网强制 `X-Identity-Label` 头，Tuck（127.0.0.1:9000）边缘注入真实凭证。独立模式（无 Tuck）下凭证由调用者本地提供（用户自担），Helix 生态模式用标签流转
   - **修正 2 已见熵布隆过滤器**（🟡 节能）：`forager.rs` 接收 `seen_entropy_bloom: Option<BloomFilter>`（Callosum 导出）；布隆假阳性可配置（如 1%）；依赖 Callosum 渐进（先本地 seen_entropy，布隆可选 feature）
   - **修正 3 异步协程沙箱**（🟢 性能）：JS 沙箱固定 worker 池（绑定物理核）+ QuickJS 多 context 协程调度，协变中断降级为协程级协作终止
   - **修正 4 动态共识适配**（🟡 体验）：`ConsensusMode{Standalone/Helix/MCP}`——Standalone 终端确认（`eprintln!` + stdin `yes`）；Helix gRPC 桥接 CAP；MCP 回调。独立模式不返回 503。与 Anaphase HITL（执行闸）职责不重叠：HITL 管编排层，共识钩子管工具层 Critical
5. **对齐链**：Tentacle 对齐 Anaphase（gRPC + 凭证标签 + 共识 Helix 模式）→ Anaphase 对齐 Mind

## 白皮书升级

- `docs/vision/tentacle-whitepaper-v3.4.md`：在 v3.3.2 基础上纳入四修正（T05/3.3 凭证标签、T07/3.4 布隆、T06/6.3 协程、6.7 共识适配）+ 独立/Helix 双轨语义

## 回滚阈值

若 Rust 重构导致与 Anaphase gRPC 契约不兼容，或四修正验收失败产生安全回归，可回滚至清理前状态重新评估。

## 关联

- Anaphase-Helix ADR-0001（契约对齐，P10a-P11b）
- Helix-Mind ADR-0010/0020/0021
- Tentacle 工程白皮书 v3.4
