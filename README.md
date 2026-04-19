# Helix-Tentacle v2.3

  [![中文](https://img.shields.io/badge/简体中文-README-red)](./README.zh-CN.md)

External perception and progressive information sniffing microservice for the Helix ecosystem.

Tentacle is the "world perception organ" of the Helix digital lifeform. It implements **progressive information foraging** based on information foraging theory, enabling two-phase web content extraction:
1. **Low-resolution scan**: Generate document topography with keyword hit density
2. **High-resolution extract**: Pull raw text only from high-value sections

## What's New in v2.3

- **Agent-Programmable Perception**: Full `KeywordFilter` support (`include`, `exclude`, `boost`) applied consistently across search, scan, and extract.
- **Domain & Site Configuration**: Load domain-specific defaults and restrict searches to specific sites via `--domain` and `--site`.
- **Cookie/Session Support**: Authenticated scraping using Netscape-format cookie files.
- **Content Quality Scoring**: Automatic quality estimation for each DOM section (purity, position, tag diversity).
- **Multi-level Filtering**: `--filter-level` (`none`, `standard`, `strict`) to control aggressiveness of content filtering.

## Dual-mode Architecture

Tentacle supports two running modes sharing the same core engine:

### 🔌 Embedded Mode (Default)
Run as a REST API microservice within the Helix ecosystem.
- Used by Anaphase (Helix's prefrontal cortex)
- Full HXR audit logging, Trace ID propagation
- Integrated with Tuck gateway for security
- Disables CLI interface

### 🖥️ Standalone Mode
Run as a standalone CLI tool for local development and usage.
- Human-friendly CLI interface
- Local logging, basic SSRF protection
- Independent of Helix deployment

## Installation

```bash
# Install from source (editable)
pip install -e .

# Or install with dev dependencies
pip install -e ".[dev]"
```

## Quick Start

### Standalone CLI Mode

First enable standalone mode:
```bash
export TENTACLE_MODE=standalone
```

#### Scan a URL with filtering
```bash
# Basic scan
tentacle scan https://example.com --keywords "AI,Agents" --format rich

# Scan with advanced filtering
tentacle scan https://example.com \
  --keywords "AI" \
  --require "report" \
  --exclude "advertisement" \
  --boost "2025:2.0" \
  --filter-level strict \
  --format table
```

#### Extract sections
```bash
# Extract raw text from specific sections
tentacle extract https://example.com --sections sec_001,sec_003
```

#### Search with domain and site restrictions
```bash
# Search the web
tentacle search "LLM reasoning" --limit 3

# Search only within specific sites
tentacle search "AI anxiety" --site "zhihu.com,bbc.com" --limit 2

# Search using a domain configuration (loads default keywords/sites)
tentacle search "electronics" --domain trade --limit 5
```

#### Authenticated scraping (cookie support)
```bash
# Place cookie file in ./cookies/zhihu.txt
tentacle scan https://www.zhihu.com/people/me --cookie zhihu.txt --format table
```

#### Interactive exploration
```bash
# Explore document interactively
tentacle explore https://example.com
```

### Embedded Server Mode

Start the API server:
```bash
# Default mode is embedded
uvicorn tentacle.api.main:app --host 0.0.0.0 --port 8021
```

The server will be available at `http://localhost:8021`

#### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/tentacle/scan` | Phase 1: Scan URL to get topography |
| POST | `/v1/tentacle/extract` | Phase 2: Extract raw text from sections |
| POST | `/v1/tentacle/search` | Web search proxy |
| POST | `/v1/tentacle/feedback` | Submit feedback for model evolution |
| GET | `/health` | Health check |
| GET | `/metrics` | Prometheus metrics (optional) |

Interactive API docs are available at `http://localhost:8021/docs`

## Configuration

All configuration is done via environment variables or `.env` file. See `.env.example` for all available options.

Key configurations:
- `TENTACLE_MODE`: Running mode (`embedded`/`standalone`)
- `TENTACLE_PORT`: API server port (default: 8021)
- `TENTACLE_SEARCH_PROVIDER`: Search engine provider (`duckduckgo`/`serpapi`)
- `TENTACLE_DOMAINS_DIR`: Path to domain configuration YAML files
- `TENTACLE_MAX_SNIPPET_SIZE`: Maximum snippet size per section
- `TENTACLE_FILTER_LEVEL`: Global default filter level (`none`/`standard`/`strict`)

## Security

- **SSRF Protection**: Blocks access to private IP addresses by default
- **Input Validation**: Strict schema validation for all inputs
- **Rate Limiting**: Handled by Tuck gateway in embedded mode
- **Audit Logging**: Full request/response logging with trace IDs (HXR compatible)

## Development

```bash
# Run tests
pytest

# Lint code
ruff check .

# Format code
ruff format .
```

## License

MIT