//! 动态共识适配层——四修正4

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsensusMode {
    Standalone,
    Helix,
    Mcp,
}

impl Default for ConsensusMode {
    fn default() -> Self { ConsensusMode::Standalone }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub approved: bool,
    pub approver: String,
    pub timestamp: u64,
    #[serde(default)]
    pub reason: Option<String>,
}

impl Approval {
    pub fn approve(approver: impl Into<String>) -> Self {
        Self { approved: true, approver: approver.into(), timestamp: 0, reason: None }
    }
    pub fn deny(approver: impl Into<String>, reason: impl Into<String>) -> Self {
        Self { approved: false, approver: approver.into(), timestamp: 0, reason: Some(reason.into()) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConsensusError {
    Unavailable { message: String },
    Denied { reason: String },
    Timeout,
    UnsupportedMode { mode: String },
}

impl std::fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsensusError::Unavailable { message } => write!(f, "consensus unavailable: {}", message),
            ConsensusError::Denied { reason } => write!(f, "consensus denied: {}", reason),
            ConsensusError::Timeout => write!(f, "consensus timeout"),
            ConsensusError::UnsupportedMode { mode } => write!(f, "unsupported mode: {}", mode),
        }
    }
}
impl std::error::Error for ConsensusError {}

/// 共识钩子 trait
pub trait ConsensusHook: Send + Sync {
    fn mode(&self) -> ConsensusMode;
    fn request_approval(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> Result<Approval, ConsensusError>;
}

pub struct NoopConsensus;
impl ConsensusHook for NoopConsensus {
    fn mode(&self) -> ConsensusMode { ConsensusMode::Standalone }
    fn request_approval(&self, _tool: &str, _params: &serde_json::Value) -> Result<Approval, ConsensusError> {
        Ok(Approval::approve("noop"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_standalone() {
        assert_eq!(ConsensusMode::default(), ConsensusMode::Standalone);
    }

    #[test]
    fn approval_approve_and_deny() {
        assert!(Approval::approve("u").approved);
        assert!(!Approval::deny("u", "r").approved);
    }

    #[test]
    fn noop_always_approves() {
        let hook = NoopConsensus;
        assert!(hook.request_approval("t", &serde_json::json!({})).unwrap().approved);
    }
}
