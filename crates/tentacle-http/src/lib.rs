//! tentacle-http — HTTP 客户端（凭证标签流转，四修正1）
//!
//! 出网请求强制添加 X-Identity-Label 头，不注入明文凭证。
//! 真实凭证由 Tuck（127.0.0.1:9000）在物理边缘识别标签并注入。

use reqwest::{Client, RequestBuilder};
use std::collections::HashMap;

/// X-Identity-Label 头名
pub const IDENTITY_LABEL_HEADER: &str = "X-Identity-Label";

/// 带凭证标签的 HTTP 客户端
#[derive(Clone)]
pub struct IdentityHttpClient {
    client: Client,
    identity_labels: HashMap<String, String>,
}

impl IdentityHttpClient {
    pub fn new() -> Self {
        Self { client: Client::new(), identity_labels: HashMap::new() }
    }

    pub fn with_identity_labels(labels: HashMap<String, String>) -> Self {
        Self { client: Client::new(), identity_labels: labels }
    }

    pub fn add_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.identity_labels.insert(key.into(), value.into());
        self
    }

    pub fn identity_labels(&self) -> &HashMap<String, String> {
        &self.identity_labels
    }

    pub fn get(&self, url: &str) -> RequestBuilder {
        self.apply_labels(self.client.get(url))
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        self.apply_labels(self.client.post(url))
    }

    pub fn put(&self, url: &str) -> RequestBuilder {
        self.apply_labels(self.client.put(url))
    }

    pub fn delete(&self, url: &str) -> RequestBuilder {
        self.apply_labels(self.client.delete(url))
    }

    pub fn patch(&self, url: &str) -> RequestBuilder {
        self.apply_labels(self.client.patch(url))
    }

    fn apply_labels(&self, builder: RequestBuilder) -> RequestBuilder {
        if self.identity_labels.is_empty() { return builder; }
        let header_value = self.identity_labels.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>().join(", ");
        builder.header(IDENTITY_LABEL_HEADER, header_value)
    }

    pub fn inner(&self) -> &Client { &self.client }
}

impl Default for IdentityHttpClient {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client_no_labels() {
        let client = IdentityHttpClient::new();
        assert!(client.identity_labels().is_empty());
    }

    #[test]
    fn test_with_identity_labels() {
        let mut labels = HashMap::new();
        labels.insert("weibo".to_string(), "session_1".to_string());
        let client = IdentityHttpClient::with_identity_labels(labels);
        assert_eq!(client.identity_labels().get("weibo"), Some(&"session_1".to_string()));
    }

    #[test]
    fn test_add_label() {
        let client = IdentityHttpClient::new().add_label("weibo", "session_1");
        assert_eq!(client.identity_labels().len(), 1);
    }

    #[test]
    fn test_get_request_includes_identity_label_header() {
        let client = IdentityHttpClient::new().add_label("weibo", "session_1");
        let req = client.get("http://example.com/test").build().unwrap();
        let header = req.headers().get("X-Identity-Label").unwrap();
        assert_eq!(header.to_str().unwrap(), "weibo=session_1");
    }

    #[test]
    fn test_no_labels_no_header() {
        let client = IdentityHttpClient::new();
        let req = client.get("http://example.com/test").build().unwrap();
        assert!(req.headers().get("X-Identity-Label").is_none());
    }

    #[test]
    fn test_multiple_labels_joined() {
        let client = IdentityHttpClient::new()
            .add_label("weibo", "session_1")
            .add_label("github", "token_2");
        let req = client.get("http://example.com/test").build().unwrap();
        let header = req.headers().get("X-Identity-Label").unwrap().to_str().unwrap();
        assert!(header.contains("weibo=session_1"));
        assert!(header.contains("github=token_2"));
    }
}
