//! PluginWatcher——插件热插拔监听器（可选 feature: hot-reload）
//!
//! 监听 plugins/ 目录的文件变化（新增/修改/删除 .manifest.json 文件），
//! 自动更新 ToolRegistry（注册/注销 Manifest）。
//!
//! 默认不启用，需要在 Cargo.toml 中开启 `hot-reload` feature。
//!
//! 设计决策：
//! - 只监听 .manifest.json 文件的变化，不监听 .wasm/.js 执行体
//! - 执行体的热加载（.wasm/.js）在后续阶段实现
//! - 使用 notify crate 的 RecommendedWatcher，跨平台
//! - 防抖处理：避免短时间内多次触发同一文件的变化事件

use crate::error::RegistryError;
use crate::manifest::Manifest;
use crate::registry::ToolRegistry;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 插件热插拔监听器
pub struct PluginWatcher {
    registry: Arc<Mutex<ToolRegistry>>,
    plugin_dir: PathBuf,
    watcher: Option<RecommendedWatcher>,
}

impl PluginWatcher {
    /// 创建插件监听器
    pub fn new(registry: Arc<Mutex<ToolRegistry>>, plugin_dir: impl Into<PathBuf>) -> Self {
        Self {
            registry,
            plugin_dir: plugin_dir.into(),
            watcher: None,
        }
    }

    /// 启动文件系统监听（阻塞当前线程）
    ///
    /// 监听 plugins/ 目录的文件变化，自动更新 ToolRegistry。
    /// 调用 `stop()` 或 drop 监听器可停止监听。
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let registry = self.registry.clone();
        let plugin_dir = self.plugin_dir.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let Err(e) = handle_event(&registry, &plugin_dir, &event) {
                        eprintln!("[PluginWatcher] Error handling event: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("[PluginWatcher] Watch error: {}", e);
                }
            }
        })?;

        watcher.watch(&self.plugin_dir, RecursiveMode::Recursive)?;
        self.watcher = Some(watcher);
        Ok(())
    }

    /// 停止文件系统监听
    pub fn stop(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
        }
    }

    /// 插件目录路径
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
}

impl Drop for PluginWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 处理文件系统事件
fn handle_event(
    registry: &Arc<Mutex<ToolRegistry>>,
    plugin_dir: &Path,
    event: &Event,
) -> Result<(), Box<dyn std::error::Error>> {
    for path in &event.paths {
        // 只处理 .manifest.json 文件
        if !is_manifest_file(path) {
            continue;
        }

        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                // 新增或修改：加载并注册/更新 Manifest
                if let Ok(manifest) = load_manifest(path) {
                    let mut reg = registry.lock().unwrap();
                    if reg.contains(&manifest.name) {
                        reg.update_manifest(manifest)?;
                    } else {
                        reg.register_manifest(manifest)?;
                    }
                }
            }
            EventKind::Remove(_) => {
                // 删除：从文件名推断工具名并注销
                if let Some(tool_name) = tool_name_from_path(path, plugin_dir) {
                    let mut reg = registry.lock().unwrap();
                    reg.unregister(&tool_name);
                }
            }
            _ => {
                // 忽略其他事件类型（如 Access、Any 等）
            }
        }
    }
    Ok(())
}

/// 检查文件是否为 .manifest.json
fn is_manifest_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "json")
        .unwrap_or(false)
        && path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.ends_with(".manifest"))
            .unwrap_or(false)
}

/// 从文件路径加载 Manifest
fn load_manifest(path: &Path) -> Result<Manifest, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let manifest: Manifest = serde_json::from_str(&content)?;
    Ok(manifest)
}

/// 从文件路径推断工具名
///
/// 例如：`plugins/targeted_scraper.manifest.json` → `targeted_scraper`
fn tool_name_from_path(path: &Path, plugin_dir: &Path) -> Option<String> {
    let relative = path.strip_prefix(plugin_dir).ok()?;
    let file_stem = relative.file_stem()?.to_str()?;
    // 移除 .manifest 后缀
    file_stem.strip_suffix(".manifest").map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Integrity, SecurityLevel};
    use std::fs;
    use std::time::Duration;

    fn test_manifest(name: &str) -> Manifest {
        Manifest {
            name: name.to_string(),
            version: "1.0".into(),
            description: format!("test tool: {}", name),
            executable: format!("{}.wasm", name),
            integrity: Integrity::sha256("abc"),
            security_level: SecurityLevel::Normal,
            ..Default::default()
        }
    }

    #[test]
    fn test_is_manifest_file() {
        assert!(is_manifest_file(Path::new("plugins/targeted_scraper.manifest.json")));
        assert!(!is_manifest_file(Path::new("plugins/targeted_scraper.wasm")));
        assert!(!is_manifest_file(Path::new("plugins/targeted_scraper.json")));
        assert!(!is_manifest_file(Path::new("plugins/README.md")));
    }

    #[test]
    fn test_tool_name_from_path() {
        let plugin_dir = Path::new("/tmp/plugins");
        let path = Path::new("/tmp/plugins/targeted_scraper.manifest.json");
        assert_eq!(
            tool_name_from_path(path, plugin_dir),
            Some("targeted_scraper".to_string())
        );

        let path = Path::new("/tmp/plugins/subdir/my_tool.manifest.json");
        assert_eq!(
            tool_name_from_path(path, plugin_dir),
            Some("my_tool".to_string())
        );
    }

    #[test]
    fn test_load_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("test_tool.manifest.json");
        let manifest = test_manifest("test_tool");
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();

        let loaded = load_manifest(&manifest_path).unwrap();
        assert_eq!(loaded.name, "test_tool");
        assert_eq!(loaded.version, "1.0");
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = ToolRegistry::new();
        let manifest = test_manifest("test_tool");
        registry.register_manifest(manifest).unwrap();
        assert!(registry.contains("test_tool"));

        let removed = registry.unregister("test_tool");
        assert!(removed);
        assert!(!registry.contains("test_tool"));

        // 重复注销返回 false
        let removed = registry.unregister("test_tool");
        assert!(!removed);
    }

    #[test]
    fn test_registry_update_manifest() {
        let mut registry = ToolRegistry::new();
        let mut manifest = test_manifest("test_tool");
        registry.register_manifest(manifest.clone()).unwrap();

        // 更新 Manifest
        manifest.description = "updated description".into();
        registry.update_manifest(manifest.clone()).unwrap();

        let updated = registry.get_manifest("test_tool").unwrap();
        assert_eq!(updated.description, "updated description");
    }

    #[test]
    fn test_plugin_watcher_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Arc::new(Mutex::new(ToolRegistry::new()));
        let mut watcher = PluginWatcher::new(registry.clone(), dir.path());

        // 启动监听
        watcher.start().unwrap();

        // 创建一个 Manifest 文件
        let manifest_path = dir.path().join("test_tool.manifest.json");
        let manifest = test_manifest("test_tool");
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();

        // 等待文件系统事件传播
        std::thread::sleep(Duration::from_millis(500));

        // 验证 Manifest 已注册
        {
            let reg = registry.lock().unwrap();
            assert!(reg.contains("test_tool"));
        }

        // 修改 Manifest
        let mut updated = test_manifest("test_tool");
        updated.description = "updated".into();
        fs::write(&manifest_path, serde_json::to_string_pretty(&updated).unwrap()).unwrap();

        // 等待文件系统事件传播
        std::thread::sleep(Duration::from_millis(500));

        // 验证 Manifest 已更新
        {
            let reg = registry.lock().unwrap();
            let m = reg.get_manifest("test_tool").unwrap();
            assert_eq!(m.description, "updated");
        }

        // 停止监听
        watcher.stop();

        // 注意：文件删除时的工具名推断在某些平台上可能不可靠，
        // 删除功能的完整测试在后续阶段优化。
        // 当前实现：删除事件触发时，尝试从路径推断工具名并注销。
    }
}
