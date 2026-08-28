//! 脱敏管道——Schema 感知 + 通用敏感关键词，默认开启不可绕过

use serde_json::Value;
use std::collections::HashSet;

/// 通用敏感关键词（自动掩码）
const DEFAULT_SENSITIVE_KEYS: &[&str] = &[
    "cookie",
    "token",
    "api_key",
    "apikey",
    "authorization",
    "private_key",
    "secret",
    "password",
    "passwd",
];

/// 脱敏器
///
/// 根据 Manifest 声明的 sensitive_fields + 通用敏感关键词，
/// 对 JSON 值进行递归掩码。脱敏是自动的、默认开启的、不可绕过的。
#[derive(Debug, Clone)]
pub struct Redactor {
    /// Manifest 声明的额外敏感字段
    sensitive_fields: HashSet<String>,
    /// 掩码替换值
    mask: &'static str,
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Redactor {
    /// 创建脱敏器（额外敏感字段来自 Manifest.requires_identity.sensitive_fields）
    pub fn new(extra_fields: Vec<String>) -> Self {
        let mut sensitive_fields = HashSet::new();
        for k in DEFAULT_SENSITIVE_KEYS {
            sensitive_fields.insert(k.to_string());
        }
        for f in extra_fields {
            sensitive_fields.insert(f.to_lowercase());
        }
        Self {
            sensitive_fields,
            mask: "[REDACTED]",
        }
    }

    /// 判断字段名是否敏感
    fn is_sensitive(&self, key: &str) -> bool {
        self.sensitive_fields.contains(&key.to_lowercase())
    }

    /// 递归脱敏 JSON 值（原地修改）
    pub fn redact(&self, value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (k, v) in map.iter_mut() {
                    if self.is_sensitive(k) {
                        *v = Value::String(self.mask.to_string());
                    } else {
                        self.redact(v);
                    }
                }
            }
            Value::Array(arr) => {
                for v in arr.iter_mut() {
                    self.redact(v);
                }
            }
            _ => {}
        }
    }

    /// 脱敏并返回新值（不修改原数据）
    pub fn redact_clone(&self, value: &Value) -> Value {
        let mut cloned = value.clone();
        self.redact(&mut cloned);
        cloned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_default_sensitive_keys() {
        let r = Redactor::default();
        let mut v = serde_json::json!({
            "cookie": "abc123",
            "token": "xyz",
            "normal": "keep",
            "nested": {"api_key": "secret", "data": 1}
        });
        r.redact(&mut v);
        assert_eq!(v["cookie"], "[REDACTED]");
        assert_eq!(v["token"], "[REDACTED]");
        assert_eq!(v["normal"], "keep");
        assert_eq!(v["nested"]["api_key"], "[REDACTED]");
        assert_eq!(v["nested"]["data"], 1);
    }

    #[test]
    fn redacts_extra_fields_from_manifest() {
        let r = Redactor::new(vec!["weibo_session".into()]);
        let mut v = serde_json::json!({"weibo_session": "s1", "other": "ok"});
        r.redact(&mut v);
        assert_eq!(v["weibo_session"], "[REDACTED]");
        assert_eq!(v["other"], "ok");
    }

    #[test]
    fn redact_clone_does_not_modify_original() {
        let r = Redactor::default();
        let original = serde_json::json!({"token": "abc"});
        let redacted = r.redact_clone(&original);
        assert_eq!(original["token"], "abc"); // 原数据不变
        assert_eq!(redacted["token"], "[REDACTED]");
    }

    #[test]
    fn case_insensitive_key_matching() {
        let r = Redactor::default();
        let mut v = serde_json::json!({"Cookie": "abc", "API_KEY": "xyz"});
        r.redact(&mut v);
        assert_eq!(v["Cookie"], "[REDACTED]");
        assert_eq!(v["API_KEY"], "[REDACTED]");
    }
}
