//! P3-T5：HTTP 传输层端到端集成测试
//!
//! 测试范围：
//! - HTTP 传输层 + WasmTool（真实 WASM 工具）
//! - HTTP 传输层 + JsTool（真实 JS 工具）
//! - HTTP 传输层 + PluginLoader（插件加载 → 注册 → HTTP 执行，完整链路）
//! - SSE 流式执行
//! - 凭证标签透传（X-Identity-Label 头不进入执行请求体）
//!
//! 这些测试验证"传输层 → 工具执行引擎 → 沙箱"的完整链路，
//! 确保 P1-T4（HTTP 传输层）、P3-T3（工具执行引擎）、P3-T4（targeted_scraper）
//! 各组件能够正确协作。

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tentacle_core::manifest::{Integrity, Manifest, SecurityLevel};
use tentacle_core::registry::ToolRegistry;
use tentacle_core::tool::{ExecutionRequest, ToolOutput};
use tentacle_tools::{JsTool, PluginLoader, WasmTool};
use tentacle_transport_http::{router, AppState};
use tentacle_wasm::WasmSandbox;
use tentacle_js::JsSandbox;
use tower::ServiceExt;

/// 简单 WASM 模块（WAT）：返回 42
const SIMPLE_WAT: &str = r#"
(module
  (func (export "call") (result i32)
    i32.const 42
  )
)
"#;

/// 创建测试用 Manifest
fn test_manifest(name: &str, executable: &str) -> Manifest {
    Manifest {
        name: name.into(),
        version: "1.0.0".into(),
        description: format!("test tool: {}", name),
        executable: executable.into(),
        integrity: Integrity::sha256("test"),
        security_level: SecurityLevel::Normal,
        timeout_ms: 5000,
        ..Default::default()
    }
}

/// 创建 HTTP 请求
fn post_execute(name: &str, req: &ExecutionRequest) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/v1/tools/{}/execute", name))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(req).unwrap()))
        .unwrap()
}

/// 解析响应体为 ToolOutput
async fn parse_output(resp: axum::response::Response) -> ToolOutput {
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

// === 测试 1：HTTP 传输层 + WasmTool ===

#[tokio::test]
async fn test_http_wasm_tool_execute() {
    // 1. 创建 WasmTool
    let sandbox = Arc::new(WasmSandbox::new().unwrap());
    let manifest = test_manifest("wasm_calc", "calc.wasm");
    let tool = WasmTool::new(manifest, SIMPLE_WAT.as_bytes().to_vec(), sandbox);

    // 2. 注册到 AppState
    let state = AppState::new(ToolRegistry::new());
    state.register_tool(Arc::new(tool)).unwrap();

    // 3. 通过 HTTP 传输层执行
    let app = router(state);
    let req = ExecutionRequest {
        tool: "wasm_calc".into(),
        params: json!({}),
        identity_labels: HashMap::new(),
        trace_id: None,
        seen_entropy_bloom: None,
    };
    let resp = app.oneshot(post_execute("wasm_calc", &req)).await.unwrap();

    // 4. 验证
    assert_eq!(resp.status(), StatusCode::OK);
    let output = parse_output(resp).await;
    assert!(output.ok);
    assert_eq!(output.data.unwrap()["exit_code"], 42);
}

// === 测试 2：HTTP 传输层 + JsTool ===

#[tokio::test]
async fn test_http_js_tool_execute() {
    // 1. 创建 JsTool
    let sandbox = Arc::new(JsSandbox::default());
    let manifest = test_manifest("js_calc", "calc.js");
    let tool = JsTool::new(manifest, "1 + 41".to_string(), sandbox);

    // 2. 注册到 AppState
    let state = AppState::new(ToolRegistry::new());
    state.register_tool(Arc::new(tool)).unwrap();

    // 3. 通过 HTTP 传输层执行
    let app = router(state);
    let req = ExecutionRequest {
        tool: "js_calc".into(),
        params: json!({}),
        identity_labels: HashMap::new(),
        trace_id: None,
        seen_entropy_bloom: None,
    };
    let resp = app.oneshot(post_execute("js_calc", &req)).await.unwrap();

    // 4. 验证
    assert_eq!(resp.status(), StatusCode::OK);
    let output = parse_output(resp).await;
    assert!(output.ok);
    assert_eq!(output.data.unwrap()["value"], "42");
}

// === 测试 3：HTTP 传输层 + PluginLoader（完整链路） ===

#[tokio::test]
async fn test_http_plugin_loader_end_to_end() {
    // 1. 创建临时插件目录
    let dir = tempfile::tempdir().unwrap();

    // 2. 写 JS 执行体
    let js_code = "'hello' + ' ' + 'world'";
    std::fs::write(dir.path().join("greet.js"), js_code).unwrap();
    let hash = tentacle_core::manifest::compute_sha256(js_code.as_bytes());

    // 3. 写 Manifest
    let manifest_json = json!({
        "name": "greet",
        "version": "1.0.0",
        "description": "greeting tool",
        "executable": "greet.js",
        "integrity": {"algorithm": "sha256", "hash": hash},
        "parameters_schema": {},
        "timeout_ms": 5000
    });
    std::fs::write(
        dir.path().join("greet.manifest.json"),
        serde_json::to_string_pretty(&manifest_json).unwrap(),
    ).unwrap();

    // 4. PluginLoader 加载插件
    let js_sandbox = Arc::new(JsSandbox::default());
    let loader = PluginLoader::new(dir.path()).with_js_sandbox(js_sandbox);
    let mut registry = ToolRegistry::with_plugin_dir(dir.path());
    let report = loader.load_all(&mut registry).unwrap();

    // 验证加载报告
    assert_eq!(report.scanned, 1);
    assert_eq!(report.registered, 1);
    assert!(registry.contains("greet"));
    assert!(registry.get_tool("greet").is_some());

    // 5. 通过 HTTP 传输层执行
    let state = AppState::new(registry);
    let app = router(state);
    let req = ExecutionRequest {
        tool: "greet".into(),
        params: json!({}),
        identity_labels: HashMap::new(),
        trace_id: None,
        seen_entropy_bloom: None,
    };
    let resp = app.oneshot(post_execute("greet", &req)).await.unwrap();

    // 6. 验证端到端结果
    assert_eq!(resp.status(), StatusCode::OK);
    let output = parse_output(resp).await;
    assert!(output.ok);
    assert_eq!(output.data.unwrap()["value"], "hello world");
}

// === 测试 4：SSE 流式执行 ===

#[tokio::test]
async fn test_http_execute_stream() {
    // 1. 创建 JsTool
    let sandbox = Arc::new(JsSandbox::default());
    let manifest = test_manifest("stream_tool", "stream.js");
    let tool = JsTool::new(manifest, "2 * 21".to_string(), sandbox);

    // 2. 注册
    let state = AppState::new(ToolRegistry::new());
    state.register_tool(Arc::new(tool)).unwrap();

    // 3. SSE 流式执行
    let app = router(state);
    let req = ExecutionRequest {
        tool: "stream_tool".into(),
        params: json!({}),
        identity_labels: HashMap::new(),
        trace_id: None,
        seen_entropy_bloom: None,
    };
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/tools/stream_tool/execute_stream")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 4. 验证 SSE 响应
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/event-stream"));

    // 读取 SSE 流
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    // SSE 流应包含 data: 前缀
    assert!(body_str.contains("data:"));
    // 应包含执行结果
    assert!(body_str.contains("42"));
}

// === 测试 5：凭证标签不进入执行结果（脱敏验证） ===

#[tokio::test]
async fn test_http_identity_labels_not_leaked() {
    // 1. 创建 JsTool（返回 identity_labels 验证不泄露）
    let sandbox = Arc::new(JsSandbox::default());
    let manifest = test_manifest("safe_tool", "safe.js");
    // JS 工具只返回简单值，不访问 identity_labels
    let tool = JsTool::new(manifest, "'safe'".to_string(), sandbox);

    // 2. 注册
    let state = AppState::new(ToolRegistry::new());
    state.register_tool(Arc::new(tool)).unwrap();

    // 3. 执行（携带凭证标签）
    let app = router(state);
    let mut labels = HashMap::new();
    labels.insert("weibo".to_string(), "weibo_secret_session_123".to_string());
    let req = ExecutionRequest {
        tool: "safe_tool".into(),
        params: json!({}),
        identity_labels: labels,
        trace_id: None,
        seen_entropy_bloom: None,
    };
    let resp = app.oneshot(post_execute("safe_tool", &req)).await.unwrap();

    // 4. 验证：响应中不包含明文凭证
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    // 明文凭证不应出现在响应中
    assert!(!body_str.contains("weibo_secret_session_123"));
    // 但执行结果应正常返回
    assert!(body_str.contains("safe"));
}

// === 测试 6：工具未注册时返回 404 ===

#[tokio::test]
async fn test_http_tool_not_registered() {
    let state = AppState::new(ToolRegistry::new());
    let app = router(state);

    let req = ExecutionRequest {
        tool: "nonexistent".into(),
        params: json!({}),
        identity_labels: HashMap::new(),
        trace_id: None,
        seen_entropy_bloom: None,
    };
    let resp = app.oneshot(post_execute("nonexistent", &req)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// === 测试 7：说明书索引（渐进披露） ===

#[tokio::test]
async fn test_http_manifest_index() {
    // 1. 注册两个工具
    let state = AppState::new(ToolRegistry::new());
    let js_sandbox = Arc::new(JsSandbox::default());
    state.register_tool(Arc::new(JsTool::new(
        test_manifest("tool_a", "a.js"),
        "1".to_string(),
        js_sandbox.clone(),
    ))).unwrap();
    state.register_tool(Arc::new(JsTool::new(
        test_manifest("tool_b", "b.js"),
        "2".to_string(),
        js_sandbox,
    ))).unwrap();

    // 2. 获取说明书索引
    let app = router(state);
    let resp = app
        .oneshot(Request::builder().uri("/v1/manifest").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // 3. 验证渐进披露
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let index: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(index.len(), 2);
    // 只暴露名称、描述、版本、安全等级
    assert!(index[0].get("name").is_some());
    assert!(index[0].get("description").is_some());
    assert!(index[0].get("version").is_some());
    assert!(index[0].get("security_level").is_some());
    // 不暴露完整性校验、参数 Schema 等敏感信息
    assert!(index[0].get("integrity").is_none());
    assert!(index[0].get("parameters_schema").is_none());
}
