//! tentacle-transport-http — HTTP 传输层（axum RESTful + SSE）
//!
//! 端点：
//! - GET  /v1/manifest              → 说明书索引（渐进披露）
//! - GET  /v1/tools/{name}/manifest → 完整说明书
//! - POST /v1/tools/{name}/execute  → 执行工具
//! - POST /v1/tools/{name}/execute_stream → SSE 流式执行

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use tentacle_core::{
    consensus::{ConsensusHook, NoopConsensus},
    manifest::Manifest,
    redact::Redactor,
    registry::{ScanReport, ToolRegistry},
    tool::{ExecutionRequest, Tool, ToolOutput},
};
use tower_http::cors::CorsLayer;

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<Mutex<ToolRegistry>>,
    pub redactor: Arc<Redactor>,
    pub consensus: Arc<dyn ConsensusHook>,
}

impl AppState {
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry)),
            redactor: Arc::new(Redactor::default()),
            consensus: Arc::new(NoopConsensus),
        }
    }

    pub fn with_consensus(mut self, consensus: Arc<dyn ConsensusHook>) -> Self {
        self.consensus = consensus;
        self
    }

    /// 扫描插件目录
    pub fn scan_plugins(&self) -> Result<ScanReport, tentacle_core::error::RegistryError> {
        self.registry.lock().unwrap().scan_plugins()
    }

    /// 注册已实例化的工具（用于内置工具或测试）
    pub fn register_tool(&self, tool: Arc<dyn Tool>) -> Result<(), tentacle_core::error::RegistryError> {
        self.registry.lock().unwrap().register_tool(tool)
    }
}

/// 构建 HTTP 路由
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/manifest", get(list_manifests))
        .route("/v1/tools/:name/manifest", get(get_manifest))
        .route("/v1/tools/:name/execute", post(execute_tool))
        .route("/v1/tools/:name/execute_stream", post(execute_stream))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// GET /v1/manifest — 说明书索引（渐进披露，只暴露名称+描述+版本+安全等级）
async fn list_manifests(State(state): State<AppState>) -> impl IntoResponse {
    let registry = state.registry.lock().unwrap();
    let index = registry.index();
    Json(index)
}

/// GET /v1/tools/{name}/manifest — 完整说明书
async fn get_manifest(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let registry = state.registry.lock().unwrap();
    match registry.get_manifest(&name) {
        Some(m) => Json(m.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "tool not found"}))).into_response(),
    }
}

/// POST /v1/tools/{name}/execute — 执行工具
async fn execute_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ExecutionRequest>,
) -> impl IntoResponse {
    // 校验工具名匹配
    if req.tool != name {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "tool name mismatch"})),
        ).into_response();
    }

    let registry = state.registry.lock().unwrap();
    let tool = match registry.get_tool(&name) {
        Some(t) => t.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "tool not found or not instantiated"})),
            ).into_response();
        }
    };

    // Critical 级工具需共识审批（四修正4）
    let manifest = tool.manifest().clone();
    if matches!(manifest.security_level, tentacle_core::manifest::SecurityLevel::Critical) {
        match state.consensus.request_approval(&name, &req.params) {
            Ok(approval) if !approval.approved => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({"error": "consensus denied", "reason": approval.reason})),
                ).into_response();
            }
            Err(e) => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({"error": "consensus unavailable", "detail": e.to_string()})),
                ).into_response();
            }
            _ => {}
        }
    }

    drop(registry);

    // 执行工具
    match tool.execute(req) {
        Ok(mut output) => {
            // 脱敏输出（默认开启不可绕过）
            if let Some(ref mut data) = output.data {
                state.redactor.redact(data);
            }
            Json(output).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ).into_response(),
    }
}

/// POST /v1/tools/{name}/execute_stream — SSE 流式执行
async fn execute_stream(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ExecutionRequest>,
) -> impl IntoResponse {
    if req.tool != name {
        return (
            StatusCode::BAD_REQUEST,
            Sse::new(stream::empty::<Result<Event, Infallible>>()).into_response(),
        ).into_response();
    }

    let registry = state.registry.lock().unwrap();
    let tool = match registry.get_tool(&name) {
        Some(t) => t.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Sse::new(stream::empty::<Result<Event, Infallible>>()).into_response(),
            ).into_response();
        }
    };
    drop(registry);

    // T4：流式执行简化为一次性输出（完整流式在 P2 工具实现后完善）
    match tool.execute(req) {
        Ok(output) => {
            let json = serde_json::to_string(&output).unwrap_or_default();
            let sse_stream = stream::once(async move {
                Ok::<_, Infallible>(Event::default().data(json))
            });
            Sse::new(sse_stream).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Sse::new(stream::empty::<Result<Event, Infallible>>()).into_response(),
        ).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;
    use tentacle_core::manifest::{Integrity, Manifest, SecurityLevel};
    use tentacle_core::tool::{ExecutionRequest, Tool, ToolOutput};
    use tentacle_core::ToolError;
    use tower::ServiceExt;

    struct MockTool {
        manifest: Manifest,
    }
    impl Tool for MockTool {
        fn name(&self) -> &str { &self.manifest.name }
        fn manifest(&self) -> &Manifest { &self.manifest }
        fn execute(&self, _req: ExecutionRequest) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(json!({"result": "ok", "token": "secret"})))
        }
    }

    fn test_state() -> AppState {
        let state = AppState::new(ToolRegistry::new());
        let m = Manifest {
            name: "mock".into(),
            version: "1.0".into(),
            description: "test".into(),
            executable: "mock.wasm".into(),
            integrity: Integrity::sha256("abc"),
            security_level: SecurityLevel::Normal,
            ..Default::default()
        };
        state.register_tool(Arc::new(MockTool { manifest: m })).unwrap();
        state
    }

    #[tokio::test]
    async fn test_list_manifests() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::builder().uri("/v1/manifest").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0]["name"], "mock");
        // 渐进披露：不暴露 integrity/parameters_schema
        assert!(v[0].get("integrity").is_none());
    }

    #[tokio::test]
    async fn test_get_manifest() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::builder().uri("/v1/tools/mock/manifest").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let m: Manifest = serde_json::from_slice(&body).unwrap();
        assert_eq!(m.name, "mock");
    }

    #[tokio::test]
    async fn test_get_manifest_not_found() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::builder().uri("/v1/tools/nonexistent/manifest").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_execute_tool() {
        let app = router(test_state());
        let req = ExecutionRequest {
            tool: "mock".into(),
            params: json!({"input": "test"}),
            identity_labels: Default::default(),
            trace_id: None,
            seen_entropy_bloom: None,
        };
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/mock/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let out: ToolOutput = serde_json::from_slice(&body).unwrap();
        assert!(out.ok);
        // 脱敏：token 字段被替换为 [REDACTED]
        assert_eq!(out.data.unwrap()["token"], "[REDACTED]");
    }

    #[tokio::test]
    async fn test_execute_tool_name_mismatch() {
        let app = router(test_state());
        let req = ExecutionRequest {
            tool: "other".into(),
            params: json!({}),
            identity_labels: Default::default(),
            trace_id: None,
            seen_entropy_bloom: None,
        };
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/mock/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_execute_tool_not_found() {
        let app = router(test_state());
        let req = ExecutionRequest {
            tool: "nonexistent".into(),
            params: json!({}),
            identity_labels: Default::default(),
            trace_id: None,
            seen_entropy_bloom: None,
        };
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/nonexistent/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
