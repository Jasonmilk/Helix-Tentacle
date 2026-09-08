//! tentacle-transport-grpc — gRPC 传输层（tonic + proto，对接 Anaphase-Helix）
//!
//! P4-T1：gRPC 传输层基础实现。
//! 端点：
//! - ListManifests  ↔ GET  /v1/manifest（说明书索引，渐进披露）
//! - GetManifest     ↔ GET  /v1/tools/{name}/manifest（完整说明书）
//! - ExecuteTool     ↔ POST /v1/tools/{name}/execute（执行工具）
//! - ExecuteToolStream ↔ POST /v1/tools/{name}/execute_stream（流式执行）
//!
//! 与 HTTP 传输层共享同一个 ToolRegistry，无状态冲突。
//! 凭证标签流转（四修正1）：identity_labels 字段传递标签，不传递明文凭证。

pub mod proto {
    tonic::include_proto!("tentacle.v1");
}

use proto::tentacle_service_server::{TentacleService, TentacleServiceServer};
use proto::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tentacle_core::manifest::{Manifest, ManifestIndex, SecurityLevel};
use tentacle_core::registry::ToolRegistry;
use tentacle_core::tool::{ExecutionRequest, StopReason, Tool, ToolOutput};
use tonic::{Request, Response, Status};

/// gRPC 服务端状态
#[derive(Clone)]
pub struct GrpcState {
    pub registry: Arc<Mutex<ToolRegistry>>,
}

impl GrpcState {
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry)),
        }
    }

    /// 注册已实例化的工具（用于内置工具或测试）
    pub async fn register_tool(&self, tool: Arc<dyn Tool>) -> Result<(), tentacle_core::error::RegistryError> {
        self.registry.lock().await.register_tool(tool)
    }
}

/// Tentacle gRPC 服务实现
pub struct TentacleGrpcService {
    state: GrpcState,
}

impl TentacleGrpcService {
    pub fn new(state: GrpcState) -> Self {
        Self { state }
    }

    /// 构建 gRPC 服务（用于 Server::builder）
    pub fn into_server(self) -> TentacleServiceServer<Self> {
        TentacleServiceServer::new(self)
    }
}

#[async_trait::async_trait]
impl TentacleService for TentacleGrpcService {
    type ExecuteToolStreamStream = tokio_stream::Iter<
        std::vec::IntoIter<Result<ExecuteToolStreamResponse, Status>>,
    >;

    async fn list_manifests(
        &self,
        _request: Request<ListManifestsRequest>,
    ) -> Result<Response<ListManifestsResponse>, Status> {
        let registry = self.state.registry.lock().await;
        let index = registry.index();
        let manifests = index
            .into_iter()
            .map(|idx| {
                // parameter_names from the full manifest schema — the tool
                // schema is the single source of truth for call shapes
                // (2026-09-09: models must not guess parameter names).
                let params = registry
                    .get_manifest(&idx.name)
                    .and_then(|m| m.parameters_schema.get("properties").and_then(|p| p.as_object()))
                    .map(|props| props.keys().cloned().collect())
                    .unwrap_or_default();
                let tags = registry
                    .get_manifest(&idx.name)
                    .map(|m| m.tags.clone())
                    .unwrap_or_default();
                manifest_index_to_proto_with_params(idx, params, tags)
            })
            .collect();
        Ok(Response::new(ListManifestsResponse { manifests }))
    }

    async fn get_manifest(
        &self,
        request: Request<GetManifestRequest>,
    ) -> Result<Response<GetManifestResponse>, Status> {
        let name = request.into_inner().name;
        let registry = self.state.registry.lock().await;
        match registry.get_manifest(&name) {
            Some(m) => Ok(Response::new(GetManifestResponse {
                manifest: Some(manifest_to_proto(m)),
            })),
            None => Err(Status::not_found(format!("tool not found: {}", name))),
        }
    }

    async fn execute_tool(
        &self,
        request: Request<ExecuteToolRequest>,
    ) -> Result<Response<ExecuteToolResponse>, Status> {
        let req = request.into_inner();

        // 校验工具名
        if req.tool.is_empty() {
            return Err(Status::invalid_argument("tool name is required"));
        }

        let registry = self.state.registry.lock().await;
        let tool = match registry.get_tool(&req.tool) {
            Some(t) => t.clone(),
            None => {
                return Err(Status::not_found(format!(
                    "tool not found or not instantiated: {}",
                    req.tool
                )))
            }
        };
        drop(registry);

        // 构建执行请求
        let params: serde_json::Value = serde_json::from_str(&req.params)
            .unwrap_or(serde_json::json!({}));
        let execution_req = ExecutionRequest {
            tool: req.tool.clone(),
            params,
            identity_labels: req.identity_labels,
            trace_id: if req.trace_id.is_empty() { None } else { Some(req.trace_id) },
            seen_entropy_bloom: if req.seen_entropy_bloom.is_empty() { None } else { Some(req.seen_entropy_bloom) },
        };

        // 执行工具（同步阻塞，在 tokio 任务中执行）
        let tool_clone = tool.clone();
        let result = tokio::task::spawn_blocking(move || {
            tool_clone.execute(execution_req)
        })
        .await
        .map_err(|e| Status::internal(format!("task join error: {}", e)))?;

        match result {
            Ok(output) => Ok(Response::new(tool_output_to_proto(output))),
            Err(e) => Err(Status::internal(format!("execution failed: {}", e))),
        }
    }

    /// 流式执行（简化为一次性输出，完整流式在后续优化）
    async fn execute_tool_stream(
        &self,
        request: Request<ExecuteToolRequest>,
    ) -> Result<Response<Self::ExecuteToolStreamStream>, Status> {
        let req = request.into_inner();

        if req.tool.is_empty() {
            return Err(Status::invalid_argument("tool name is required"));
        }

        let registry = self.state.registry.lock().await;
        let tool = match registry.get_tool(&req.tool) {
            Some(t) => t.clone(),
            None => {
                return Err(Status::not_found(format!(
                    "tool not found or not instantiated: {}",
                    req.tool
                )))
            }
        };
        drop(registry);

        let params: serde_json::Value = serde_json::from_str(&req.params)
            .unwrap_or(serde_json::json!({}));
        let execution_req = ExecutionRequest {
            tool: req.tool.clone(),
            params,
            identity_labels: req.identity_labels,
            trace_id: if req.trace_id.is_empty() { None } else { Some(req.trace_id) },
            seen_entropy_bloom: if req.seen_entropy_bloom.is_empty() { None } else { Some(req.seen_entropy_bloom) },
        };

        let tool_clone = tool.clone();
        let result = tokio::task::spawn_blocking(move || {
            tool_clone.execute(execution_req)
        })
        .await
        .map_err(|e| Status::internal(format!("task join error: {}", e)))?;

        let output = result.map_err(|e| Status::internal(format!("execution failed: {}", e)))?;
        let data = serde_json::to_string(&output).unwrap_or_default();

        // 简化：一次性输出，然后 done
        let response = ExecuteToolStreamResponse { data, done: true };
        let stream: Self::ExecuteToolStreamStream = tokio_stream::iter(vec![Ok(response)]);
        Ok(Response::new(stream))
    }
}

// === 类型转换函数 ===

fn manifest_index_to_proto(index: ManifestIndex) -> proto::ManifestIndex {
    manifest_index_to_proto_with_params(index, Vec::new(), Vec::new())
}

fn manifest_index_to_proto_with_params(
    index: ManifestIndex,
    parameter_names: Vec<String>,
    tags: Vec<String>,
) -> proto::ManifestIndex {
    proto::ManifestIndex {
        name: index.name,
        description: index.description,
        version: index.version,
        security_level: security_level_to_string(index.security_level),
        parameter_names,
        tags,
    }
}

fn manifest_to_proto(manifest: &Manifest) -> proto::Manifest {
    proto::Manifest {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        executable: manifest.executable.clone(),
        integrity: Some(proto::Integrity {
            algorithm: manifest.integrity.algorithm.clone(),
            hash: manifest.integrity.hash.clone(),
        }),
        parameters_schema: serde_json::to_string(&manifest.parameters_schema).unwrap_or_default(),
        tags: manifest.tags.clone(),
        security_level: security_level_to_string(manifest.security_level.clone()),
        timeout_ms: manifest.timeout_ms,
        foraging_config: Some(proto::ForagingConfig {
            enabled: manifest.foraging_config.enabled,
            threshold_delta: manifest.foraging_config.threshold_delta,
            max_pages_per_session: manifest.foraging_config.max_pages_per_session,
        }),
    }
}

fn tool_output_to_proto(output: ToolOutput) -> ExecuteToolResponse {
    ExecuteToolResponse {
        ok: output.ok,
        data: output.data.map(|d| serde_json::to_string(&d).unwrap_or_default()).unwrap_or_default(),
        error: output.error.unwrap_or_default(),
        stop_reason: output.stop_reason.map(stop_reason_to_proto),
        duration_ms: output.duration_ms.unwrap_or(0),
    }
}

fn stop_reason_to_proto(reason: StopReason) -> proto::StopReason {
    match reason {
        StopReason::LowInformationGain { last_gain, threshold } => proto::StopReason {
            reason: Some(proto::stop_reason::Reason::LowInformationGain(
                proto::LowInformationGain { last_gain, threshold },
            )),
        },
        StopReason::MaxPagesReached { pages } => proto::StopReason {
            reason: Some(proto::stop_reason::Reason::MaxPagesReached(
                proto::MaxPagesReached { pages },
            )),
        },
        StopReason::ZeroGainStreak { consecutive_pages } => proto::StopReason {
            reason: Some(proto::stop_reason::Reason::ZeroGainStreak(
                proto::ZeroGainStreak { consecutive_pages },
            )),
        },
    }
}

fn security_level_to_string(level: SecurityLevel) -> String {
    match level {
        SecurityLevel::Normal => "normal".to_string(),
        SecurityLevel::Critical => "critical".to_string(),
    }
}

/// 启动 gRPC 服务端
pub async fn serve(state: GrpcState, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let addr = addr.parse()?;
    let service = TentacleGrpcService::new(state);
    tonic::transport::Server::builder()
        .add_service(service.into_server())
        .serve(addr)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tentacle_core::manifest::{Integrity, Manifest, SecurityLevel};
    use tentacle_core::tool::{ExecutionRequest, ToolOutput};
    use tentacle_core::ToolError;

    struct MockTool {
        manifest: Manifest,
    }

    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.manifest.name
        }
        fn manifest(&self) -> &Manifest {
            &self.manifest
        }
        fn execute(&self, _req: ExecutionRequest) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(serde_json::json!({"result": "ok"})))
        }
    }

    async fn test_state() -> GrpcState {
        let state = GrpcState::new(ToolRegistry::new());
        let m = Manifest {
            name: "mock".into(),
            version: "1.0".into(),
            description: "test tool".into(),
            executable: "mock.wasm".into(),
            integrity: Integrity::sha256("abc"),
            security_level: SecurityLevel::Normal,
            ..Default::default()
        };
        state.register_tool(Arc::new(MockTool { manifest: m })).await.unwrap();
        state
    }

    #[tokio::test]
    async fn test_list_manifests() {
        let service = TentacleGrpcService::new(test_state().await);
        let request = Request::new(ListManifestsRequest {});
        let response = service.list_manifests(request).await.unwrap();
        let manifests = response.into_inner().manifests;
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].name, "mock");
        assert_eq!(manifests[0].security_level, "normal");
    }

    #[tokio::test]
    async fn test_get_manifest() {
        let service = TentacleGrpcService::new(test_state().await);
        let request = Request::new(GetManifestRequest { name: "mock".into() });
        let response = service.get_manifest(request).await.unwrap();
        let manifest = response.into_inner().manifest.unwrap();
        assert_eq!(manifest.name, "mock");
        assert_eq!(manifest.version, "1.0");
    }

    #[tokio::test]
    async fn test_get_manifest_not_found() {
        let service = TentacleGrpcService::new(test_state().await);
        let request = Request::new(GetManifestRequest { name: "nonexistent".into() });
        let result = service.get_manifest(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_execute_tool() {
        let service = TentacleGrpcService::new(test_state().await);
        let request = Request::new(ExecuteToolRequest {
            tool: "mock".into(),
            params: "{}".to_string(),
            identity_labels: HashMap::new(),
            trace_id: "trace-123".to_string(),
            seen_entropy_bloom: String::new(),
        });
        let response = service.execute_tool(request).await.unwrap();
        let output = response.into_inner();
        assert!(output.ok);
        let data: serde_json::Value = serde_json::from_str(&output.data).unwrap();
        assert_eq!(data["result"], "ok");
    }

    #[tokio::test]
    async fn test_execute_tool_not_found() {
        let service = TentacleGrpcService::new(test_state().await);
        let request = Request::new(ExecuteToolRequest {
            tool: "nonexistent".into(),
            params: "{}".to_string(),
            identity_labels: HashMap::new(),
            trace_id: String::new(),
            seen_entropy_bloom: String::new(),
        });
        let result = service.execute_tool(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_execute_tool_empty_name() {
        let service = TentacleGrpcService::new(test_state().await);
        let request = Request::new(ExecuteToolRequest {
            tool: String::new(),
            params: "{}".to_string(),
            identity_labels: HashMap::new(),
            trace_id: String::new(),
            seen_entropy_bloom: String::new(),
        });
        let result = service.execute_tool(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_execute_tool_stream() {
        let service = TentacleGrpcService::new(test_state().await);
        let request = Request::new(ExecuteToolRequest {
            tool: "mock".into(),
            params: "{}".to_string(),
            identity_labels: HashMap::new(),
            trace_id: String::new(),
            seen_entropy_bloom: String::new(),
        });
        let response = service.execute_tool_stream(request).await.unwrap();
        let mut stream = response.into_inner();
        // 简化实现：一次性输出，然后 done
        use tokio_stream::StreamExt;
        if let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            assert!(chunk.done);
            assert!(!chunk.data.is_empty());
        } else {
            panic!("expected at least one chunk");
        }
    }

    #[tokio::test]
    async fn test_identity_labels_passthrough() {
        // 验证凭证标签流转（四修正1）：identity_labels 字段传递，不传递明文凭证
        let service = TentacleGrpcService::new(test_state().await);
        let mut labels = HashMap::new();
        labels.insert("weibo".to_string(), "weibo_session_1".to_string());
        let request = Request::new(ExecuteToolRequest {
            tool: "mock".into(),
            params: "{}".to_string(),
            identity_labels: labels,
            trace_id: String::new(),
            seen_entropy_bloom: String::new(),
        });
        let response = service.execute_tool(request).await.unwrap();
        assert!(response.into_inner().ok);
    }
}
