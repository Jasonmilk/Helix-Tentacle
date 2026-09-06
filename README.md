# Helix-Tentacle — Universal Tactical Weapon System (Rust)

> **rs branch** ｜ **Apache 2.0** ｜ **governed by phyt-DNA methodology v1.0**
> **Nature**: independent, self-describing (Manifest), multi-transport general-purpose tool execution engine (stateless pure version)
> **Current stage**: P5 ✅ complete (performance optimization + resource limits + observability + STDIO transport), P6 ⏳ in preview (full ecosystem component integration + CI-144 v2.0 wiring + production deployment)

> **中文版 (Chinese Version)**: [README.zh-CN.md](./README.zh-CN.md)

## Positioning

Tentacle is a "blade" — independent, composable, self-describing (Manifest). It receives standardized invocation requests, executes tools in a secure sandbox, returns structured results, then forgets everything. **Tentacle is not Helix's hands; it is the public armory of the entire AI Agent community.**

- **Generic**: bound to no agent framework; serves all callers equally over HTTP/MCP/gRPC/STDIO
- **Minimal**: <2MB memory at startup; tools loaded on demand, burned after use
- **Self-describing**: every tool carries a complete Manifest (declaration separated from executor, SHA-256 cryptographically bound)
- **Identity flow**: credential label flow (`identity_label`); no plaintext credential stored
- **Progressive foraging**: information-gain evaluator + seen-entropy bloom filter; stops at information saturation
- **Sandbox security**: integrity verification / permission declaration / runtime isolation / automatic redaction / orphan-sandbox defense
- **Optional consensus**: dynamic consensus adaptation (Standalone terminal confirm / Helix CAP / MCP callback)

## Branches

| Branch | Content | Status |
|---|---|---|
| **`rs`** (current) | Rust rebuild, governed by phyt-DNA methodology | **P6 ecosystem integration in progress (M1.5 grpc+fixture done)** (153 tests) |
| **`main`** | early Python version | **kept as philosophical history reference** (history never deleted, loaded on demand) |

## Stage Overview

| Stage | Content | Status | Tests |
|---|---|---|---|
| P0 | early Python version (philosophy reference) | ✅ history | - |
| P1 | Rust rebuild: core skeleton + HTTP transport | ✅ 2026-08-28 | 37 tests |
| P2 | foraging + sandbox (all four fixes delivered) | ✅ 2026-08-29 | 51 tests |
| P3 | tool integration + WASI refinement + worker pool + first built-in tool | ✅ 2026-08-29 | 72 tests |
| **P4** | **ecosystem alignment + gRPC/MCP transports + plugin hot-plug** | **✅ 2026-08-29** | **76+ tests** |
| **P5** | **performance optimization + resource limits + observability + STDIO transport** | **✅ 2026-08-30** | **153 tests** |
| P6 | full ecosystem component integration + CI-144 v2.0 wiring + production deployment | ⏳ in preview | - |

## Transport Support

| Transport | Status | Endpoints | Use cases |
|---|---|---|---|
| **HTTP/REST + SSE** | ✅ done | `GET /v1/manifest`, `GET /v1/tools/{name}/manifest`, `POST /v1/tools/{name}/execute`, `POST /v1/tools/{name}/execute_stream` | any HTTP client, remote deployment, cloud services |
| **gRPC** | ✅ done | `ListManifests`, `GetManifest`, `ExecuteTool`, `ExecuteStream` | Anaphase-Helix internal ecosystem, high-performance binary communication |
| **MCP** | ✅ done | `tools/list`, `tools/call` (JSON-RPC 2.0 over stdio/HTTP/SSE) | MCP clients: Claude Desktop, Codex, etc. |
| **STDIO** | ✅ done | zero-config local use (`echo '{"tool":"...","params":{}}' \| tentacle`) | debugging, script integration, minimal deployment, MCP compatibility |

## Core Capabilities

| Capability | Status | Description |
|---|---|---|
| Tool Manifest | ✅ | declaration separated from executor, SHA-256 integrity verification, progressive disclosure index |
| Credential label flow | ✅ | `identity_labels` unified across three transports; plaintext credentials never in Tentacle memory (Tuck physical edge injection) |
| Progressive foraging | ✅ | `ForagingEvaluator` pure-statistical information-gain evaluation (0 tokens), multi-dimensional stop conditions, Chinese 2-Gram tokenization |
| Seen-entropy bloom filter | ✅ | optional `bloom` feature, interfaces with Callosum, no duplicate collection across sessions |
| WASM sandbox | ✅ | wasmtime v26 + epoch_deadline timeout + WASI preview1 (stdout capture + filesystem permissions) |
| JS coroutine sandbox | ✅ | rquickjs + worker pool (bound to CPU core count) + cooperative interruption + memory limits |
| Plugin hot-plug | ✅ | optional `hot-reload` feature (disabled by default), filesystem monitoring, index real-time update |
| Dynamic consensus adaptation | ✅ | Standalone terminal confirm / Helix CAP / MCP callback three modes |
| Full-transport integration tests | ✅ | 11 end-to-end tests, cross-transport consistency verification for HTTP/gRPC/MCP |
| Performance benchmarks | ✅ | criterion framework, latency/throughput/concurrency benchmarks for core/HTTP/gRPC/MCP transports |
| Unified resource limits | ✅ | ResourceLimiter trait, five-dimension quotas (memory/CPU/FD/timeout/output), WASM/JS sandbox integration |
| Observability | ✅ | MetricsCollector trait + InMemoryMetrics, Prometheus /metrics endpoint, automatic metric recording per tool execution |

## Ecosystem Alignment Chain

```
Helix-Mind ← Anaphase-Helix ← Helix-Tentacle
(memory/cognition)  (orchestration/execution)  (tools/weapons)
```

Tentacle aligns to Anaphase (gRPC contract + credential label flow + Helix consensus mode), Anaphase aligns to Mind (done). The alignment chain is a hard constraint of the rebuild.

**CI-144 v2.0 (PFP-xCF14 + SAP-xCF14)**: frozen; P6 wiring (4-byte fixed-offset PFP header, Modality/Risk-Level/Override-Flag).

## Four-Fix Hard Acceptance (ecosystem alignment)

1. **Credential label flow**: interfaces with Tuck; memory grep finds no plaintext Cookie/Token ✅
2. **Seen-entropy bloom filter**: interfaces with Callosum; no duplicate collection across sessions ✅
3. **Async coroutine sandbox**: ARM edge-side worker pool; 10 concurrent JS tools without OOM/CPU overload ✅
4. **Dynamic consensus adaptation layer**: Standalone/Helix/MCP three modes; standalone mode returns no 503 ✅

## Quick Start

```bash
# clone repository
git clone https://github.com/Jasonmilk/Helix-Tentacle.git
cd Helix-Tentacle
git checkout rs

# run tests
cargo test --workspace

# run full-transport integration tests
cargo test -p tentacle-integration-tests

# run performance benchmarks
cargo bench --package tentacle-benchmarks

# build (enable features on demand)
cargo build --release --features runtime,scraper,bloom

# STDIO mode (zero-config local use)
echo '{"tool":"mock","params":{"input":"test"}}' | ./target/release/tentacle

# HTTP mode
./target/release/tentacle --transport http --port 3000
# endpoints: GET /v1/manifest, GET /metrics, POST /v1/tools/{name}/execute

# specify plugin directory
./target/release/tentacle --transport stdio --plugins-dir ./plugins
```

## Document Navigation

| Document | Path | Description |
|---|---|---|
| Vision index | `docs/VISION.md` | root index (atomic principles + navigation + ecosystem position) |
| Knowledge ontology (complete narrative) | `docs/SPEC.md` | "A Seed's Confession", the reason the project exists |
| Philosophy / architecture / contract / safety volumes | `docs/spec/` | 5 volumes of detailed specs |
| Immutable principles | `docs/DNA.md` | 8 axioms + self-growing process |
| AI collaboration rules + three-layer loading protocol | `docs/RNA.md` | PLAN.md must-read declaration + decision interception rule |
| Retired implementations | `docs/DEPRECATE.md` | retirement records incl. early Python implementation |
| Growth records | `docs/GROWTH.md` | last 3 entries, archive beyond |
| Current-stage navigation | `docs/PLAN.md` | current stage + next-stage preview only |
| Decision records | `docs/decisions/` | ADR-0001 onwards, no overwrite after Active |
| Engineering whitepaper v3.4 | `docs/vision/tentacle-whitepaper-v3.4.md` | complete architecture design |

## Methodology Governance

This project follows the **phyt-DNA methodology v1.0** (methodology anchor project: https://github.com/Jasonmilk/phyt-DNA):

- **N layer (narrative)**: VISION.md + SPEC.md + spec/ volumes
- **D layer (decisions)**: ADR series (Draft/Active two states, no overwrite after Active)
- **A layer (architecture)**: code + contracts (crates/ + proto/)
- **Document lifecycle**: PLAN ≤150 lines, GROWTH ≤3 entries, archives never deleted
- **Commit convention**: commit messages must include ADR association `(ADR-NNNN §Tx)`

---

*Helix-Tentacle (rs) Rust rebuild. Early Python version kept on the main branch as philosophical history reference. Apache 2.0.*
