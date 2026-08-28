//! P4-T5 全传输层集成测试
//!
//! 测试 HTTP/gRPC/MCP 三传输层的端到端功能，以及跨传输层一致性。

use std::sync::Arc;

use serde_json::{json, Value};
use tentacle_core::manifest::{Integrity, Manifest, SecurityLevel};
use tentacle_core::registry::ToolRegistry;
use tentacle_core::tool::{ExecutionRequest, Tool, ToolOutput};
use tentacle_core::ToolError;

// === 测试工具 ===

/// 简单的 Mock 工具，返回固定结果
#[derive(Clone)]
struct EchoTool {
    manifest: Manifest,
}

impl EchoTool {
    fn new() -> Self {
        Self {
            manifest: Manifest {
                name: "echo".into(),
                version: "1.0".into(),
                description: "Echo tool for integration tests".into(),
                executable: "echo.wasm".into(),
                integrity: Integrity::sha256("abc123"),
                security_level: SecurityLevel::Normal,
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    },
                    "required": ["message"]
                }),
                ..Default::default()
            },
        }
    }
}

impl Tool for EchoTool {
    fn name(&self) -> &str {
        &self.manifest.name
    }
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn execute(&self, req: ExecutionRequest) -> Result<ToolOutput, ToolError> {
        let message = req.params["message"].as_str().unwrap_or("");
        Ok(ToolOutput::success(json!({
            "echo": message,
            "tool": "echo",
            "identity_labels_count": req.identity_labels.len()
        })))
    }
}

/// 创建包含 EchoTool 的测试注册器
fn test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register_tool(Arc::new(EchoTool::new())).unwrap();
    registry
}

// === HTTP 传输层测试 ===

#[tokio::test]
async fn test_http_manifest_index() {
    let state = tentacle_transport_http::AppState::new(test_registry());
    let app = tentacle_transport_http::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/v1/manifest", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    let tools = body.as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "echo");
    assert_eq!(tools[0]["description"], "Echo tool for integration tests");
}

#[tokio::test]
async fn test_http_execute_tool() {
    let state = tentacle_transport_http::AppState::new(test_registry());
    let app = tentacle_transport_http::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/v1/tools/echo/execute", addr))
        .json(&json!({
            "tool": "echo", "params": {"message": "hello http"},
            "identity_labels": {"test": "label1"}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert!(body["ok"].as_bool().unwrap());
    assert_eq!(body["data"]["echo"], "hello http");
    assert_eq!(body["data"]["tool"], "echo");
    assert_eq!(body["data"]["identity_labels_count"], 1);
}

#[tokio::test]
async fn test_http_execute_tool_not_found() {
    let state = tentacle_transport_http::AppState::new(test_registry());
    let app = tentacle_transport_http::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/v1/tools/nonexistent/execute", addr))
        .json(&json!({"tool": "nonexistent", "params": {}}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

// === gRPC 传输层测试 ===

#[tokio::test]
async fn test_grpc_list_manifests() {
    use tentacle_transport_grpc::proto::tentacle_service_client::TentacleServiceClient;
    use tentacle_transport_grpc::proto::ListManifestsRequest;

    let state = tentacle_transport_grpc::GrpcState::new(test_registry());
    let service = tentacle_transport_grpc::TentacleGrpcService::new(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let mut client = TentacleServiceClient::connect(format!("http://{}", addr))
        .await
        .unwrap();

    let response = client
        .list_manifests(ListManifestsRequest {})
        .await
        .unwrap();

    let manifests = response.into_inner().manifests;
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].name, "echo");
    assert_eq!(manifests[0].security_level, "normal");
}

#[tokio::test]
async fn test_grpc_execute_tool() {
    use std::collections::HashMap;
    use tentacle_transport_grpc::proto::tentacle_service_client::TentacleServiceClient;
    use tentacle_transport_grpc::proto::ExecuteToolRequest;

    let state = tentacle_transport_grpc::GrpcState::new(test_registry());
    let service = tentacle_transport_grpc::TentacleGrpcService::new(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let mut client = TentacleServiceClient::connect(format!("http://{}", addr))
        .await
        .unwrap();

    let mut identity_labels = HashMap::new();
    identity_labels.insert("test".to_string(), "label1".to_string());

    let request = ExecuteToolRequest {
        tool: "echo".to_string(),
        params: json!({"message": "hello grpc"}).to_string(),
        identity_labels,
        trace_id: "trace-test".to_string(),
        seen_entropy_bloom: String::new(),
    };

    let response = client.execute_tool(request).await.unwrap();
    let output = response.into_inner();
    assert!(output.ok);
    let data: Value = serde_json::from_str(&output.data).unwrap();
    assert_eq!(data["echo"], "hello grpc");
    assert_eq!(data["tool"], "echo");
    assert_eq!(data["identity_labels_count"], 1);
}

#[tokio::test]
async fn test_grpc_execute_tool_not_found() {
    use tentacle_transport_grpc::proto::tentacle_service_client::TentacleServiceClient;
    use tentacle_transport_grpc::proto::ExecuteToolRequest;

    let state = tentacle_transport_grpc::GrpcState::new(test_registry());
    let service = tentacle_transport_grpc::TentacleGrpcService::new(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let mut client = TentacleServiceClient::connect(format!("http://{}", addr))
        .await
        .unwrap();

    let request = ExecuteToolRequest {
        tool: "nonexistent".to_string(),
        params: "{}".to_string(),
        identity_labels: Default::default(),
        trace_id: String::new(),
        seen_entropy_bloom: String::new(),
    };

    let result = client.execute_tool(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

// === MCP 传输层测试（直接调用 handle_message，不通过 stdio） ===

#[tokio::test]
async fn test_mcp_tools_list() {
    let server = tentacle_transport_mcp::McpServer::new(test_registry());

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });

    let response = server.handle_message(&request.to_string()).await.unwrap().unwrap();
    assert_eq!(response.id, json!(1));
    let result = response.result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "echo");
    assert!(tools[0]["inputSchema"]["properties"]["message"].is_object());
}

#[tokio::test]
async fn test_mcp_tools_call() {
    let server = tentacle_transport_mcp::McpServer::new(test_registry());

    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "echo",
            "arguments": {"message": "hello mcp"},
            "_meta": {
                "identityLabels": {"test": "label1"}
            }
        }
    });

    let response = server.handle_message(&request.to_string()).await.unwrap().unwrap();
    assert_eq!(response.id, json!(2));
    let result = response.result.unwrap();
    assert_eq!(result["isError"], false);
    let content = result["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    let text = content[0]["text"].as_str().unwrap();
    let data: Value = serde_json::from_str(text).unwrap();
    assert_eq!(data["echo"], "hello mcp");
    assert_eq!(data["tool"], "echo");
    assert_eq!(data["identity_labels_count"], 1);
}

#[tokio::test]
async fn test_mcp_tools_call_not_found() {
    let server = tentacle_transport_mcp::McpServer::new(test_registry());

    let request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "nonexistent",
            "arguments": {}
        }
    });

    let response = server.handle_message(&request.to_string()).await.unwrap().unwrap();
    let result = response.result.unwrap();
    assert_eq!(result["isError"], true);
    let content = result["content"].as_array().unwrap();
    assert!(content[0]["text"].as_str().unwrap().contains("Tool not found"));
}

// === 跨传输层一致性测试 ===

#[tokio::test]
async fn test_cross_transport_consistency() {
    // 三传输层调用同一工具，验证结果一致
    let message = "consistency test";

    // HTTP
    let http_state = tentacle_transport_http::AppState::new(test_registry());
    let http_app = tentacle_transport_http::router(http_state);
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_addr = http_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(http_listener, http_app).await.unwrap();
    });

    // gRPC
    let grpc_state = tentacle_transport_grpc::GrpcState::new(test_registry());
    let grpc_service = tentacle_transport_grpc::TentacleGrpcService::new(grpc_state);
    let grpc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr = grpc_listener.local_addr().unwrap();
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(grpc_service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(grpc_listener))
            .await
            .unwrap();
    });

    // MCP
    let mcp_server = tentacle_transport_mcp::McpServer::new(test_registry());

    // HTTP 调用
    let client = reqwest::Client::new();
    let http_response = client
        .post(format!("http://{}/v1/tools/echo/execute", http_addr))
        .json(&json!({"tool": "echo", "params": {"message": message}}))
        .send()
        .await
        .unwrap();
    let http_body: Value = http_response.json().await.unwrap();
    let http_echo = http_body["data"]["echo"].as_str().unwrap();

    // gRPC 调用
    use tentacle_transport_grpc::proto::tentacle_service_client::TentacleServiceClient;
    use tentacle_transport_grpc::proto::ExecuteToolRequest;
    let mut grpc_client = TentacleServiceClient::connect(format!("http://{}", grpc_addr))
        .await
        .unwrap();
    let grpc_request = ExecuteToolRequest {
        tool: "echo".to_string(),
        params: json!({"message": message}).to_string(),
        identity_labels: Default::default(),
        trace_id: String::new(),
        seen_entropy_bloom: String::new(),
    };
    let grpc_response = grpc_client.execute_tool(grpc_request).await.unwrap();
    let grpc_data: Value = serde_json::from_str(&grpc_response.into_inner().data).unwrap();
    let grpc_echo = grpc_data["echo"].as_str().unwrap();

    // MCP 调用
    let mcp_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "echo",
            "arguments": {"message": message}
        }
    });
    let mcp_response = mcp_server.handle_message(&mcp_request.to_string()).await.unwrap().unwrap();
    let mcp_content = mcp_response.result.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    let mcp_data: Value = serde_json::from_str(&mcp_content).unwrap();
    let mcp_echo = mcp_data["echo"].as_str().unwrap();

    // 验证三传输层结果一致
    assert_eq!(http_echo, message);
    assert_eq!(grpc_echo, message);
    assert_eq!(mcp_echo, message);
    assert_eq!(http_echo, grpc_echo);
    assert_eq!(grpc_echo, mcp_echo);
}

#[tokio::test]
async fn test_cross_transport_manifest_consistency() {
    // 三传输层的说明书索引应该一致
    let http_state = tentacle_transport_http::AppState::new(test_registry());
    let http_app = tentacle_transport_http::router(http_state);
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_addr = http_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(http_listener, http_app).await.unwrap();
    });

    let grpc_state = tentacle_transport_grpc::GrpcState::new(test_registry());
    let grpc_service = tentacle_transport_grpc::TentacleGrpcService::new(grpc_state);
    let grpc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr = grpc_listener.local_addr().unwrap();
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(grpc_service.into_server())
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(grpc_listener))
            .await
            .unwrap();
    });

    let mcp_server = tentacle_transport_mcp::McpServer::new(test_registry());

    // HTTP
    let client = reqwest::Client::new();
    let http_response = client
        .get(format!("http://{}/v1/manifest", http_addr))
        .send()
        .await
        .unwrap();
    let http_body: Value = http_response.json().await.unwrap();
    let http_name = http_body[0]["name"].as_str().unwrap();

    // gRPC
    use tentacle_transport_grpc::proto::tentacle_service_client::TentacleServiceClient;
    use tentacle_transport_grpc::proto::ListManifestsRequest;
    let mut grpc_client = TentacleServiceClient::connect(format!("http://{}", grpc_addr))
        .await
        .unwrap();
    let grpc_response = grpc_client.list_manifests(ListManifestsRequest {}).await.unwrap();
    let grpc_name = &grpc_response.into_inner().manifests[0].name;

    // MCP
    let mcp_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });
    let mcp_response = mcp_server.handle_message(&mcp_request.to_string()).await.unwrap().unwrap();
    let mcp_result = mcp_response.result.unwrap();
    let mcp_name = mcp_result["tools"][0]["name"].as_str().unwrap();

    // 验证三传输层说明书索引一致
    assert_eq!(http_name, "echo");
    assert_eq!(grpc_name, "echo");
    assert_eq!(mcp_name, "echo");
}
