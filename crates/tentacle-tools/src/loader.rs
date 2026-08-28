//! PluginLoader — 插件加载器（扫描 → 校验 → 加载 → 注册）
//!
//! P3-T3：执行层对接。将 P1-T3 的扫描/校验能力与 P2/P3 的沙箱执行能力对接，
//! 实现完整的"插件目录 → ToolRegistry"加载链路。
//!
//! # 流程
//! 1. 扫描插件目录（ToolRegistry::scan_plugins）
//! 2. 校验完整性（SHA-256，P1-T3 已实现）
//! 3. 读取执行体（.wasm / .js）
//! 4. 根据执行体类型创建 WasmTool / JsTool
//! 5. 注册到 ToolRegistry

use tentacle_core::error::ToolError;
#[cfg(any(feature = "wasm", feature = "js"))]
use tentacle_core::manifest::verify_file_integrity;
use tentacle_core::manifest::Manifest;
use tentacle_core::registry::{ScanReport, ToolRegistry};
use tentacle_core::tool::Tool;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "wasm")]
use crate::wasm_tool::WasmTool;
#[cfg(feature = "wasm")]
use tentacle_wasm::WasmSandbox;

#[cfg(feature = "js")]
use crate::js_tool::JsTool;
#[cfg(feature = "js")]
use tentacle_js::JsSandbox;

/// 插件加载器
///
/// 持有共享沙箱（WasmSandbox / JsSandbox），负责从插件目录加载工具并注册到 ToolRegistry。
pub struct PluginLoader {
    plugin_dir: PathBuf,
    #[cfg(feature = "wasm")]
    wasm_sandbox: Option<Arc<WasmSandbox>>,
    #[cfg(feature = "js")]
    js_sandbox: Option<Arc<JsSandbox>>,
}

impl PluginLoader {
    /// 创建插件加载器
    pub fn new(plugin_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugin_dir: plugin_dir.into(),
            #[cfg(feature = "wasm")]
            wasm_sandbox: None,
            #[cfg(feature = "js")]
            js_sandbox: None,
        }
    }

    /// 设置 WASM 沙箱（启用 wasm feature 后可用）
    #[cfg(feature = "wasm")]
    pub fn with_wasm_sandbox(mut self, sandbox: Arc<WasmSandbox>) -> Self {
        self.wasm_sandbox = Some(sandbox);
        self
    }

    /// 设置 JS 沙箱（启用 js feature 后可用）
    #[cfg(feature = "js")]
    pub fn with_js_sandbox(mut self, sandbox: Arc<JsSandbox>) -> Self {
        self.js_sandbox = Some(sandbox);
        self
    }

    /// 扫描并加载所有插件到 ToolRegistry
    ///
    /// # 流程
    /// 1. 扫描插件目录（读取 .manifest.json，校验完整性）
    /// 2. 对每个已注册的 Manifest，读取执行体并创建 Tool
    /// 3. 注册到 ToolRegistry
    ///
    /// # 返回
    /// - `ScanReport`: 扫描报告（含跳过的条目和原因）
    pub fn load_all(&self, registry: &mut ToolRegistry) -> Result<ScanReport, ToolError> {
        // 1. 扫描插件目录（P1-T3 已实现：读取 .manifest.json + 校验完整性）
        let report = registry
            .scan_plugins()
            .map_err(|e| ToolError::ExecutionFailed(format!("scan failed: {}", e)))?;

        // 2. 对每个已注册的 Manifest，加载执行体并创建 Tool
        let names: Vec<String> = registry.index().iter().map(|m| m.name.clone()).collect();
        for name in &names {
            if let Some(manifest) = registry.get_manifest(name) {
                let manifest = manifest.clone();
                match self.load_tool(&manifest) {
                    Ok(tool) => {
                        // 注册工具（如果尚未注册）
                        if registry.get_tool(name).is_none() {
                            let _ = registry.register_tool(tool);
                        }
                    }
                    Err(e) => {
                        // 加载失败，记录到报告（不中断其他工具的加载）
                        eprintln!("[PluginLoader] 加载工具 {} 失败: {}", name, e);
                    }
                }
            }
        }

        Ok(report)
    }

    /// 加载单个工具（根据 Manifest.executable 扩展名选择沙箱）
    fn load_tool(&self, manifest: &Manifest) -> Result<Arc<dyn Tool>, ToolError> {
        let exe_path = self.plugin_dir.join(&manifest.executable);

        if !exe_path.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "执行体不存在: {:?}",
                exe_path
            )));
        }

        // 根据扩展名判断工具类型
        let ext = exe_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "wasm" => self.load_wasm_tool(manifest, &exe_path),
            "js" => self.load_js_tool(manifest, &exe_path),
            other => Err(ToolError::ExecutionFailed(format!(
                "不支持的执行体类型: .{}",
                other
            ))),
        }
    }

    /// 加载 WASM 工具
    #[cfg(feature = "wasm")]
    fn load_wasm_tool(
        &self,
        manifest: &Manifest,
        exe_path: &Path,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        let sandbox = self.wasm_sandbox.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("WASM 沙箱未初始化".to_string())
        })?;

        let wasm_bytes = std::fs::read(exe_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("读取 WASM 失败: {}", e)))?;

        // 完整性校验（双重校验：scan_plugins 已校验，这里再校验一次）
        verify_file_integrity(manifest, &self.plugin_dir)
            .map_err(|e| ToolError::ExecutionFailed(format!("完整性校验失败: {}", e)))?;

        let tool = WasmTool::new(manifest.clone(), wasm_bytes, sandbox.clone());
        Ok(Arc::new(tool))
    }

    /// WASM feature 未启用时的占位
    #[cfg(not(feature = "wasm"))]
    fn load_wasm_tool(
        &self,
        _manifest: &Manifest,
        _exe_path: &Path,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        Err(ToolError::ExecutionFailed(
            "WASM 支持未启用（请启用 --features wasm）".to_string(),
        ))
    }

    /// 加载 JS 工具
    #[cfg(feature = "js")]
    fn load_js_tool(
        &self,
        manifest: &Manifest,
        exe_path: &Path,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        let sandbox = self.js_sandbox.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("JS 沙箱未初始化".to_string())
        })?;

        let js_code = std::fs::read_to_string(exe_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("读取 JS 失败: {}", e)))?;

        // 完整性校验
        verify_file_integrity(manifest, &self.plugin_dir)
            .map_err(|e| ToolError::ExecutionFailed(format!("完整性校验失败: {}", e)))?;

        let tool = JsTool::new(manifest.clone(), js_code, sandbox.clone());
        Ok(Arc::new(tool))
    }

    /// JS feature 未启用时的占位
    #[cfg(not(feature = "js"))]
    fn load_js_tool(
        &self,
        _manifest: &Manifest,
        _exe_path: &Path,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        Err(ToolError::ExecutionFailed(
            "JS 支持未启用（请启用 --features js）".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_loader_new() {
        let loader = PluginLoader::new("/tmp/plugins");
        assert_eq!(loader.plugin_dir, PathBuf::from("/tmp/plugins"));
    }

    #[test]
    fn test_load_all_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let loader = PluginLoader::new(dir.path());
        let mut registry = ToolRegistry::with_plugin_dir(dir.path());

        let report = loader.load_all(&mut registry).unwrap();
        assert_eq!(report.scanned, 0);
        assert_eq!(report.registered, 0);
        assert!(registry.is_empty());
    }

    #[cfg(feature = "js")]
    #[test]
    fn test_load_js_plugin() {
        let dir = tempfile::tempdir().unwrap();

        // 写 JS 执行体
        let js_code = "1 + 41";
        std::fs::write(dir.path().join("calc.js"), js_code).unwrap();
        let hash = tentacle_core::manifest::compute_sha256(js_code.as_bytes());

        // 写 Manifest
        let manifest_json = serde_json::json!({
            "name": "calc",
            "version": "1.0.0",
            "description": "simple calculator",
            "executable": "calc.js",
            "integrity": {"algorithm": "sha256", "hash": hash},
            "parameters_schema": {},
            "timeout_ms": 5000
        });
        std::fs::write(
            dir.path().join("calc.manifest.json"),
            serde_json::to_string_pretty(&manifest_json).unwrap(),
        )
        .unwrap();

        // 加载
        let sandbox = Arc::new(JsSandbox::default());
        let loader = PluginLoader::new(dir.path()).with_js_sandbox(sandbox);
        let mut registry = ToolRegistry::with_plugin_dir(dir.path());

        let report = loader.load_all(&mut registry).unwrap();
        assert_eq!(report.scanned, 1);
        assert_eq!(report.registered, 1);
        assert!(registry.contains("calc"));
        assert!(registry.get_tool("calc").is_some());

        // 执行
        let tool = registry.get_tool("calc").unwrap();
        let req = tentacle_core::tool::ExecutionRequest {
            tool: "calc".into(),
            params: serde_json::json!({}),
            identity_labels: std::collections::HashMap::new(),
            trace_id: None,
            seen_entropy_bloom: None,
        };
        let output = tool.execute(req).unwrap();
        assert!(output.ok);
        assert_eq!(output.data.unwrap()["value"], "42");
    }
}
