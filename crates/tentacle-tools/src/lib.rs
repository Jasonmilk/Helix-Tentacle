//! tentacle-tools — 内置工具集 + 插件加载器
//!
//! P2-T1: forager.rs（渐进式觅食评估器）
//! P2-T2: 已见熵布隆过滤器（修正2）
//! P3-T3: wasm_tool.rs + js_tool.rs + loader.rs（执行层对接）
//! P3-T4: targeted_scraper（首个内置工具）

pub mod forager;

#[cfg(feature = "wasm")]
pub mod wasm_tool;
#[cfg(feature = "js")]
pub mod js_tool;
pub mod loader;

#[cfg(feature = "wasm")]
pub use wasm_tool::WasmTool;
#[cfg(feature = "js")]
pub use js_tool::JsTool;
pub use loader::PluginLoader;
