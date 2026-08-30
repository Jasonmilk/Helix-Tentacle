//! tentacle-core — Helix-Tentacle 核心
//!
//! 纯逻辑层，无网络依赖。定义 Tool trait、Manifest、ToolRegistry、
//! 安全等级、脱敏管道、ConsensusHook。
//!
//! 四修正硬性验收：
//! - 修正1：ExecutionRequest 用 identity_labels，无明文 credentials
//! - 修正4：ConsensusHook trait（Standalone/Helix/Mcp 动态适配）

pub mod manifest;
pub mod tool;
pub mod registry;
pub mod redact;
pub mod consensus;
pub mod error;
pub mod resource;

/// 插件热插拔监听器（可选 feature: hot-reload）
#[cfg(feature = "hot-reload")]
pub mod plugin_watcher;

pub use manifest::{
    Manifest, ManifestIndex, Integrity, ForagingConfig, Permission, SecurityLevel,
    RequiresIdentity,
};
pub use tool::{Tool, ToolOutput, OutputChunk, ExecutionRequest, StopReason};
pub use registry::ToolRegistry;
pub use redact::Redactor;
pub use consensus::{ConsensusHook, ConsensusMode, Approval, ConsensusError};
pub use error::{ToolError, RegistryError};
pub use resource::{
    ResourceQuota, ResourceUsage, ResourceLimiter, ResourceLimitError, AtomicResourceLimiter,
};
