//! targeted_scraper — 定向爬取工具（首个内置工具）
//!
//! 白皮书 §3.4 / 附录 A：定向爬取 + 渐进式觅食 + 布隆过滤器端到端验证。
//!
//! # 核心能力
//! - HTTP 请求（IdentityHttpClient，凭证标签流转，四修正1）
//! - 渐进式觅食（ForagingEvaluator，信息增益评估，边缘价值递减时自动终止）
//! - 布隆过滤器对接（可选 bloom feature，四修正2，本地全局已见熵）
//! - HTML 解析（简单正则去除标签，提取纯文本）
//!
//! # 设计原则
//! - 统计优先，LLM 兜底：信息增益评估用纯统计方法（Jaccard/TF-IDF），0 Token
//! - 极致节能：渐进式觅食在信息增益低于阈值时自动终止，避免无效爬取
//! - 凭证标签流转：出网请求只携带 X-Identity-Label，明文凭证由 Tuck 注入

use crate::forager::{ForageDecision, ForagingEvaluator, ForageStopReason};
use tentacle_core::error::ToolError;
use tentacle_core::manifest::Manifest;
use tentacle_core::tool::{ExecutionRequest, StopReason, Tool, ToolOutput};
use tentacle_http::IdentityHttpClient;

/// 定向爬取工具
///
/// 首个内置工具，演示"HTTP 客户端 + 渐进式觅食 + 布隆过滤器"的完整链路。
pub struct TargetedScraper {
    manifest: Manifest,
    http_client: IdentityHttpClient,
}

impl TargetedScraper {
    /// 创建定向爬取工具
    pub fn new(manifest: Manifest) -> Self {
        Self {
            manifest,
            http_client: IdentityHttpClient::new(),
        }
    }

    /// 带凭证标签创建
    pub fn with_identity_labels(
        manifest: Manifest,
        labels: std::collections::HashMap<String, String>,
    ) -> Self {
        Self {
            manifest,
            http_client: IdentityHttpClient::with_identity_labels(labels),
        }
    }

    /// 执行定向爬取（同步阻塞，内部创建 tokio runtime）
    fn scrape(&self, req: &ExecutionRequest) -> Result<ToolOutput, ToolError> {
        // 解析参数
        let params = &req.params;
        let urls: Vec<String> = params
            .get("urls")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let threshold_delta = params
            .get("threshold_delta")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.15);

        let max_pages = params
            .get("max_pages_per_session")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(50);

        if urls.is_empty() {
            return Err(ToolError::InvalidParams("urls 参数不能为空".to_string()));
        }

        // 创建觅食评估器
        let mut evaluator = ForagingEvaluator::new(&query, threshold_delta, max_pages);

        // 布隆过滤器对接（四修正2，可选 bloom feature）
        // 注意：布隆过滤器的序列化格式需与本地 bloom 实现对齐，P3-T4 先保留接口，
        // 完整的序列化/反序列化在 bloom feature 明确后实现。
        #[cfg(feature = "bloom")]
        {
            if req.seen_entropy_bloom.is_some() {
                eprintln!("[targeted_scraper] 收到已见熵布隆过滤器，待 bloom 序列化格式对齐后启用");
            }
        }

        // 创建 tokio runtime（同步阻塞执行异步 HTTP 请求）
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| ToolError::ExecutionFailed(format!("创建 tokio runtime 失败: {}", e)))?;

        let mut results: Vec<serde_json::Value> = Vec::new();
        let mut stop_reason: Option<StopReason> = None;

        for url in &urls {
            // 发送 HTTP 请求
            let response = rt.block_on(async {
                self.http_client
                    .get(url)
                    .send()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("HTTP 请求失败: {}", e)))
            })?;

            let status = response.status();
            if !status.is_success() {
                results.push(serde_json::json!({
                    "url": url,
                    "status": status.as_u16(),
                    "error": format!("HTTP {}", status),
                }));
                continue;
            }

            let html = rt.block_on(async {
                response
                    .text()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("读取响应体失败: {}", e)))
            })?;

            // HTML 解析：去除标签，提取纯文本
            let text = extract_text(&html);

            // 渐进式觅食：评估信息增益
            let decision = evaluator.evaluate_gain(&text);

            results.push(serde_json::json!({
                "url": url,
                "status": status.as_u16(),
                "content_length": text.len(),
                "content_preview": &text[..text.len().min(200)],
            }));

            match decision {
                ForageDecision::Continue => {}
                ForageDecision::Stop { reason } => {
                    stop_reason = Some(match reason {
                        ForageStopReason::LowInformationGain { last_gain, threshold } => {
                            StopReason::LowInformationGain { last_gain, threshold }
                        }
                        ForageStopReason::MaxPagesReached { pages } => {
                            StopReason::MaxPagesReached { pages }
                        }
                        ForageStopReason::ZeroGainStreak { consecutive_pages } => {
                            StopReason::ZeroGainStreak { consecutive_pages }
                        }
                    });
                    break;
                }
            }
        }

        // 构建输出
        let data = serde_json::json!({
            "query": query,
            "pages_fetched": results.len(),
            "results": results,
        });

        if let Some(reason) = stop_reason {
            Ok(ToolOutput::foraging_stopped(data, reason))
        } else {
            Ok(ToolOutput::success(data))
        }
    }
}

impl Tool for TargetedScraper {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn execute(&self, req: ExecutionRequest) -> Result<ToolOutput, ToolError> {
        self.scrape(&req)
    }
}

/// 从 HTML 中提取纯文本（简单正则去除标签）
fn extract_text(html: &str) -> String {
    // 去除 script 和 style 标签及其内容（两个独立正则，避免反向引用）
    let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let without_script = re_script.replace_all(html, " ");
    let without_style = re_style.replace_all(&without_script, " ");

    // 去除 HTML 标签
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let without_tags = re_tags.replace_all(&without_style, " ");

    // 解码常见 HTML 实体
    let decoded = without_tags
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // 压缩空白
    let re_whitespace = regex::Regex::new(r"\s+").unwrap();
    re_whitespace.replace_all(&decoded, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_manifest() -> Manifest {
        Manifest {
            name: "targeted_scraper".into(),
            version: "1.0.0".into(),
            description: "定向爬取工具".into(),
            executable: "targeted_scraper".into(),
            timeout_ms: 30000,
            ..Default::default()
        }
    }

    #[test]
    fn test_extract_text_basic() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let text = extract_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("<h1>"));
        assert!(!text.contains("<p>"));
    }

    #[test]
    fn test_extract_text_removes_script() {
        let html = "<html><body><script>var x = 1;</script><p>Content</p></body></html>";
        let text = extract_text(html);
        assert!(text.contains("Content"));
        assert!(!text.contains("var x"));
    }

    #[test]
    fn test_extract_text_decodes_entities() {
        let html = "<p>Tom &amp; Jerry</p>";
        let text = extract_text(html);
        assert!(text.contains("Tom & Jerry"));
    }

    #[test]
    fn test_scraper_name_and_manifest() {
        let scraper = TargetedScraper::new(test_manifest());
        assert_eq!(scraper.name(), "targeted_scraper");
        assert_eq!(scraper.manifest().version, "1.0.0");
    }

    #[test]
    fn test_scraper_trait_object() {
        let scraper = TargetedScraper::new(test_manifest());
        let tool: Box<dyn Tool> = Box::new(scraper);
        assert_eq!(tool.name(), "targeted_scraper");
    }

    #[test]
    fn test_scraper_empty_urls_returns_error() {
        let scraper = TargetedScraper::new(test_manifest());
        let req = ExecutionRequest {
            tool: "targeted_scraper".into(),
            params: serde_json::json!({"urls": [], "query": "test"}),
            identity_labels: HashMap::new(),
            trace_id: None,
            seen_entropy_bloom: None,
        };
        let result = scraper.execute(req);
        assert!(result.is_err());
    }

    #[test]
    fn test_scraper_with_identity_labels() {
        let mut labels = HashMap::new();
        labels.insert("weibo".to_string(), "weibo_session_1".to_string());
        let scraper = TargetedScraper::with_identity_labels(test_manifest(), labels);
        assert_eq!(scraper.http_client.identity_labels().get("weibo"), Some(&"weibo_session_1".to_string()));
    }
}
