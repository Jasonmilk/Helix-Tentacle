//! Tool trait + 执行请求/响应类型
//!
//! 四修正1：ExecutionRequest 用 identity_labels，无明文 credentials 字段。

use crate::error::ToolError;
use crate::manifest::Manifest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 执行请求
///
/// 注意：使用 identity_labels（凭证标签流转），不接受明文 credentials。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub tool: String,
    pub params: serde_json::Value,
    /// 凭证标签（如 {"weibo": "weibo_user_session_1"}）
    /// 四修正1：无明文凭证，只流转标签。
    #[serde(default)]
    pub identity_labels: HashMap<String, String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    /// 已见熵布隆过滤器（Callosum 导出，可选；四修正2）
    #[serde(default)]
    pub seen_entropy_bloom: Option<String>,
}

/// 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub ok: bool,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub stop_reason: Option<StopReason>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

impl ToolOutput {
    pub fn success(data: serde_json::Value) -> Self {
        Self { ok: true, data: Some(data), error: None, stop_reason: None, duration_ms: None }
    }
    pub fn failure(error: impl Into<String>) -> Self {
        Self { ok: false, data: None, error: Some(error.into()), stop_reason: None, duration_ms: None }
    }
    /// 觅食自适应终止（成功完成的优化退场，非错误）
    pub fn foraging_stopped(data: serde_json::Value, reason: StopReason) -> Self {
        Self { ok: true, data: Some(data), error: None, stop_reason: Some(reason), duration_ms: None }
    }
}

/// 流式输出块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputChunk {
    pub data: serde_json::Value,
    #[serde(default)]
    pub done: bool,
}

/// 停止原因（渐进式觅食）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StopReason {
    LowInformationGain { last_gain: f64, threshold: f64 },
    MaxPagesReached { pages: u32 },
    ZeroGainStreak { consecutive_pages: u32 },
}

/// 工具 trait
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn manifest(&self) -> &Manifest;
    fn execute(&self, req: ExecutionRequest) -> Result<ToolOutput, ToolError>;
    fn execute_stream(
        &self,
        _req: ExecutionRequest,
    ) -> Result<Box<dyn Iterator<Item = OutputChunk> + Send>, ToolError> {
        Err(ToolError::StreamNotSupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_request_no_plaintext_credentials() {
        let json = r#"{"tool":"t","params":{},"identity_labels":{"weibo":"s1"}}"#;
        let req: ExecutionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.identity_labels.get("weibo"), Some(&"s1".to_string()));
    }

    #[test]
    fn tool_output_success_and_failure() {
        assert!(ToolOutput::success(serde_json::json!({})).ok);
        assert!(!ToolOutput::failure("x").ok);
    }

    #[test]
    fn foraging_stop_is_success() {
        let out = ToolOutput::foraging_stopped(
            serde_json::json!({}),
            StopReason::LowInformationGain { last_gain: 0.05, threshold: 0.15 },
        );
        assert!(out.ok);
        assert!(out.stop_reason.is_some());
    }

    struct MockTool { manifest: Manifest }
    impl Tool for MockTool {
        fn name(&self) -> &str { &self.manifest.name }
        fn manifest(&self) -> &Manifest { &self.manifest }
        fn execute(&self, _req: ExecutionRequest) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(serde_json::json!({"mock":true})))
        }
    }

    #[test]
    fn mock_tool_trait_object() {
        let m = Manifest { name: "mock".into(), ..Default::default() };
        let tool: Box<dyn Tool> = Box::new(MockTool { manifest: m });
        assert_eq!(tool.name(), "mock");
        let out = tool.execute(ExecutionRequest {
            tool: "mock".into(), params: serde_json::json!({}),
            identity_labels: HashMap::new(), trace_id: None, seen_entropy_bloom: None,
        }).unwrap();
        assert!(out.ok);
    }
}
