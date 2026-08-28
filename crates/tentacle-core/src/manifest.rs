//! 工具说明书（Manifest）——声明与执行分离，SHA-256 完整性绑定

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// 安全等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecurityLevel {
    /// 普通操作（读为主，低风险）
    Normal,
    /// 关键操作（写/网络/凭证使用，需共识确认）
    Critical,
}

impl Default for SecurityLevel {
    fn default() -> Self {
        SecurityLevel::Normal
    }
}

/// 完整性校验（声明与执行体的密码学绑定）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Integrity {
    #[serde(default)]
    pub algorithm: String,
    #[serde(default)]
    pub hash: String,
}

impl Integrity {
    pub fn sha256(hash: impl Into<String>) -> Self {
        Self {
            algorithm: "sha256".into(),
            hash: hash.into(),
        }
    }
}

/// 权限声明
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Permission {
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub filesystem: Vec<String>,
    #[serde(default)]
    pub execute: bool,
    #[serde(default)]
    pub max_memory_mb: Option<u32>,
    #[serde(default)]
    pub max_cpu_time_ms: Option<u32>,
}

/// 身份需求声明
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequiresIdentity {
    /// 身份类型（如 "cookie"、"token"）
    #[serde(default)]
    pub identity_type: Option<String>,
    /// 目标域（如 "weibo.com"）
    #[serde(default)]
    pub domain: Option<String>,
    /// 敏感字段列表（用于脱敏）
    #[serde(default)]
    pub sensitive_fields: Vec<String>,
}

/// 渐进式觅食配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForagingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 信息增益阈值（低于此值终止）
    #[serde(default = "default_threshold")]
    pub threshold_delta: f64,
    /// 单次会话最大页面数
    #[serde(default = "default_max_pages")]
    pub max_pages_per_session: u32,
}

impl Default for ForagingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_delta: 0.15,
            max_pages_per_session: 50,
        }
    }
}

fn default_true() -> bool { true }
fn default_threshold() -> f64 { 0.15 }
fn default_max_pages() -> u32 { 50 }

/// 工具说明书（Manifest）——静态声明文件
///
/// 与执行体（.wasm/.js）物理分离，通过 integrity 字段 SHA-256 绑定。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// 执行体文件名（如 "targeted_scraper.wasm"）
    pub executable: String,
    /// 完整性校验（SHA-256）
    pub integrity: Integrity,
    /// 参数 Schema（JSON Schema）
    #[serde(default)]
    pub parameters_schema: serde_json::Value,
    #[serde(default)]
    pub examples: Vec<serde_json::Value>,
    #[serde(default)]
    pub trigger_phrases: Vec<String>,
    #[serde(default)]
    pub security_level: SecurityLevel,
    #[serde(default)]
    pub permissions: Permission,
    #[serde(default)]
    pub requires_identity: Option<RequiresIdentity>,
    /// 是否需要外部签名（宿主代签）
    #[serde(default)]
    pub requires_external_signer: bool,
    /// 允许签名的目标域（requires_external_signer 为 true 时生效）
    #[serde(default)]
    pub allowed_signing_domains: Vec<String>,
    /// 代签频率限制（每分钟）
    #[serde(default)]
    pub rate_limit_per_minute: Option<u32>,
    /// 执行超时（毫秒）
    #[serde(default = "default_timeout")]
    pub timeout_ms: u32,
    /// 渐进式觅食配置
    #[serde(default)]
    pub foraging_config: ForagingConfig,
}

fn default_timeout() -> u32 { 30000 }

/// 轻量说明书索引（渐进披露：只暴露名称和一句话描述）
#[derive(Debug, Clone, Serialize)]
pub struct ManifestIndex {
    pub name: String,
    pub description: String,
    pub version: String,
    pub security_level: SecurityLevel,
}

impl From<&Manifest> for ManifestIndex {
    fn from(m: &Manifest) -> Self {
        Self {
            name: m.name.clone(),
            description: m.description.clone(),
            version: m.version.clone(),
            security_level: m.security_level,
        }
    }
}

/// 从 JSON 字符串解析 Manifest
pub fn parse_manifest(json: &str) -> Result<Manifest, serde_json::Error> {
    serde_json::from_str(json)
}

/// 计算执行体的 SHA-256（纯 Rust 实现，无外部依赖）
///
/// 注意：core 层不引入 sha2 crate 以保持 <500KB。
/// 实际 SHA-256 计算由 tentacle-tools 或传输层注入。
/// 此处提供占位接口，实际实现通过依赖注入。
pub fn verify_integrity(
    manifest: &Manifest,
    actual_hash: &str,
) -> Result<(), crate::error::RegistryError> {
    if manifest.integrity.hash != actual_hash {
        return Err(crate::error::RegistryError::IntegrityMismatch {
            tool: manifest.name.clone(),
            expected: manifest.integrity.hash.clone(),
            actual: actual_hash.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip() {
        let json = r#"{
            "name": "targeted_scraper",
            "version": "1.0.0",
            "description": "定向爬取",
            "executable": "targeted_scraper.wasm",
            "integrity": {"algorithm": "sha256", "hash": "abc123"},
            "parameters_schema": {},
            "security_level": "normal",
            "foraging_config": {"enabled": true, "threshold_delta": 0.15, "max_pages_per_session": 50}
        }"#;
        let m: Manifest = parse_manifest(json).unwrap();
        assert_eq!(m.name, "targeted_scraper");
        assert_eq!(m.security_level, SecurityLevel::Normal);
        assert!(m.foraging_config.enabled);
        assert_eq!(m.foraging_config.threshold_delta, 0.15);
    }

    #[test]
    fn manifest_index_progressive_disclosure() {
        let json = r#"{"name":"t","version":"1","description":"desc","executable":"t.wasm","integrity":{"algorithm":"sha256","hash":"h"},"parameters_schema":{}}"#;
        let m: Manifest = parse_manifest(json).unwrap();
        let idx = ManifestIndex::from(&m);
        assert_eq!(idx.name, "t");
        assert_eq!(idx.description, "desc");
        // 索引不暴露 parameters_schema/integrity 等细节
    }

    #[test]
    fn integrity_verify_pass() {
        let m = Manifest {
            name: "t".into(), version: "1".into(), description: "".into(),
            executable: "t.wasm".into(), integrity: Integrity::sha256("abc"),
            parameters_schema: serde_json::json!({}), ..Default::default()
        };
        assert!(verify_integrity(&m, "abc").is_ok());
        assert!(verify_integrity(&m, "xyz").is_err());
    }

    #[test]
    fn compute_sha256_known_value() {
        // SHA-256 of "hello"
        let hash = compute_sha256(b"hello");
        assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn compute_file_sha256_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello").unwrap();
        let hash = compute_file_sha256(&path).unwrap();
        assert_eq!(hash, compute_sha256(b"hello"));
    }
}
