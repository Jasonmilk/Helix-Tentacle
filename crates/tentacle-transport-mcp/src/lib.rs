//! tentacle-transport-mcp — MCP 传输层（Model Context Protocol，对接通用生态）
//!
//! P4-T2：MCP 传输层基础实现。
//!
//! 设计决策：手动实现 MCP 协议（JSON-RPC 2.0 over stdio），而非使用 rmcp 框架。
//! 原因：
//! 1. rmcp 的 #[tool_router] 宏是编译时静态注册，不适合 Tentacle 的动态工具注册
//! 2. MCP 协议本身很简单（JSON-RPC 2.0，几个方法），手动实现更轻量
//! 3. 更符合 Tentacle 的"极致解耦、极致节能"哲学
//!
//! 支持的方法：
//! - initialize：初始化握手
//! - notifications/initialized：客户端初始化完成通知
//! - tools/list：返回工具列表（从 ToolRegistry 动态获取）
//! - tools/call：执行工具
//!
//! 传输层：stdio（每行一个 JSON-RPC 消息）
//! 后续可扩展：HTTP/SSE 传输层

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tentacle_core::manifest::{Manifest, ManifestIndex, SecurityLevel};
use tentacle_core::registry::ToolRegistry;
use tentacle_core::tool::{ExecutionRequest, Tool, ToolOutput};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// MCP 服务器状态
#[derive(Clone)]
pub struct McpServer {
    registry: Arc<Mutex<ToolRegistry>>,
    server_name: String,
    server_version: String,
}

impl McpServer {
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry)),
            server_name: "helix-tentacle".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// 注册已实例化的工具（用于内置工具或测试）
    pub fn register_tool(&self, tool: Arc<dyn Tool>) -> Result<(), tentacle_core::error::RegistryError> {
        self.registry.lock().unwrap().register_tool(tool)
    }

    /// 启动 stdio MCP 服务器（阻塞直到 stdin 关闭）
    pub async fn serve_stdio(&self) -> Result<(), Box<dyn std::error::Error>> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut writer = stdout;

        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                break; // stdin 关闭
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match self.handle_message(line).await {
                Ok(Some(response)) => {
                    let response_json = serde_json::to_string(&response)?;
                    writer.write_all(response_json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                }
                Ok(None) => {
                    // 通知，无响应
                }
                Err(e) => {
                    eprintln!("[MCP] Error handling message: {}", e);
                }
            }
        }

        Ok(())
    }

    /// 处理一条 JSON-RPC 消息
    async fn handle_message(&self, line: &str) -> Result<Option<JsonRpcResponse>, String> {
        let request: JsonRpcRequest = serde_json::from_str(line)
            .map_err(|e| format!("Invalid JSON-RPC request: {}", e))?;

        match request.method.as_str() {
            "initialize" => {
                let result = self.handle_initialize(&request.params);
                Ok(Some(JsonRpcResponse::result(request.id, result)))
            }
            "notifications/initialized" => {
                // 客户端初始化完成，无响应
                Ok(None)
            }
            "tools/list" => {
                let result = self.handle_tools_list();
                Ok(Some(JsonRpcResponse::result(request.id, result)))
            }
            "tools/call" => {
                let result = self.handle_tools_call(&request.params).await;
                Ok(Some(JsonRpcResponse::result(request.id, result)))
            }
            _ => {
                let error = JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                };
                Ok(Some(JsonRpcResponse::error(request.id, error)))
            }
        }
    }

    /// 处理 initialize 请求
    fn handle_initialize(&self, _params: &Option<Value>) -> Value {
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": self.server_name,
                "version": self.server_version
            }
        })
    }

    /// 处理 tools/list 请求
    fn handle_tools_list(&self) -> Value {
        let registry = self.registry.lock().unwrap();
        let index = registry.index();

        let tools: Vec<Value> = index
            .iter()
            .map(|m| {
                // 获取完整说明书以获取 parameters_schema
                let full_manifest = registry.get_manifest(&m.name);
                manifest_to_mcp_tool(m, full_manifest)
            })
            .collect();

        json!({ "tools": tools })
    }

    /// 处理 tools/call 请求
    async fn handle_tools_call(&self, params: &Option<Value>) -> Value {
        let params = match params {
            Some(p) => p,
            None => {
                return tool_error("Missing params");
            }
        };

        let tool_name = params["name"].as_str().unwrap_or("");
        let arguments = params["arguments"].as_object().cloned().unwrap_or_default();

        if tool_name.is_empty() {
            return tool_error("Missing tool name");
        }

        let registry = self.registry.lock().unwrap();
        let tool = match registry.get_tool(tool_name) {
            Some(t) => t.clone(),
            None => {
                return tool_error(&format!("Tool not found: {}", tool_name));
            }
        };
        drop(registry);

        // 构建执行请求
        let execution_req = ExecutionRequest {
            tool: tool_name.to_string(),
            params: Value::Object(arguments),
            identity_labels: HashMap::new(),
            trace_id: None,
            seen_entropy_bloom: None,
        };

        // 执行工具（同步阻塞，在 tokio 任务中执行）
        let tool_clone = tool.clone();
        let result = tokio::task::spawn_blocking(move || {
            tool_clone.execute(execution_req)
        })
        .await;

        match result {
            Ok(Ok(output)) => tool_success(output),
            Ok(Err(e)) => tool_error(&format!("Execution failed: {}", e)),
            Err(e) => tool_error(&format!("Task join error: {}", e)),
        }
    }
}

// === JSON-RPC 消息类型 ===

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    id: Value,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    fn result(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

// === 辅助函数 ===

/// 将 Manifest 转换为 MCP Tool 格式
fn manifest_to_mcp_tool(index: &ManifestIndex, full_manifest: Option<&Manifest>) -> Value {
    let input_schema = full_manifest
        .map(|m| m.parameters_schema.clone())
        .unwrap_or_else(|| json!({"type": "object", "properties": {}}));

    json!({
        "name": index.name,
        "description": index.description,
        "inputSchema": input_schema,
        "annotations": {
            "title": index.name,
            "readOnlyHint": matches!(index.security_level, SecurityLevel::Normal),
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": true
        }
    })
}

/// 构建工具成功响应
fn tool_success(output: ToolOutput) -> Value {
    let content = if let Some(data) = &output.data {
        vec![json!({
            "type": "text",
            "text": serde_json::to_string_pretty(data).unwrap_or_default()
        })]
    } else {
        vec![json!({
            "type": "text",
            "text": "Tool executed successfully"
        })]
    };

    json!({
        "content": content,
        "isError": !output.ok
    })
}

/// 构建工具错误响应
fn tool_error(message: &str) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": message
            }
        ],
        "isError": true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
            Ok(ToolOutput::success(json!({"result": "ok"})))
        }
    }

    fn test_server() -> McpServer {
        let server = McpServer::new(ToolRegistry::new());
        let m = Manifest {
            name: "mock".into(),
            version: "1.0".into(),
            description: "test tool".into(),
            executable: "mock.wasm".into(),
            integrity: Integrity::sha256("abc"),
            security_level: SecurityLevel::Normal,
            parameters_schema: json!({"type": "object", "properties": {"query": {"type": "string"}}}),
            ..Default::default()
        };
        server.register_tool(Arc::new(MockTool { manifest: m })).unwrap();
        server
    }

    #[tokio::test]
    async fn test_initialize() {
        let server = test_server();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "1.0"}
            }
        });

        let response = server.handle_message(&request.to_string()).await.unwrap().unwrap();
        assert_eq!(response.id, json!(1));
        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "helix-tentacle");
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn test_tools_list() {
        let server = test_server();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let response = server.handle_message(&request.to_string()).await.unwrap().unwrap();
        assert_eq!(response.id, json!(2));
        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "mock");
        assert_eq!(tools[0]["description"], "test tool");
        assert!(tools[0]["inputSchema"]["properties"]["query"].is_object());
    }

    #[tokio::test]
    async fn test_tools_call() {
        let server = test_server();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "mock",
                "arguments": {"query": "test"}
            }
        });

        let response = server.handle_message(&request.to_string()).await.unwrap().unwrap();
        assert_eq!(response.id, json!(3));
        let result = response.result.unwrap();
        assert_eq!(result["isError"], false);
        let content = result["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert!(content[0]["text"].as_str().unwrap().contains("ok"));
    }

    #[tokio::test]
    async fn test_tools_call_not_found() {
        let server = test_server();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "nonexistent",
                "arguments": {}
            }
        });

        let response = server.handle_message(&request.to_string()).await.unwrap().unwrap();
        assert_eq!(response.id, json!(4));
        let result = response.result.unwrap();
        assert_eq!(result["isError"], true);
        let content = result["content"].as_array().unwrap();
        assert!(content[0]["text"].as_str().unwrap().contains("Tool not found"));
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let server = test_server();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "nonexistent/method"
        });

        let response = server.handle_message(&request.to_string()).await.unwrap().unwrap();
        assert_eq!(response.id, json!(5));
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_notifications_initialized() {
        let server = test_server();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let response = server.handle_message(&request.to_string()).await.unwrap();
        assert!(response.is_none()); // 通知无响应
    }

    #[tokio::test]
    async fn test_invalid_json() {
        let server = test_server();
        let result = server.handle_message("invalid json").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid JSON-RPC"));
    }

    #[test]
    fn test_manifest_to_mcp_tool() {
        let index = ManifestIndex {
            name: "test".into(),
            description: "test tool".into(),
            version: "1.0".into(),
            security_level: SecurityLevel::Normal,
        };
        let tool = manifest_to_mcp_tool(&index, None);
        assert_eq!(tool["name"], "test");
        assert_eq!(tool["description"], "test tool");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
}
