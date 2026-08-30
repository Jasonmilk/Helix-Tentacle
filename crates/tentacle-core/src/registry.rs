//! ToolRegistry——工具注册器（渐进披露 + 完整性校验 + 懒加载）
//!
//! T2 仅定义类型与接口签名，实际文件扫描/运行时加载在 T3+ 实现。

use crate::error::RegistryError;
use crate::manifest::{verify_file_integrity, Manifest, ManifestIndex};
use crate::tool::Tool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// 插件扫描报告（白盒可观测）
#[derive(Debug, Default, Clone)]
pub struct ScanReport {
    /// 发现的 manifest 文件数
    pub scanned: usize,
    /// 成功注册数
    pub registered: usize,
    /// 跳过的条目（含原因）
    pub skipped: Vec<SkippedEntry>,
}

/// 跳过的条目
#[derive(Debug, Clone)]
pub struct SkippedEntry {
    pub manifest_path: PathBuf,
    pub reason: String,
}

/// 工具注册器
///
/// 职责：
/// - 扫描 plugins/ 目录，构建轻量说明书索引（渐进披露）
/// - 校验 Manifest 完整性（SHA-256 绑定）
/// - 懒加载工具运行时（执行请求到达时才加载）
/// - 维护已注册工具的索引
#[derive(Default)]
pub struct ToolRegistry {
    /// 已注册的 Manifest（名称 → 说明书）
    manifests: HashMap<String, Manifest>,
    /// 已实例化的工具（名称 → Tool trait object）
    ///
    /// 懒加载：执行请求到达时才实例化，T2 阶段仅提供注册接口。
    tools: HashMap<String, Arc<dyn Tool>>,
    /// 插件目录路径
    plugin_dir: Option<PathBuf>,
}

impl ToolRegistry {
    /// 创建空注册器
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建注册器并指定插件目录
    pub fn with_plugin_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            plugin_dir: Some(dir.into()),
            ..Default::default()
        }
    }

    /// 注册工具 Manifest（T2：手动注册；T3+：从插件目录扫描）
    pub fn register_manifest(&mut self, manifest: Manifest) -> Result<(), RegistryError> {
        if self.manifests.contains_key(&manifest.name) {
            return Err(RegistryError::Duplicate(manifest.name.clone()));
        }
        self.manifests.insert(manifest.name.clone(), manifest);
        Ok(())
    }

    /// 注册已实例化的工具（用于内置工具或测试）
    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) -> Result<(), RegistryError> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(RegistryError::Duplicate(name));
        }
        // 同步注册 Manifest
        if !self.manifests.contains_key(&name) {
            self.manifests.insert(name.clone(), tool.manifest().clone());
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// 获取轻量说明书索引（渐进披露：只暴露名称+描述+版本+安全等级）
    pub fn index(&self) -> Vec<ManifestIndex> {
        self.manifests.values().map(ManifestIndex::from).collect()
    }

    /// 获取当前平台支持的轻量说明书索引（平台感知过滤）
    ///
    /// 只返回 `platform_support.host_os` 包含当前 OS，或 `host_os` 为空（通用工具）的工具。
    pub fn index_for_current_platform(&self) -> Vec<ManifestIndex> {
        let current = std::env::consts::OS;
        self.manifests
            .values()
            .filter(|m| {
                m.platform_support.host_os.is_empty()
                    || m.platform_support.host_os.iter().any(|os| os == current)
            })
            .map(ManifestIndex::from)
            .collect()
    }

    /// 获取指定 OS 支持的轻量说明书索引（平台感知过滤）
    pub fn index_for_os(&self, os: &str) -> Vec<ManifestIndex> {
        self.manifests
            .values()
            .filter(|m| {
                m.platform_support.host_os.is_empty()
                    || m.platform_support.host_os.iter().any(|host_os| host_os == os)
            })
            .map(ManifestIndex::from)
            .collect()
    }

    /// 检查指定工具是否在当前平台可用
    pub fn is_supported_on_current_platform(&self, name: &str) -> bool {
        self.manifests
            .get(name)
            .map(|m| m.platform_support.is_supported_on_current_platform())
            .unwrap_or(false)
    }

    /// 获取当前平台支持的工具名称列表
    pub fn supported_tool_names(&self) -> Vec<String> {
        let current = std::env::consts::OS;
        self.manifests
            .values()
            .filter(|m| {
                m.platform_support.host_os.is_empty()
                    || m.platform_support.host_os.iter().any(|os| os == current)
            })
            .map(|m| m.name.clone())
            .collect()
    }

    /// 获取当前平台不支持的工具名称列表（用于 CLI 显示"不可用"标签）
    pub fn unsupported_tool_names(&self) -> Vec<String> {
        let current = std::env::consts::OS;
        self.manifests
            .values()
            .filter(|m| {
                !m.platform_support.host_os.is_empty()
                    && !m.platform_support.host_os.iter().any(|os| os == current)
            })
            .map(|m| m.name.clone())
            .collect()
    }

    /// 获取工具的完整 Manifest
    pub fn get_manifest(&self, name: &str) -> Option<&Manifest> {
        self.manifests.get(name)
    }

    /// 获取已实例化的工具
    pub fn get_tool(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// 检查工具是否已注册（Manifest 级别）
    pub fn contains(&self, name: &str) -> bool {
        self.manifests.contains_key(name)
    }

    /// 注销工具 Manifest（插件热插拔：文件删除时调用）
    ///
    /// 同时注销已实例化的工具（如果存在）。
    pub fn unregister(&mut self, name: &str) -> bool {
        let manifest_removed = self.manifests.remove(name).is_some();
        let tool_removed = self.tools.remove(name).is_some();
        manifest_removed || tool_removed
    }

    /// 更新工具 Manifest（插件热插拔：文件修改时调用）
    ///
    /// 如果工具不存在，则注册；如果存在，则替换 Manifest。
    /// 已实例化的工具不受影响（执行体的热加载在后续阶段实现）。
    pub fn update_manifest(&mut self, manifest: Manifest) -> Result<(), RegistryError> {
        let name = manifest.name.clone();
        if self.manifests.contains_key(&name) {
            self.manifests.insert(name, manifest);
        } else {
            self.manifests.insert(name, manifest);
        }
        Ok(())
    }

    /// 已注册工具数量
    pub fn len(&self) -> usize {
        self.manifests.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }

    /// 插件目录路径
    pub fn plugin_dir(&self) -> Option<&PathBuf> {
        self.plugin_dir.as_ref()
    }

    // === T3+ 预留接口（T2 不实现） ===

    /// 扫描插件目录（T3 实现：读取 .manifest.json + 完整性校验）
    ///
    /// T2 阶段为占位，返回空 Ok。
    /// 扫描插件目录：读取 *.manifest.json → 解析 → 校验执行体 SHA-256 → 注册
    ///
    /// 渐进披露：只注册 Manifest（说明书），不加载执行体运行时。
    /// 完整性校验失败的条目跳过并记录在 ScanReport.skipped 中。
    pub fn scan_plugins(&mut self) -> Result<ScanReport, RegistryError> {
        let dir = match &self.plugin_dir {
            Some(d) => d.clone(),
            None => return Ok(ScanReport::default()),
        };
        if !dir.exists() {
            return Err(RegistryError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("plugin dir not found: {:?}", dir),
            )));
        }
        let mut report = ScanReport::default();
        for entry in walkdir::WalkDir::new(&dir).max_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    report.skipped.push(SkippedEntry {
                        manifest_path: dir.clone(),
                        reason: format!("walkdir error: {}", e),
                    });
                    continue;
                }
            };
            let path = entry.path();
            if !path.is_file() { continue; }
            let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !fname.ends_with(".manifest.json") { continue; }
            report.scanned += 1;

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    report.skipped.push(SkippedEntry {
                        manifest_path: path.to_path_buf(),
                        reason: format!("read error: {}", e),
                    });
                    continue;
                }
            };
            let manifest: Manifest = match serde_json::from_str(&content) {
                Ok(m) => m,
                Err(e) => {
                    report.skipped.push(SkippedEntry {
                        manifest_path: path.to_path_buf(),
                        reason: format!("invalid manifest JSON: {}", e),
                    });
                    continue;
                }
            };

            if let Err(e) = verify_file_integrity(&manifest, &dir) {
                report.skipped.push(SkippedEntry {
                    manifest_path: path.to_path_buf(),
                    reason: format!("integrity check failed: {}", e),
                });
                continue;
            }

            if self.manifests.contains_key(&manifest.name) {
                report.skipped.push(SkippedEntry {
                    manifest_path: path.to_path_buf(),
                    reason: format!("duplicate tool: {}", manifest.name),
                });
                continue;
            }
            self.manifests.insert(manifest.name.clone(), manifest);
            report.registered += 1;
        }
        Ok(report)
    }

    /// 重新扫描（清空已注册 Manifest 后重新扫描）
    pub fn rescan_plugins(&mut self) -> Result<ScanReport, RegistryError> {
        self.manifests.clear();
        self.tools.clear();
        self.scan_plugins()
    }

    /// 懒加载工具（P2 实现：根据 Manifest 加载 WASM/JS 运行时）
    ///
    /// T3 阶段为占位，返回 None（工具需预先 register_tool）。
    pub fn load_tool(&self, _name: &str) -> Option<Arc<dyn Tool>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::{ExecutionRequest, ToolOutput};

    struct MockTool {
        manifest: Manifest,
    }
    impl Tool for MockTool {
        fn name(&self) -> &str { &self.manifest.name }
        fn manifest(&self) -> &Manifest { &self.manifest }
        fn execute(&self, _req: ExecutionRequest) -> Result<ToolOutput, crate::error::ToolError> {
            Ok(ToolOutput::success(serde_json::json!({"mock":true})))
        }
    }

    #[test]
    fn register_and_index() {
        let mut reg = ToolRegistry::new();
        let m = Manifest { name: "t1".into(), version: "1.0".into(), description: "test".into(), ..Default::default() };
        reg.register_manifest(m).unwrap();
        assert_eq!(reg.len(), 1);
        assert!(reg.contains("t1"));
        let idx = reg.index();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].name, "t1");
    }

    #[test]
    fn duplicate_registration_rejected() {
        let mut reg = ToolRegistry::new();
        let m = Manifest { name: "t".into(), ..Default::default() };
        reg.register_manifest(m.clone()).unwrap();
        assert!(reg.register_manifest(m).is_err());
    }

    #[test]
    fn register_tool_syncs_manifest() {
        let mut reg = ToolRegistry::new();
        let m = Manifest { name: "mock".into(), ..Default::default() };
        let tool = Arc::new(MockTool { manifest: m });
        reg.register_tool(tool).unwrap();
        assert!(reg.contains("mock"));
        assert!(reg.get_tool("mock").is_some());
        assert!(reg.get_manifest("mock").is_some());
    }

    #[test]
    fn empty_registry() {
        let reg = ToolRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.index().is_empty());
    }

    #[test]
    fn with_plugin_dir() {
        let reg = ToolRegistry::with_plugin_dir("/tmp/plugins");
        assert_eq!(reg.plugin_dir().unwrap(), &PathBuf::from("/tmp/plugins"));
    }

    #[test]
    fn t2_placeholders_return_safely() {
        let mut reg = ToolRegistry::new();
        let report = reg.scan_plugins().unwrap(); // 无 plugin_dir → 空 report
        assert_eq!(report.scanned, 0);
        assert_eq!(report.registered, 0);
        assert!(reg.load_tool("nonexistent").is_none());
    }

    #[test]
    fn scan_plugins_registers_valid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        // 写执行体
        let exe_content = b"wasm-binary-content";
        std::fs::write(dir.path().join("scraper.wasm"), exe_content).unwrap();
        let hash = crate::manifest::compute_sha256(exe_content);
        // 写 manifest
        let manifest_json = serde_json::json!({
            "name": "scraper",
            "version": "1.0.0",
            "description": "test scraper",
            "executable": "scraper.wasm",
            "integrity": {"algorithm": "sha256", "hash": hash},
            "parameters_schema": {}
        });
        std::fs::write(
            dir.path().join("scraper.manifest.json"),
            serde_json::to_string_pretty(&manifest_json).unwrap(),
        ).unwrap();

        let mut reg = ToolRegistry::with_plugin_dir(dir.path());
        let report = reg.scan_plugins().unwrap();
        assert_eq!(report.scanned, 1);
        assert_eq!(report.registered, 1);
        assert!(report.skipped.is_empty());
        assert!(reg.contains("scraper"));
        let idx = reg.index();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].name, "scraper");
    }

    #[test]
    fn scan_plugins_skips_integrity_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bad.wasm"), b"real-content").unwrap();
        // 错误的 hash
        let manifest_json = serde_json::json!({
            "name": "bad",
            "version": "1.0.0",
            "description": "bad integrity",
            "executable": "bad.wasm",
            "integrity": {"algorithm": "sha256", "hash": "00000000000000000000000000000000000000000000000000000000000000"},
            "parameters_schema": {}
        });
        std::fs::write(
            dir.path().join("bad.manifest.json"),
            serde_json::to_string_pretty(&manifest_json).unwrap(),
        ).unwrap();

        let mut reg = ToolRegistry::with_plugin_dir(dir.path());
        let report = reg.scan_plugins().unwrap();
        assert_eq!(report.scanned, 1);
        assert_eq!(report.registered, 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(!reg.contains("bad"));
    }

    #[test]
    fn scan_plugins_ignores_non_manifest_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"hello").unwrap();
        std::fs::write(dir.path().join("scraper.wasm"), b"data").unwrap();
        let hash = crate::manifest::compute_sha256(b"data");
        let manifest_json = serde_json::json!({
            "name": "scraper", "version": "1", "description": "d",
            "executable": "scraper.wasm",
            "integrity": {"algorithm": "sha256", "hash": hash},
            "parameters_schema": {}
        });
        std::fs::write(dir.path().join("scraper.manifest.json"),
            serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let mut reg = ToolRegistry::with_plugin_dir(dir.path());
        let report = reg.scan_plugins().unwrap();
        assert_eq!(report.scanned, 1); // 只有 .manifest.json 被扫描
        assert_eq!(report.registered, 1);
    }

    #[test]
    fn rescan_clears_and_reregisters() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("t.wasm"), b"v1").unwrap();
        let hash = crate::manifest::compute_sha256(b"v1");
        let manifest_json = serde_json::json!({
            "name": "t", "version": "1", "description": "d",
            "executable": "t.wasm",
            "integrity": {"algorithm": "sha256", "hash": hash},
            "parameters_schema": {}
        });
        std::fs::write(dir.path().join("t.manifest.json"),
            serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let mut reg = ToolRegistry::with_plugin_dir(dir.path());
        reg.scan_plugins().unwrap();
        assert_eq!(reg.len(), 1);
        // rescan
        let report = reg.rescan_plugins().unwrap();
        assert_eq!(report.registered, 1);
        assert_eq!(reg.len(), 1);
    }
}
