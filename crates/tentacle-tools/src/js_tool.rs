//! JsTool — JS 插件工具（包装 tentacle-js 沙箱，实现 Tool trait）
//!
//! P3-T3：执行层对接。将 Manifest + js code 包装为 Tool trait object，
//! 注册到 ToolRegistry，实现"扫描 → 校验 → 加载 → 执行"的完整链路。

use tentacle_core::error::ToolError;
use tentacle_core::manifest::Manifest;
use tentacle_core::tool::{ExecutionRequest, Tool, ToolOutput};
use tentacle_js::{JsSandbox, JsOutput};

/// JS 工具执行器
///
/// 持有 Manifest + js code + 共享 JsSandbox（worker 池）。
/// 执行时调用 JsSandbox.execute()，将结果转换为 ToolOutput。
///
/// # 安全模型
/// - JS 沙箱隔离：worker 池 + 协变中断 + 内存限制 + 危险 API 移除
/// - 完整性校验：加载前校验 SHA-256（由 PluginLoader 负责）
/// - 超时控制：manifest.timeout_ms 控制执行超时
pub struct JsTool {
    manifest: Manifest,
    js_code: String,
    sandbox: std::sync::Arc<JsSandbox>,
}

impl JsTool {
    /// 创建 JS 工具
    ///
    /// # 参数
    /// - `manifest`: 工具说明书（含权限配置、超时、完整性校验）
    /// - `js_code`: JS 源代码（已通过完整性校验）
    /// - `sandbox`: 共享 JsSandbox（worker 池复用，避免重复创建）
    pub fn new(
        manifest: Manifest,
        js_code: String,
        sandbox: std::sync::Arc<JsSandbox>,
    ) -> Self {
        Self {
            manifest,
            js_code,
            sandbox,
        }
    }

    /// 将 JsOutput 转换为 ToolOutput
    fn convert_output(&self, output: JsOutput) -> ToolOutput {
        let data = serde_json::json!({
            "value": output.value,
        });
        ToolOutput::success(data)
    }
}

impl Tool for JsTool {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn execute(&self, _req: ExecutionRequest) -> Result<ToolOutput, ToolError> {
        let timeout_ms = self.manifest.timeout_ms as u64;
        let output = self
            .sandbox
            .execute(&self.js_code, &self.manifest, timeout_ms)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(self.convert_output(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_js_tool_execute() {
        let sandbox = Arc::new(JsSandbox::default());
        let manifest = Manifest {
            name: "test_js".into(),
            version: "1.0.0".into(),
            description: "test".into(),
            executable: "test.js".into(),
            timeout_ms: 5000,
            ..Default::default()
        };
        let tool = JsTool::new(manifest, "1 + 2".to_string(), sandbox);

        let req = ExecutionRequest {
            tool: "test_js".into(),
            params: serde_json::json!({}),
            identity_labels: HashMap::new(),
            trace_id: None,
            seen_entropy_bloom: None,
        };

        let output = tool.execute(req).unwrap();
        assert!(output.ok);
        let data = output.data.unwrap();
        assert_eq!(data["value"], "3");
    }

    #[test]
    fn test_js_tool_name_and_manifest() {
        let sandbox = Arc::new(JsSandbox::default());
        let manifest = Manifest {
            name: "my_js_tool".into(),
            version: "2.0".into(),
            ..Default::default()
        };
        let tool = JsTool::new(manifest.clone(), "1".to_string(), sandbox);

        assert_eq!(tool.name(), "my_js_tool");
        assert_eq!(tool.manifest().version, "2.0");
    }

    #[test]
    fn test_js_tool_trait_object() {
        let sandbox = Arc::new(JsSandbox::default());
        let manifest = Manifest {
            name: "trait_js".into(),
            ..Default::default()
        };
        let tool: Box<dyn Tool> = Box::new(JsTool::new(
            manifest,
            "42".to_string(),
            sandbox,
        ));

        assert_eq!(tool.name(), "trait_js");
    }

    #[test]
    fn test_js_tool_string_result() {
        let sandbox = Arc::new(JsSandbox::default());
        let manifest = Manifest {
            name: "string_tool".into(),
            timeout_ms: 5000,
            ..Default::default()
        };
        let tool = JsTool::new(manifest, "'hello' + ' ' + 'world'".to_string(), sandbox);

        let req = ExecutionRequest {
            tool: "string_tool".into(),
            params: serde_json::json!({}),
            identity_labels: HashMap::new(),
            trace_id: None,
            seen_entropy_bloom: None,
        };

        let output = tool.execute(req).unwrap();
        assert!(output.ok);
        assert_eq!(output.data.unwrap()["value"], "hello world");
    }
}
