//! 错误类型

use std::fmt;

/// 工具执行错误
#[derive(Debug)]
pub enum ToolError {
    NotFound(String),
    PermissionDenied(String),
    IdentityRequired(String),
    Timeout,
    ExecutionFailed(String),
    StreamNotSupported,
    ConsensusDenied(String),
    InvalidParams(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::NotFound(n) => write!(f, "tool not found: {}", n),
            ToolError::PermissionDenied(m) => write!(f, "permission denied: {}", m),
            ToolError::IdentityRequired(m) => write!(f, "identity required: {}", m),
            ToolError::Timeout => write!(f, "execution timeout"),
            ToolError::ExecutionFailed(m) => write!(f, "execution failed: {}", m),
            ToolError::StreamNotSupported => write!(f, "stream not supported"),
            ToolError::ConsensusDenied(m) => write!(f, "consensus denied: {}", m),
            ToolError::InvalidParams(m) => write!(f, "invalid params: {}", m),
        }
    }
}

impl std::error::Error for ToolError {}

/// 注册器错误
#[derive(Debug)]
pub enum RegistryError {
    InvalidManifest(String),
    IntegrityMismatch { tool: String, expected: String, actual: String },
    Io(std::io::Error),
    Duplicate(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::InvalidManifest(m) => write!(f, "invalid manifest: {}", m),
            RegistryError::IntegrityMismatch { tool, expected, actual } =>
                write!(f, "integrity mismatch for {}: expected {}, got {}", tool, expected, actual),
            RegistryError::Io(e) => write!(f, "io error: {}", e),
            RegistryError::Duplicate(n) => write!(f, "duplicate registration: {}", n),
        }
    }
}

impl std::error::Error for RegistryError {}

impl From<std::io::Error> for RegistryError {
    fn from(e: std::io::Error) -> Self { RegistryError::Io(e) }
}
