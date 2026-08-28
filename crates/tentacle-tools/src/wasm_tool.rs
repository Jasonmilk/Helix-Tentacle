//! WasmTool — WASM 插件工具（包装 tentacle-wasm 沙箱，实现 Tool trait）
//!
//! P3-T3：执行层对接。将 Manifest + wasm bytes 包装为 Tool trait object，
//! 注册到 ToolRegistry，实现"扫描 → 校验 → 加载 → 执行"的完整链路。

use tentacle_core::error::ToolError;
use tentacle_core::manifest::Manifest;
use tentacle_core::tool::{ExecutionRequest, Tool, ToolOutput};
use tentacle_wasm::{WasmSandbox, WasmOutput};

/// WASM 工具执行器
///
/// 持有 Manifest + wasm bytes + 共享 WasmSandbox。
/// 执行时调用 WasmSandbox.execute()，将结果转换为 ToolOutput。
///
/// # 安全模型
/// - WASM 沙箱隔离：模块只能访问预打开的目录和声明的权限
/// - 完整性校验：加载前校验 SHA-256（由 PluginLoader 负责）
/// - 超时控制：manifest.timeout_ms 控制执行超时
pub struct WasmTool {
    manifest: Manifest,
    wasm_bytes: Vec<u8>,
    sandbox: std::sync::Arc<WasmSandbox>,
}

impl WasmTool {
    /// 创建 WASM 工具
    ///
    /// # 参数
    /// - `manifest`: 工具说明书（含权限配置、超时、完整性校验）
    /// - `wasm_bytes`: WASM 执行体字节（已通过完整性校验）
    /// - `sandbox`: 共享 WasmSandbox（worker 池复用，避免重复创建）
    pub fn new(
        manifest: Manifest,
        wasm_bytes: Vec<u8>,
        sandbox: std::sync::Arc<WasmSandbox>,
    ) -> Self {
        Self {
            manifest,
            wasm_bytes,
            sandbox,
        }
    }

    /// 将 WasmOutput 转换为 ToolOutput
    fn convert_output(&self, output: WasmOutput) -> ToolOutput {
        let data = serde_json::json!({
            "exit_code": output.exit_code,
            "stdout": output.stdout,
        });
        // WASM 执行成功（未超时/未 trap），返回值放在 data 中
        ToolOutput::success(data)
    }
}

impl Tool for WasmTool {
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
            .execute(&self.wasm_bytes, &self.manifest, timeout_ms)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(self.convert_output(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// 简单 WASM 模块（WAT）：返回 42
    const SIMPLE_WAT: &str = r#"
(module
  (func (export "call") (result i32)
    i32.const 42
  )
)
"#;

    #[test]
    fn test_wasm_tool_execute() {
        let sandbox = Arc::new(WasmSandbox::new().unwrap());
        let manifest = Manifest {
            name: "test_wasm".into(),
            version: "1.0.0".into(),
            description: "test".into(),
            executable: "test.wasm".into(),
            timeout_ms: 5000,
            ..Default::default()
        };
        let tool = WasmTool::new(manifest, SIMPLE_WAT.as_bytes().to_vec(), sandbox);

        let req = ExecutionRequest {
            tool: "test_wasm".into(),
            params: serde_json::json!({}),
            identity_labels: HashMap::new(),
            trace_id: None,
            seen_entropy_bloom: None,
        };

        let output = tool.execute(req).unwrap();
        assert!(output.ok);
        let data = output.data.unwrap();
        assert_eq!(data["exit_code"], 42);
    }

    #[test]
    fn test_wasm_tool_name_and_manifest() {
        let sandbox = Arc::new(WasmSandbox::new().unwrap());
        let manifest = Manifest {
            name: "my_tool".into(),
            version: "2.0".into(),
            ..Default::default()
        };
        let tool = WasmTool::new(manifest.clone(), vec![], sandbox);

        assert_eq!(tool.name(), "my_tool");
        assert_eq!(tool.manifest().version, "2.0");
    }

    #[test]
    fn test_wasm_tool_trait_object() {
        let sandbox = Arc::new(WasmSandbox::new().unwrap());
        let manifest = Manifest {
            name: "trait_tool".into(),
            ..Default::default()
        };
        let tool: Box<dyn Tool> = Box::new(WasmTool::new(
            manifest,
            SIMPLE_WAT.as_bytes().to_vec(),
            sandbox,
        ));

        assert_eq!(tool.name(), "trait_tool");
    }
}
