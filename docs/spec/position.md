# 生态定位卷（spec/position.md）
> **来源**：Tentacle 重构初心与共识 v1.0 + 工程白皮书 v3.4 第四部分
> **性质**：Tentacle 在 Helix 生态中的角色、边界与使用模式

## 一、组件边界

| 组件 | 角色 | 职责 | Tentacle 的关系 |
|---|---|---|---|
| **Helix-Mind** | 灵魂 / 海马体 | 思考、认知工艺、记忆、代谢 | Tentacle 不被调用时不参与认知工艺；不直接调用 Mind |
| **Anaphase-Helix** | 身体 / 编排中枢 | 编排、调度、感知、生命周期 | Tentacle 被 Anaphase 调用，执行后返回结果 |
| **Helix-Tentacle** | 手脚 / 战术武器库 | 工具执行、信息采集、插件沙箱 | **本组件**——只执行，不思考不编排 |
| **Tuck** | 免疫系统 / 安全闸门 | 凭证物理注入、安全审计、硬拦截 | Tentacle 只传递 `identity_label`，明文凭证由 Tuck 边缘注入 |
| **Cellrix** | 眼睛 / 皮肤 | 观测、渲染、可视化 | Tentacle 执行状态可在 Cellrix 中展示 |
| **Callosum** | 胼胝体 / 神经桥接 | 上下文压缩、KV 缓存复用 | Tentacle 接收 Callosum 导出的已见熵布隆过滤器 |

**核心铁律**：
- Tentacle 不持有长期记忆（归 Mind）
- Tentacle 不做编排决策（归 Anaphase）
- Tentacle 不存储明文凭证（归 Tuck）
- Tentacle 只做一件事：**高效、安全地执行已分配的工具任务**

## 二、与认知工艺的关系

**Tentacle 与认知工艺无直接关系。**

认知工艺是 Helix-Mind 的能力（工序编排、独立会话隔离、黑格尔辩证收敛、System 0 门控、预算路由）。Tentacle 是工具执行引擎，它不参与"思考"，只负责"执行"。

**调用链路**：认知工艺（Mind）→ 编排（Anaphase）→ 工具执行（Tentacle）。Tentacle 通过 Anaphase 编排被间接调用，不直接对接认知工艺。

**边界意义**：Tentacle 的觅食评估用统计方法（0 Token），不调用 LLM——这与认知工艺的 LLM 驱动完全分离。Tentacle 是"手"，手不需要思考。

## 三、两种使用模式

| 模式 | 说明 | 场景 |
|---|---|---|
| **Anaphase 编排调用** | Anaphase 根据 Mind 认知输出，决定是否调用 Tentacle | Helix 生态内，全自动编排 |
| **人类直接调用** | 通过 HTTP/MCP/STDIO 直接调用 Tentacle | 类似 Harness，手动使用；独立开发者 / Claude Desktop |

**两种模式共享同一套 ToolRegistry 和安全层**，无状态冲突。Tentacle 的通用性保证了它既是 Helix 的手脚，也是全 AI Agent 社区的公共武器库。

## 四、哲学检查清单（8 条）

| 原则 | Tentacle 实现 |
|---|---|
| **极致解耦** | 与 Mind/Anaphase 职责分离，无状态，多传输层 |
| **极致复用** | 插件化，Manifest 驱动，通用工具服务器 |
| **极致节能** | 信息增益评估（统计方法 0 Token），提前终止，避免无效爬取 |
| **白盒可观测** | Trace ID 穿透，审计日志脱敏，状态机驱动觅食（广扫→聚焦→深挖） |
| **零信任** | 凭证标签流转，Tuck 物理注入，沙箱权限声明 |
| **如无必要勿增实体** | 工具按需加载，执行即焚，统计优先 LLM 兜底 |
| **脑手分离** | Tentacle 是"手"（执行），不参与"脑"（思考/编排） |
| **意志优先** | Tentacle 不决策，只执行 Anaphase 的明确指令 |

## 五、前沿研究与工业验证（晶体/胶体支撑）

### 5.1 L1 信息触手（渐进式觅食）

| 来源 | 对应机制 | 验证类型 |
|---|---|---|
| InForage（NeurIPS 2025） | 动态信息寻求过程 | 学术 |
| CaRT（2025） | 何时停止（多维停止条件） | 学术 |
| ProMSA（ECCV 2026） | 渐进式多模态搜索 | 学术 |
| Crawl4AI AdaptiveCrawler | 闭环信息觅食系统 | 工业 |
| Claude 渐进搜索 | 智能研究过程 | 工业 |

### 5.2 L2 武器工坊（插件化）

| 项目 | 核心特性 | 与 Tentacle 对齐 |
|---|---|---|
| DeepSeek Harness | "万物皆插件" | 插件化范围一致 |
| MCP 工具生态 | 工具说明书驱动 | Manifest 驱动一致 |
| AnyTool | 通用工具层 + 质量排序 | 通用工具层一致 |

### 5.3 L3 安全沙箱

| 项目 | 核心特性 | 与 Tentacle 对齐 |
|---|---|---|
| Wassette（微软） | WASM 沙箱 + 最小权限原则 | 沙箱隔离一致 |
| LangChain Sandbox | WASM Python 执行 | 沙箱隔离一致 |
| Claude Managed Agents | 沙箱环境 + 自托管沙箱 | 沙箱隔离一致 |
| E2B 接口标准 | 行业沙箱接口标准 | 接口设计参考 |

**行业背景**：MCP 生态在 2025-2026 年暴露多个严重安全漏洞（如 mcp.run 平台曾被发现允许恶意工具执行系统命令）。业界已形成共识：工具执行层必须有独立沙箱隔离，不能依赖"协议安全"保障"执行安全"。

---

*《生态定位卷》完。*
