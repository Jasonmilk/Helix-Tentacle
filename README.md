# Helix-Tentacle · 硅基生命体的感官触手

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)

Helix-Tentacle 是专为 Anaphase-Helix 自主进化智能体系统设计的**文档探查与脱水器官**。它像一条敏锐的触手，先轻轻触碰长文本的轮廓，再根据你的目的进行定向压缩，将海量信息提炼为高密度的核心内容。

- **渐进式探查**：两次探查机制，先取头尾，看不清再深入中间，低成本获取完整大纲。
- **定向脱水**：基于探查得到的轮廓，根据指定目的（如“提取错误原因”、“生成周报摘要”）精准降维，保留关键信息。
- **模型无关**：所有模型调用均通过 [Tuck 网关](https://github.com/Jasonmilk/Tuck) 完成，无缝对接任何 OpenAI 兼容后端（vLLM、llama.cpp、One-API 等）。
- **微服务化**：独立部署，提供 REST API，可被 Anaphase 或其他组件远程调用。

---

## 目录结构

```text
Helix-Tentacle/
├── README.md
├── requirements.txt         # 依赖列表
├── config.py                # 配置加载（基于 pydantic-settings）
├── tentacle.py              # FastAPI 服务入口
├── core/
│   ├── __init__.py
│   ├── prober.py            # 渐进式轮廓探查器
│   └── dehydrator.py        # 定向脱水器
├── .env.example             # 环境变量示例
└── tests/                   # 单元测试（可选）
```

---

## 快速开始

### 环境要求
- Python 3.10+
- 一个可用的 Tuck 网关（或任何 OpenAI 兼容的模型服务端点）

### 安装

```bash
# 克隆仓库
git clone https://github.com/Jasonmilk/Helix-Tentacle.git
cd Helix-Tentacle

# 创建虚拟环境（推荐）
python3 -m venv venv
source venv/bin/activate   # Linux/Mac
# 或 venv\Scripts\activate   # Windows

# 安装依赖
pip install -r requirements.txt
```

### 配置

复制环境变量示例文件并根据实际情况修改：

```bash
cp .env.example .env
```

`.env` 中的关键配置项：

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `TUCK_URL` | Tuck 网关的 `/v1/chat/completions` 地址 | `http://127.0.0.1:8000/v1/chat/completions` |
| `TUCK_API_KEY` | 如果 Tuck 开启了鉴权，在此填写 Bearer Token | 空 |
| `DEFAULT_PROBE_MODEL` | 探查和脱水默认使用的模型名（需在 Tuck 中可路由） | `qwen2.5-2b` |
| `SAMPLE_SIZE` | 每次探查截取的最大字符数 | `4000` |

### 启动服务

```bash
python tentacle.py
```

默认监听 `0.0.0.0:8010`。可通过修改 `tentacle.py` 中的 `uvicorn.run` 参数调整。

---

## API 文档

### POST `/v1/tentacle/process`

对长文本进行探查和脱水。

**请求体**（JSON）：

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `text` | string | ✅ | 待处理的原始文本（支持超长文本） |
| `purpose` | string | ❌ | 脱水目的，如“提取核心错误原因”“生成技术周报摘要”等，默认为“提取核心摘要” |
| `model` | string | ❌ | 本次处理使用的模型名，若不指定则使用 `DEFAULT_PROBE_MODEL` |

**响应**（JSON）：

| 字段 | 类型 | 描述 |
|------|------|------|
| `outline` | string | 探查到的文档大纲 |
| `dehydrated_content` | string | 定向脱水后的精简内容 |

**示例请求**：

```bash
curl -X POST http://127.0.0.1:8010/v1/tentacle/process \
  -H "Content-Type: application/json" \
  -d '{
    "text": "（这里放几万字的日志或文章）",
    "purpose": "找出导致系统崩溃的根本原因和报错行号",
    "model": "qwen2.5-7b"
  }'
```

**示例响应**：

```json
{
  "outline": "文档主要分为三部分：1. 环境配置；2. 错误日志堆栈；3. 可能原因分析。",
  "dehydrated_content": "根本原因：内存分配失败，发生在行 123。建议增加 swap 或降低并发。"
}
```

### GET `/health`

健康检查，返回服务状态。

---

## 部署建议

### 使用 systemd 作为后台服务（Linux）

创建 `/etc/systemd/system/helix-tentacle.service`：

```ini
[Unit]
Description=Helix Tentacle Service
After=network.target

[Service]
User=youruser
WorkingDirectory=/opt/Helix-Tentacle
Environment="PATH=/opt/Helix-Tentacle/venv/bin"
ExecStart=/opt/Helix-Tentacle/venv/bin/python /opt/Helix-Tentacle/tentacle.py
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

启动并启用：

```bash
systemctl daemon-reload
systemctl start helix-tentacle
systemctl enable helix-tentacle
```

### 使用 Docker（可选）

待补充 Dockerfile。

---

## 开发与扩展

### 添加新的探查策略

继承或修改 `core/prober.py` 中的 `TentacleProber`，重写 `_first_probe` 或 `_second_probe` 方法。

### 优化脱水效果

在 `core/dehydrator.py` 中，可以根据 `purpose` 使用更精细的提示词模板，甚至引入 Map-Reduce 分块处理超长文档。

### 增加缓存

为了避免重复处理相同文本，可以在服务层增加基于文本哈希的 LRU 缓存（使用 `functools.lru_cache` 或 Redis）。

---

## 常见问题

**Q: 触手调用 Tuck 时返回 404？**  
A: 请确认 `TUCK_URL` 配置正确，并且 Tuck 服务已启动，且 `/v1/chat/completions` 路径存在。

**Q: 模型调用超时或返回空？**  
A: 检查 `DEFAULT_PROBE_MODEL` 是否在 Tuck 中可用（通过 `curl http://tuck:8000/v1/models` 查看模型列表）。另外可增大 `timeout` 参数（在 `prober.py` 和 `dehydrator.py` 中调整 `httpx.AsyncClient` 的 `timeout`）。

**Q: 内存占用过高？**  
A: 目前对大文档会进行截断处理，若仍需处理完整文档，建议在脱水器实现 Map-Reduce 分块处理。

---

## 许可证

MIT © Jasonmilk

---

## 相关项目

- [Tuck](https://github.com/Jasonmilk/Tuck) — 智能体安全网关与模型路由引擎
- [Anaphase-Helix](https://github.com/Jasonmilk/Anaphase-Helix) — 自主进化型硅基智能体系统
- [Helix-Mind](https://github.com/Jasonmilk/Helix-Mind) — 认知中枢（记忆与技能管理）

---

欢迎提交 Issue 和 Pull Request，共同完善 Helix 生态！
