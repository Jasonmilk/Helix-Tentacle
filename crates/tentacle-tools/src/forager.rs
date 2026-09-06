//! 渐进式觅食评估器（Foraging Evaluator）
//!
//! 白皮书 §3.4 / T07：网络采集工具内建信息增益评估器，
//! 基于本地统计算法（Jaccard/TF-IDF）动态评估每一页的信息增益。
//! 边缘价值递减时自动终止，避免无效爬取和平台防御触发。
//!
//! 核心原则：统计优先，LLM 兜底。能用统计方法解决的，绝不调用 LLM。
//! T2 已实现已见熵布隆过滤器（可选 `bloom` feature，四修正2）。
//! 未启用 feature 时，seen_entropy 仅在本次会话内维护，零开销。

use std::collections::{HashMap, HashSet};

#[cfg(feature = "bloom")]
use bloomfilter::Bloom;

/// 觅食决策
#[derive(Debug, Clone, PartialEq)]
pub enum ForageDecision {
    /// 继续抓取
    Continue,
    /// 停止（附原因）
    Stop { reason: ForageStopReason },
}

/// 停止原因
#[derive(Debug, Clone, PartialEq)]
pub enum ForageStopReason {
    /// 信息增益低于阈值
    LowInformationGain { last_gain: f64, threshold: f64 },
    /// 达到最大页面数上限
    MaxPagesReached { pages: u32 },
    /// 连续多次零增益
    ZeroGainStreak { consecutive_pages: u32 },
}

/// 渐进式觅食评估器
///
/// 无状态设计的例外：评估器在单次觅食会话内维护已见 token 池，
/// 会话结束后销毁。不持久化，不跨请求复用。
#[derive(Debug, Clone)]
pub struct ForagingEvaluator {
    /// 检索目标的关键 token 集合（从用户查询中提取）
    target_tokens: HashSet<String>,
    /// 已观察到的 token 及其出现频次（本次会话内）
    seen_entropy: HashMap<String, u32>,
    /// 信息增益阈值（低于此值则终止）
    threshold_delta: f64,
    /// 本次会话已抓取的页面数
    pages_fetched: u32,
    /// 最大页面数上限（与 Manifest 中 max_pages_per_session 对应）
    max_pages_per_session: u32,
    /// 连续零增益次数
    zero_gain_streak: u32,
    /// 连续零增益上限（达到则终止）
    max_zero_gain_streak: u32,
    /// 全局已见熵布隆过滤器（本地 bloom feature，四修正2）
    ///
    /// 仅在 `bloom` feature 启用时存在。命中布隆的 token 增益计 0
    /// （全局记忆已覆盖，避免重复采集）。未启用时完全零开销。
    #[cfg(feature = "bloom")]
    seen_entropy_bloom: Option<Bloom<String>>,
}

impl ForagingEvaluator {
    /// 创建评估器
    ///
    /// # 参数
    /// - `query`: 用户查询文本，用于提取目标 token
    /// - `threshold_delta`: 信息增益阈值（默认 0.15）
    /// - `max_pages_per_session`: 最大页面数（默认 50）
    pub fn new(query: &str, threshold_delta: f64, max_pages_per_session: u32) -> Self {
        let target_tokens = extract_tokens(query);
        Self {
            target_tokens,
            seen_entropy: HashMap::new(),
            threshold_delta,
            pages_fetched: 0,
            max_pages_per_session,
            zero_gain_streak: 0,
            max_zero_gain_streak: 3,
            #[cfg(feature = "bloom")]
            seen_entropy_bloom: None,
        }
    }

    /// 设置全局已见熵布隆过滤器（四修正2，本地 bloom feature）
    ///
    /// 仅在 `bloom` feature 启用时可用。命中布隆的 token 增益计 0。
    #[cfg(feature = "bloom")]
    pub fn with_seen_entropy_bloom(mut self, bloom: Bloom<String>) -> Self {
        self.seen_entropy_bloom = Some(bloom);
        self
    }

    /// 检查 token 是否被全局已见熵覆盖（布隆命中）
    #[cfg(feature = "bloom")]
    fn is_globally_seen(&self, token: &str) -> bool {
        self.seen_entropy_bloom
            .as_ref()
            .map_or(false, |bloom| bloom.check(&token.to_string()))
    }

    /// 未启用 bloom feature 时的占位：token 永远不被全局覆盖
    #[cfg(not(feature = "bloom"))]
    fn is_globally_seen(&self, _token: &str) -> bool {
        false
    }

    /// 评估新页面的信息增益，决定继续还是停止
    ///
    /// # 算法
    /// 1. pages_fetched += 1
    /// 2. 检查最大页面数上限
    /// 3. 分词（英文按非字母数字分割，中文用字符级 2-Gram）
    /// 4. 计算新信息增益：新页面中与目标相关的、且之前未出现过的 token 数量 / 目标 token 总数
    /// 5. 更新已观察 token 池
    /// 6. 判断终止条件：增益低于阈值 / 连续零增益
    pub fn evaluate_gain(&mut self, page_text: &str) -> ForageDecision {
        self.pages_fetched += 1;

        // 检查最大页面数
        if self.pages_fetched > self.max_pages_per_session {
            return ForageDecision::Stop {
                reason: ForageStopReason::MaxPagesReached { pages: self.pages_fetched },
            };
        }

        // 分词
        let new_tokens = extract_tokens(page_text);

        // 计算新信息增益：与目标相关、本次会话未见、且全局已见熵未覆盖的 token 数量
        let novel_target_tokens = new_tokens
            .iter()
            .filter(|t| {
                self.target_tokens.contains(*t)
                    && !self.seen_entropy.contains_key(*t)
                    && !self.is_globally_seen(t)
            })
            .count();

        let gain = if self.target_tokens.is_empty() {
            0.0
        } else {
            novel_target_tokens as f64 / self.target_tokens.len() as f64
        };

        // 更新已观察 token 池
        for t in &new_tokens {
            *self.seen_entropy.entry(t.clone()).or_insert(0) += 1;
        }

        // 连续零增益检测
        if gain == 0.0 {
            self.zero_gain_streak += 1;
            if self.zero_gain_streak >= self.max_zero_gain_streak {
                return ForageDecision::Stop {
                    reason: ForageStopReason::ZeroGainStreak {
                        consecutive_pages: self.zero_gain_streak,
                    },
                };
            }
        } else {
            self.zero_gain_streak = 0;
        }

        // 信息增益阈值检测
        if gain < self.threshold_delta {
            return ForageDecision::Stop {
                reason: ForageStopReason::LowInformationGain {
                    last_gain: gain,
                    threshold: self.threshold_delta,
                },
            };
        }

        ForageDecision::Continue
    }

    /// 获取当前已抓取页面数
    pub fn pages_fetched(&self) -> u32 {
        self.pages_fetched
    }

    /// 获取当前已见 token 数
    pub fn seen_token_count(&self) -> usize {
        self.seen_entropy.len()
    }

    /// 获取目标 token 数
    pub fn target_token_count(&self) -> usize {
        self.target_tokens.len()
    }
}

/// 从文本中提取 token 集合
///
/// 分词策略：
/// - 英文/数字：按非字母数字字符分割，转小写，长度 >= 2
/// - 中文：字符级 2-Gram 滑窗（连续两个中文字符组成一个 token）
/// - 混合文本：两种策略并行，结果合并
///
/// 中文 N-Gram 不引入重型字典分词器，保持零依赖下的足够鲁棒性。
pub fn extract_tokens(text: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();

    // 策略 1：英文/数字分词（按非字母数字分割）
    let mut current = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() && !is_cjk(c) {
            current.push(c.to_ascii_lowercase());
        } else {
            if current.len() >= 2 {
                tokens.insert(current.clone());
            }
            current.clear();
        }
    }
    if current.len() >= 2 {
        tokens.insert(current);
    }

    // 策略 2：中文字符级 2-Gram 滑窗
    let cjk_chars: Vec<char> = text.chars().filter(|c| is_cjk(*c)).collect();
    for pair in cjk_chars.windows(2) {
        let mut token = String::with_capacity(2);
        token.push(pair[0]);
        token.push(pair[1]);
        tokens.insert(token);
    }

    tokens
}

/// 判断字符是否为 CJK（中日韩）字符
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF   // CJK Unified Ideographs
        | 0x3400..=0x4DBF   // CJK Unified Ideographs Extension A
        | 0x3040..=0x309F   // Hiragana
        | 0x30A0..=0x30FF   // Katakana
        | 0xAC00..=0xD7AF    // Hangul Syllables
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tokens_english() {
        let tokens = extract_tokens("Hello World hello");
        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        // 单字符不提取
        assert!(!tokens.contains("a"));
    }

    #[test]
    fn test_extract_tokens_chinese_ngram() {
        let tokens = extract_tokens("赛格大厦振动");
        // 2-Gram: 赛格, 格大, 大厦, 厦振, 振动
        assert!(tokens.contains("赛格"));
        assert!(tokens.contains("大厦"));
        assert!(tokens.contains("振动"));
        assert_eq!(tokens.len(), 5);
    }

    #[test]
    fn test_extract_tokens_mixed() {
        let tokens = extract_tokens("赛格大厦 SEG vibration");
        assert!(tokens.contains("赛格"));
        assert!(tokens.contains("大厦"));
        assert!(tokens.contains("seg"));
        assert!(tokens.contains("vibration"));
    }

    #[test]
    fn test_forager_continue_high_gain() {
        let mut forager = ForagingEvaluator::new("赛格大厦 振动", 0.15, 50);
        // 第一页包含大量目标相关内容
        let decision = forager.evaluate_gain("赛格大厦发生振动事件，居民紧急疏散，专家分析原因");
        assert_eq!(decision, ForageDecision::Continue);
        assert_eq!(forager.pages_fetched(), 1);
    }

    #[test]
    fn test_forager_stop_low_gain() {
        let mut forager = ForagingEvaluator::new("赛格大厦 振动", 0.15, 50);
        // 第一页高增益
        forager.evaluate_gain("赛格大厦发生振动事件，居民紧急疏散");
        // 第二页完全无关（增益为 0 < 0.15）
        let decision = forager.evaluate_gain("今天天气很好，适合出去散步");
        match decision {
            ForageDecision::Stop { reason: ForageStopReason::LowInformationGain { .. } } => {}
            _ => panic!("expected LowInformationGain, got {:?}", decision),
        }
    }

    #[test]
    fn test_forager_stop_max_pages() {
        let mut forager = ForagingEvaluator::new("test query", 0.01, 2);
        // 第 1 页
        forager.evaluate_gain("test query content page one");
        // 第 2 页
        forager.evaluate_gain("test query content page two");
        // 第 3 页超过上限
        let decision = forager.evaluate_gain("test query content page three");
        match decision {
            ForageDecision::Stop { reason: ForageStopReason::MaxPagesReached { pages } } => {
                assert_eq!(pages, 3);
            }
            _ => panic!("expected MaxPagesReached, got {:?}", decision),
        }
    }

    #[test]
    fn test_forager_zero_gain_streak() {
        let mut forager = ForagingEvaluator::new("赛格大厦", 0.15, 50);
        // 第一页高增益
        forager.evaluate_gain("赛格大厦振动事件分析");
        // 连续 3 页零增益
        forager.evaluate_gain("无关内容一");
        forager.evaluate_gain("无关内容二");
        let decision = forager.evaluate_gain("无关内容三");
        match decision {
            ForageDecision::Stop { reason: ForageStopReason::ZeroGainStreak { consecutive_pages } } => {
                assert_eq!(consecutive_pages, 3);
            }
            _ => panic!("expected ZeroGainStreak, got {:?}", decision),
        }
    }

    #[test]
    fn test_forager_seen_entropy_accumulates() {
        let mut forager = ForagingEvaluator::new("hello world", 0.15, 50);
        assert_eq!(forager.seen_token_count(), 0);
        forager.evaluate_gain("hello world foo bar");
        assert!(forager.seen_token_count() > 0);
        // 第二页重复 token 不增加增益
        let before = forager.seen_token_count();
        forager.evaluate_gain("hello world"); // 重复，无新 token
        // seen_entropy 频次增加，但 token 种类不变
        assert_eq!(forager.seen_token_count(), before);
    }

    #[test]
    fn test_forager_empty_query() {
        // 空查询：target_tokens 为空，增益始终为 0
        let mut forager = ForagingEvaluator::new("", 0.15, 50);
        assert_eq!(forager.target_token_count(), 0);
        let decision = forager.evaluate_gain("any content");
        // 空查询时增益为 0，应触发 LowInformationGain 或 ZeroGainStreak
        assert!(matches!(decision, ForageDecision::Stop { .. }));
    }

    #[test]
    fn test_forager_chinese_query() {
        let mut forager = ForagingEvaluator::new("赛格大厦 振动 政策", 0.15, 50);
        // 目标 token 应包含中文 2-Gram
        assert!(forager.target_token_count() > 0);
        // 高增益页面
        let decision = forager.evaluate_gain("赛格大厦振动事件后，政策变迁研究，居民安置方案");
        assert_eq!(decision, ForageDecision::Continue);
    }

    // ===== 已见熵布隆过滤器测试（四修正2，仅 bloom feature 启用时编译）=====

    #[cfg(feature = "bloom")]
    #[test]
    fn test_bloom_hit_zero_gain() {
        // 先提取查询的所有目标 token（含中文 2-Gram 中间词如"格大"），全部插入布隆
        let query = "赛格大厦 振动";
        let target_tokens = extract_tokens(query);
        let mut bloom = Bloom::new(100, 1000);
        for token in &target_tokens {
            bloom.set(&token.clone());
        }

        let mut forager = ForagingEvaluator::new(query, 0.15, 50)
            .with_seen_entropy_bloom(bloom);

        // 页面包含目标 token，但全部被布隆覆盖 → 增益为 0 → Stop
        let decision = forager.evaluate_gain("赛格大厦振动事件，居民紧急疏散");
        match decision {
            ForageDecision::Stop { reason: ForageStopReason::LowInformationGain { last_gain, .. } } => {
                assert_eq!(last_gain, 0.0);
            }
            ForageDecision::Stop { reason: ForageStopReason::ZeroGainStreak { .. } } => {}
            _ => panic!("expected Stop (zero gain due to bloom), got {:?}", decision),
        }
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn test_bloom_miss_normal_gain() {
        // 布隆中不包含目标 token
        let bloom = Bloom::new(100, 1000);

        let mut forager = ForagingEvaluator::new("赛格大厦 振动", 0.15, 50)
            .with_seen_entropy_bloom(bloom);

        // 页面包含目标 token，布隆未覆盖 → 正常增益 → Continue
        let decision = forager.evaluate_gain("赛格大厦振动事件，居民紧急疏散");
        assert_eq!(decision, ForageDecision::Continue);
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn test_bloom_partial_coverage() {
        // 布隆只覆盖部分目标 token
        let mut bloom = Bloom::new(100, 1000);
        bloom.set(&"赛格".to_string());
        bloom.set(&"大厦".to_string());
        // "振动" 未被覆盖

        let mut forager = ForagingEvaluator::new("赛格大厦 振动", 0.15, 50)
            .with_seen_entropy_bloom(bloom);

        // "振动" 未被布隆覆盖 → 仍有增益 → Continue
        let decision = forager.evaluate_gain("赛格大厦振动事件，居民紧急疏散");
        assert_eq!(decision, ForageDecision::Continue);
    }

    #[test]
    fn test_no_bloom_backward_compatible() {
        // 不启用布隆（或不设置），行为与 T1 完全一致
        let mut forager = ForagingEvaluator::new("赛格大厦 振动", 0.15, 50);
        let decision = forager.evaluate_gain("赛格大厦振动事件，居民紧急疏散");
        assert_eq!(decision, ForageDecision::Continue);
    }
}
