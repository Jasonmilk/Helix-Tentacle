# Helix-Tentacle v2.3

[![EN](https://img.shields.io/badge/English-README-blue)](./README.md)

Helix 生态系统的外部感知与渐进式信息嗅探微服务。

Tentacle 是 Helix 数字生命体的“世界感知器官”。它基于**信息觅食理论**实现了**渐进式信息觅食**，支持两阶段的网页内容提取：
1. **低分辨率扫描**：生成包含关键词命中密度的文档地形图。
2. **高分辨率提取**：仅从高价值区块中拉取原始文本。

## v2.3 新特性

- **面向 Agent 的可编程感知**：完整的 `KeywordFilter` 支持（`include`、`exclude`、`boost`），贯穿搜索、扫描与提取全流程。
- **领域与站点配置**：通过 `--domain` 和 `--site` 加载领域特定默认值并限定搜索站点。
- **Cookie / 会话支持**：使用 Netscape 格式的 Cookie 文件进行认证抓取。
- **内容质量评分**：对每个 DOM 区块自动评估内容质量（纯度、结构位置、标签多样性）。
- **多级过滤**：通过 `--filter-level`（`none`、`standard`、`strict`）控制内容过滤的激进程度。

## 双态架构

Tentacle 支持两种运行模式，共享同一套核心引擎：

### 🔌 内嵌模式（Embedded，默认）
作为 Helix 生态系统内的 REST API 微服务运行。
- 供 Anaphase（Helix 的前额叶）调用
- 完整的 HXR 审计日志、Trace ID 透传
- 与 Tuck 网关集成以保障安全
- 禁用 CLI 接口

### 🖥️ 独立模式（Standalone）
作为独立的 CLI 工具在本地开发和使用。
- 人性化的命令行界面
- 本地日志、基础 SSRF 防护
- 独立于 Helix 部署环境

## 安装

```bash
# 从源码安装（可编辑模式）
pip install -e .

# 或安装时包含开发依赖
pip install -e ".[dev]"
```

## 快速开始

### 独立 CLI 模式

首先启用独立模式：
```bash
export TENTACLE_MODE=standalone
```

#### 扫描 URL（支持过滤）
```bash
# 基础扫描
tentacle scan https://example.com --keywords "AI,Agents" --format rich

# 高级过滤扫描
tentacle scan https://example.com \
  --keywords "AI" \
  --require "报告" \
  --exclude "广告" \
  --boost "2025:2.0" \
  --filter-level strict \
  --format table
```

#### 提取指定区块
```bash
# 从特定区块提取原始文本
tentacle extract https://example.com --sections sec_001,sec_003
```

#### 带领域和站点限制的搜索
```bash
# 搜索网页
tentacle search "LLM reasoning" --limit 3

# 仅在特定站点内搜索
tentacle search "AI 焦虑" --site "zhihu.com,bbc.com" --limit 2

# 使用领域配置进行搜索（自动加载默认关键词/站点）
tentacle search "电子产品" --domain trade --limit 5
```

#### 认证抓取（Cookie 支持）
```bash
# 将 Cookie 文件放置在 ./cookies/zhihu.txt
tentacle scan https://www.zhihu.com/people/me --cookie zhihu.txt --format table
```

#### 交互式探索
```bash
# 交互式探索文档结构
tentacle explore https://example.com
```

### 内嵌服务器模式

启动 API 服务器：
```bash
# 默认模式为 embedded
uvicorn tentacle.api.main:app --host 0.0.0.0 --port 8021
```

服务器将在 `http://localhost:8021` 上可用。

#### API 端点

| 方法 | 路径 | 描述 |
|--------|------|-------------|
| POST | `/v1/tentacle/scan` | 阶段一：扫描 URL 以获取地形图 |
| POST | `/v1/tentacle/extract` | 阶段二：从指定区块提取原始文本 |
| POST | `/v1/tentacle/search` | 网页搜索代理 |
| POST | `/v1/tentacle/feedback` | 提交反馈以支持模型演进 |
| GET | `/health` | 健康检查 |
| GET | `/metrics` | Prometheus 指标（可选） |

交互式 API 文档可通过 `http://localhost:8021/docs` 访问。

## 配置

所有配置均通过环境变量或 `.env` 文件完成。所有可用选项请参考 `.env.example`。

关键配置项：
- `TENTACLE_MODE`：运行模式（`embedded`/`standalone`）
- `TENTACLE_PORT`：API 服务器端口（默认：8021）
- `TENTACLE_SEARCH_PROVIDER`：搜索引擎提供商（`duckduckgo`/`serpapi`）
- `TENTACLE_DOMAINS_DIR`：领域配置 YAML 文件的路径
- `TENTACLE_MAX_SNIPPET_SIZE`：每个区块的最大片段大小
- `TENTACLE_FILTER_LEVEL`：全局默认过滤级别（`none`/`standard`/`strict`）

## 安全性

- **SSRF 防护**：默认禁止访问私有 IP 地址。
- **输入校验**：对所有输入进行严格的模式校验。
- **速率限制**：在内嵌模式下由 Tuck 网关处理。
- **审计日志**：包含 Trace ID 的完整请求/响应日志（兼容 HXR）。

## 开发

```bash
# 运行测试
pytest

# 代码检查
ruff check .

# 代码格式化
ruff format .
```

## 许可证

MIT