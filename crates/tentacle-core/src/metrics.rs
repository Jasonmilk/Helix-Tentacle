//! tentacle-core — 可观测性模块（Metrics）
//!
//! P5-T3：轻量级指标收集器，支持 Counter/Gauge/Histogram 三种指标类型，
//! 集成 ResourceLimiter 使用量统计，支持 JSON 格式导出。
//!
//! 设计原则：
//! - 极致解耦：MetricsCollector trait 与具体实现分离
//! - 按需加载：不依赖外部 metrics crate，轻量级实现
//! - 确定性优先：指标收集是固定路径，无分支判断
//! - 极致复用：InMemoryMetrics 可复用于所有传输层和沙箱

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

// ============================================================================
// 指标类型定义
// ============================================================================

/// 指标类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricType {
    /// 计数器（单调递增）
    Counter,
    /// 仪表盘（可增可减）
    Gauge,
    /// 直方图（统计分布）
    Histogram,
}

/// 指标值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    /// 整数计数器/仪表盘
    Int(u64),
    /// 浮点数仪表盘
    Float(f64),
    /// 直方图（桶统计）
    Histogram {
        /// 样本总数
        count: u64,
        /// 样本总和
        sum: f64,
        /// 桶边界（上界）
        buckets: Vec<f64>,
        /// 每个桶的累计计数
        bucket_counts: Vec<u64>,
    },
}

impl MetricValue {
    /// 获取数值（用于 Counter/Gauge）
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Int(v) => *v as f64,
            Self::Float(v) => *v,
            Self::Histogram { sum, count, .. } => {
                if *count > 0 {
                    sum / *count as f64
                } else {
                    0.0
                }
            }
        }
    }
}

/// 指标定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    /// 指标名称（如 "tentacle_tool_executions_total"）
    pub name: String,
    /// 指标描述
    #[serde(default)]
    pub help: String,
    /// 指标类型
    pub metric_type: MetricType,
    /// 标签（如 {"tool": "scraper", "transport": "http"}）
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// 指标值
    pub value: MetricValue,
    /// 最后更新时间（Unix 时间戳，毫秒）
    #[serde(default)]
    pub updated_at_ms: u64,
}

// ============================================================================
// 指标收集器 trait
// ============================================================================

/// 指标收集器 trait
///
/// 所有需要收集指标的组件都应使用此 trait。
pub trait MetricsCollector: Send + Sync {
    /// 增加计数器
    fn increment_counter(&self, name: &str, value: u64, labels: Option<HashMap<String, String>>);

    /// 设置仪表盘值（整数）
    fn set_gauge_int(&self, name: &str, value: u64, labels: Option<HashMap<String, String>>);

    /// 设置仪表盘值（浮点数）
    fn set_gauge_float(&self, name: &str, value: f64, labels: Option<HashMap<String, String>>);

    /// 观察直方图值
    fn observe_histogram(&self, name: &str, value: f64, labels: Option<HashMap<String, String>>);

    /// 获取所有指标
    fn get_metrics(&self) -> Vec<Metric>;

    /// 获取指定名称的指标
    fn get_metric(&self, name: &str) -> Option<Metric>;

    /// 注册指标（设置 help 信息）
    fn register_metric(&self, name: &str, help: &str, metric_type: MetricType);

    /// 导出为 JSON 格式
    fn export_json(&self) -> String {
        let metrics = self.get_metrics();
        serde_json::to_string_pretty(&metrics).unwrap_or_else(|_| "[]".to_string())
    }

    /// 导出为 Prometheus 文本格式（简化版）
    fn export_prometheus(&self) -> String {
        let metrics = self.get_metrics();
        let mut output = String::new();

        for metric in &metrics {
            if !metric.help.is_empty() {
                output.push_str(&format!("# HELP {} {}\n", metric.name, metric.help));
            }
            output.push_str(&format!(
                "# TYPE {} {}\n",
                metric.name,
                match metric.metric_type {
                    MetricType::Counter => "counter",
                    MetricType::Gauge => "gauge",
                    MetricType::Histogram => "histogram",
                }
            ));

            let labels_str = if metric.labels.is_empty() {
                String::new()
            } else {
                let pairs: Vec<String> = metric
                    .labels
                    .iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                format!("{{{}}}", pairs.join(","))
            };

            match &metric.value {
                MetricValue::Int(v) => {
                    output.push_str(&format!("{}{} {}\n", metric.name, labels_str, v));
                }
                MetricValue::Float(v) => {
                    output.push_str(&format!("{}{} {}\n", metric.name, labels_str, v));
                }
                MetricValue::Histogram {
                    count,
                    sum,
                    buckets,
                    bucket_counts,
                } => {
                    for (i, bucket) in buckets.iter().enumerate() {
                        let le_label = if i < bucket_counts.len() {
                            bucket_counts[i]
                        } else {
                            0
                        };
                        output.push_str(&format!(
                            "{}_bucket{{le=\"{}\"{}}} {}\n",
                            metric.name,
                            bucket,
                            if labels_str.is_empty() {
                                String::new()
                            } else {
                                format!(",{}", &labels_str[1..labels_str.len() - 1])
                            },
                            le_label
                        ));
                    }
                    output.push_str(&format!("{}_sum{} {}\n", metric.name, labels_str, sum));
                    output.push_str(&format!("{}_count{} {}\n", metric.name, labels_str, count));
                }
            }
            output.push('\n');
        }

        output
    }
}

// ============================================================================
// 内存指标收集器（默认实现）
// ============================================================================

/// 内存指标收集器
///
/// 使用 Mutex<HashMap> 存储指标，线程安全，适用于多线程环境。
#[derive(Debug, Clone)]
pub struct InMemoryMetrics {
    inner: Arc<Mutex<InMemoryMetricsInner>>,
}

#[derive(Debug)]
struct InMemoryMetricsInner {
    /// 指标存储：key = name + 排序后的标签
    metrics: HashMap<String, Metric>,
    /// 指标元数据：name -> (help, type)
    metadata: HashMap<String, (String, MetricType)>,
    /// 直方图默认桶边界
    default_histogram_buckets: Vec<f64>,
}

impl Default for InMemoryMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryMetrics {
    /// 创建新的内存指标收集器
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(InMemoryMetricsInner {
                metrics: HashMap::new(),
                metadata: HashMap::new(),
                // 默认直方图桶（毫秒）：0.1ms, 0.5ms, 1ms, 5ms, 10ms, 50ms, 100ms, 500ms, 1s, 5s, +Inf
                default_histogram_buckets: vec![
                    0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, f64::INFINITY,
                ],
            })),
        }
    }

    /// 生成指标的唯一 key（name + 排序后的标签）
    fn metric_key(name: &str, labels: &Option<HashMap<String, String>>) -> String {
        match labels {
            Some(labels) if !labels.is_empty() => {
                let mut sorted: Vec<(&String, &String)> = labels.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));
                let label_str: String = sorted
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{}|{}", name, label_str)
            }
            _ => name.to_string(),
        }
    }

    /// 获取当前时间戳（毫秒）
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

impl MetricsCollector for InMemoryMetrics {
    fn increment_counter(&self, name: &str, value: u64, labels: Option<HashMap<String, String>>) {
        let key = Self::metric_key(name, &labels);
        let mut inner = self.inner.lock().unwrap();
        let now = Self::now_ms();

        if let Some(metric) = inner.metrics.get_mut(&key) {
            if let MetricValue::Int(ref mut v) = metric.value {
                *v += value;
                metric.updated_at_ms = now;
            }
        } else {
            let (help, metric_type) = inner
                .metadata
                .get(name)
                .cloned()
                .unwrap_or_else(|| (String::new(), MetricType::Counter));
            inner.metrics.insert(
                key,
                Metric {
                    name: name.to_string(),
                    help,
                    metric_type,
                    labels: labels.unwrap_or_default(),
                    value: MetricValue::Int(value),
                    updated_at_ms: now,
                },
            );
        }
    }

    fn set_gauge_int(&self, name: &str, value: u64, labels: Option<HashMap<String, String>>) {
        let key = Self::metric_key(name, &labels);
        let mut inner = self.inner.lock().unwrap();
        let now = Self::now_ms();

        if let Some(metric) = inner.metrics.get_mut(&key) {
            metric.value = MetricValue::Int(value);
            metric.updated_at_ms = now;
        } else {
            let (help, metric_type) = inner
                .metadata
                .get(name)
                .cloned()
                .unwrap_or_else(|| (String::new(), MetricType::Gauge));
            inner.metrics.insert(
                key,
                Metric {
                    name: name.to_string(),
                    help,
                    metric_type,
                    labels: labels.unwrap_or_default(),
                    value: MetricValue::Int(value),
                    updated_at_ms: now,
                },
            );
        }
    }

    fn set_gauge_float(&self, name: &str, value: f64, labels: Option<HashMap<String, String>>) {
        let key = Self::metric_key(name, &labels);
        let mut inner = self.inner.lock().unwrap();
        let now = Self::now_ms();

        if let Some(metric) = inner.metrics.get_mut(&key) {
            metric.value = MetricValue::Float(value);
            metric.updated_at_ms = now;
        } else {
            let (help, metric_type) = inner
                .metadata
                .get(name)
                .cloned()
                .unwrap_or_else(|| (String::new(), MetricType::Gauge));
            inner.metrics.insert(
                key,
                Metric {
                    name: name.to_string(),
                    help,
                    metric_type,
                    labels: labels.unwrap_or_default(),
                    value: MetricValue::Float(value),
                    updated_at_ms: now,
                },
            );
        }
    }

    fn observe_histogram(&self, name: &str, value: f64, labels: Option<HashMap<String, String>>) {
        let key = Self::metric_key(name, &labels);
        let mut inner = self.inner.lock().unwrap();
        let now = Self::now_ms();
        let buckets = inner.default_histogram_buckets.clone();

        if let Some(metric) = inner.metrics.get_mut(&key) {
            if let MetricValue::Histogram {
                ref mut count,
                ref mut sum,
                ref mut bucket_counts,
                ..
            } = metric.value
            {
                *count += 1;
                *sum += value;
                // 更新桶计数（累计）
                for (i, bucket) in buckets.iter().enumerate() {
                    if value <= *bucket && i < bucket_counts.len() {
                        bucket_counts[i] += 1;
                    }
                }
                metric.updated_at_ms = now;
            }
        } else {
            let (help, metric_type) = inner
                .metadata
                .get(name)
                .cloned()
                .unwrap_or_else(|| (String::new(), MetricType::Histogram));
            // 初始化桶计数
            let mut bucket_counts = vec![0u64; buckets.len()];
            for (i, bucket) in buckets.iter().enumerate() {
                if value <= *bucket {
                    bucket_counts[i] = 1;
                }
            }
            inner.metrics.insert(
                key,
                Metric {
                    name: name.to_string(),
                    help,
                    metric_type,
                    labels: labels.unwrap_or_default(),
                    value: MetricValue::Histogram {
                        count: 1,
                        sum: value,
                        buckets,
                        bucket_counts,
                    },
                    updated_at_ms: now,
                },
            );
        }
    }

    fn get_metrics(&self) -> Vec<Metric> {
        let inner = self.inner.lock().unwrap();
        inner.metrics.values().cloned().collect()
    }

    fn get_metric(&self, name: &str) -> Option<Metric> {
        let inner = self.inner.lock().unwrap();
        inner
            .metrics
            .values()
            .find(|m| m.name == name)
            .cloned()
    }

    fn register_metric(&self, name: &str, help: &str, metric_type: MetricType) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .metadata
            .insert(name.to_string(), (help.to_string(), metric_type));
    }
}

// ============================================================================
// 标准指标定义（Tentacle 内置指标）
// ============================================================================

/// Tentacle 标准指标名称常量
pub mod metrics_names {
    /// 工具执行总数（Counter）
    pub const TOOL_EXECUTIONS_TOTAL: &str = "tentacle_tool_executions_total";
    /// 工具执行失败总数（Counter）
    pub const TOOL_EXECUTIONS_FAILED_TOTAL: &str = "tentacle_tool_executions_failed_total";
    /// 工具执行耗时（Histogram，秒）
    pub const TOOL_EXECUTION_DURATION_SECONDS: &str = "tentacle_tool_execution_duration_seconds";
    /// 当前活跃工具数（Gauge）
    pub const TOOLS_ACTIVE: &str = "tentacle_tools_active";
    /// 已注册工具数（Gauge）
    pub const TOOLS_REGISTERED: &str = "tentacle_tools_registered";
    /// 沙箱内存使用（Gauge，字节）
    pub const SANDBOX_MEMORY_USED_BYTES: &str = "tentacle_sandbox_memory_used_bytes";
    /// 沙箱内存峰值（Gauge，字节）
    pub const SANDBOX_MEMORY_PEAK_BYTES: &str = "tentacle_sandbox_memory_peak_bytes";
    /// 沙箱 CPU 时间使用（Gauge，毫秒）
    pub const SANDBOX_CPU_TIME_USED_MS: &str = "tentacle_sandbox_cpu_time_used_ms";
    /// 沙箱文件描述符使用（Gauge）
    pub const SANDBOX_FD_USED: &str = "tentacle_sandbox_fd_used";
    /// 传输层请求总数（Counter）
    pub const TRANSPORT_REQUESTS_TOTAL: &str = "tentacle_transport_requests_total";
    /// 传输层请求失败总数（Counter）
    pub const TRANSPORT_REQUESTS_FAILED_TOTAL: &str = "tentacle_transport_requests_failed_total";
    /// 传输层请求耗时（Histogram，秒）
    pub const TRANSPORT_REQUEST_DURATION_SECONDS: &str = "tentacle_transport_request_duration_seconds";
}

/// 注册所有 Tentacle 标准指标
pub fn register_standard_metrics(collector: &dyn MetricsCollector) {
    use metrics_names::*;

    collector.register_metric(
        TOOL_EXECUTIONS_TOTAL,
        "Total number of tool executions",
        MetricType::Counter,
    );
    collector.register_metric(
        TOOL_EXECUTIONS_FAILED_TOTAL,
        "Total number of failed tool executions",
        MetricType::Counter,
    );
    collector.register_metric(
        TOOL_EXECUTION_DURATION_SECONDS,
        "Tool execution duration in seconds",
        MetricType::Histogram,
    );
    collector.register_metric(TOOLS_ACTIVE, "Number of currently active tools", MetricType::Gauge);
    collector.register_metric(
        TOOLS_REGISTERED,
        "Number of registered tools",
        MetricType::Gauge,
    );
    collector.register_metric(
        SANDBOX_MEMORY_USED_BYTES,
        "Sandbox memory usage in bytes",
        MetricType::Gauge,
    );
    collector.register_metric(
        SANDBOX_MEMORY_PEAK_BYTES,
        "Sandbox peak memory usage in bytes",
        MetricType::Gauge,
    );
    collector.register_metric(
        SANDBOX_CPU_TIME_USED_MS,
        "Sandbox CPU time usage in milliseconds",
        MetricType::Gauge,
    );
    collector.register_metric(
        SANDBOX_FD_USED,
        "Sandbox file descriptor usage",
        MetricType::Gauge,
    );
    collector.register_metric(
        TRANSPORT_REQUESTS_TOTAL,
        "Total number of transport layer requests",
        MetricType::Counter,
    );
    collector.register_metric(
        TRANSPORT_REQUESTS_FAILED_TOTAL,
        "Total number of failed transport layer requests",
        MetricType::Counter,
    );
    collector.register_metric(
        TRANSPORT_REQUEST_DURATION_SECONDS,
        "Transport layer request duration in seconds",
        MetricType::Histogram,
    );
}

// ============================================================================
// 资源使用指标桥接（ResourceLimiter -> Metrics）
// ============================================================================

use crate::resource::{ResourceLimiter, ResourceUsage};

/// 将 ResourceLimiter 的使用量同步到 MetricsCollector
pub fn record_resource_usage(
    collector: &dyn MetricsCollector,
    usage: &ResourceUsage,
    labels: Option<HashMap<String, String>>,
) {
    use metrics_names::*;

    collector.set_gauge_int(SANDBOX_MEMORY_USED_BYTES, usage.memory_used_bytes as u64, labels.clone());
    collector.set_gauge_int(SANDBOX_MEMORY_PEAK_BYTES, usage.memory_peak_bytes as u64, labels.clone());
    collector.set_gauge_int(SANDBOX_CPU_TIME_USED_MS, usage.cpu_time_used_ms, labels.clone());
    collector.set_gauge_int(SANDBOX_FD_USED, usage.fd_used as u64, labels);
}

/// 工具执行指标记录器（RAII 风格，自动记录耗时）
pub struct ToolExecutionMetrics<'a> {
    collector: &'a dyn MetricsCollector,
    tool_name: String,
    start: Instant,
    labels: HashMap<String, String>,
}

impl<'a> ToolExecutionMetrics<'a> {
    /// 创建新的工具执行指标记录器
    pub fn new(collector: &'a dyn MetricsCollector, tool_name: &str) -> Self {
        let mut labels = HashMap::new();
        labels.insert("tool".to_string(), tool_name.to_string());
        Self {
            collector,
            tool_name: tool_name.to_string(),
            start: Instant::now(),
            labels,
        }
    }

    /// 添加自定义标签
    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    /// 记录执行成功
    pub fn record_success(self) {
        use metrics_names::*;
        let duration = self.start.elapsed().as_secs_f64();
        self.collector
            .increment_counter(TOOL_EXECUTIONS_TOTAL, 1, Some(self.labels.clone()));
        self.collector.observe_histogram(
            TOOL_EXECUTION_DURATION_SECONDS,
            duration,
            Some(self.labels),
        );
    }

    /// 记录执行失败
    pub fn record_failure(self, error: &str) {
        use metrics_names::*;
        let duration = self.start.elapsed().as_secs_f64();
        let mut labels = self.labels.clone();
        labels.insert("error".to_string(), error.to_string());
        self.collector
            .increment_counter(TOOL_EXECUTIONS_TOTAL, 1, Some(self.labels.clone()));
        self.collector
            .increment_counter(TOOL_EXECUTIONS_FAILED_TOTAL, 1, Some(labels));
        self.collector.observe_histogram(
            TOOL_EXECUTION_DURATION_SECONDS,
            duration,
            Some(self.labels),
        );
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_value_as_f64() {
        assert_eq!(MetricValue::Int(42).as_f64(), 42.0);
        assert!((MetricValue::Float(3.14).as_f64() - 3.14).abs() < 0.01);
    }

    #[test]
    fn test_in_memory_metrics_counter() {
        let metrics = InMemoryMetrics::new();
        metrics.register_metric("test_counter", "test help", MetricType::Counter);

        metrics.increment_counter("test_counter", 1, None);
        metrics.increment_counter("test_counter", 5, None);

        let metric = metrics.get_metric("test_counter").unwrap();
        assert_eq!(metric.name, "test_counter");
        assert_eq!(metric.help, "test help");
        assert_eq!(metric.metric_type, MetricType::Counter);
        if let MetricValue::Int(v) = metric.value {
            assert_eq!(v, 6);
        } else {
            panic!("expected Int value");
        }
    }

    #[test]
    fn test_in_memory_metrics_gauge() {
        let metrics = InMemoryMetrics::new();

        metrics.set_gauge_int("test_gauge", 100, None);
        metrics.set_gauge_int("test_gauge", 200, None);

        let metric = metrics.get_metric("test_gauge").unwrap();
        if let MetricValue::Int(v) = metric.value {
            assert_eq!(v, 200);
        } else {
            panic!("expected Int value");
        }

        metrics.set_gauge_float("test_gauge_float", 3.14, None);
        let metric = metrics.get_metric("test_gauge_float").unwrap();
        if let MetricValue::Float(v) = metric.value {
            assert!((v - 3.14).abs() < 0.01);
        } else {
            panic!("expected Float value");
        }
    }

    #[test]
    fn test_in_memory_metrics_histogram() {
        let metrics = InMemoryMetrics::new();

        metrics.observe_histogram("test_hist", 0.001, None); // 1ms
        metrics.observe_histogram("test_hist", 0.01, None); // 10ms
        metrics.observe_histogram("test_hist", 0.1, None); // 100ms

        let metric = metrics.get_metric("test_hist").unwrap();
        if let MetricValue::Histogram { count, sum, .. } = metric.value {
            assert_eq!(count, 3);
            assert!((sum - 0.111).abs() < 0.001);
        } else {
            panic!("expected Histogram value");
        }
    }

    #[test]
    fn test_metrics_with_labels() {
        let metrics = InMemoryMetrics::new();

        let mut labels1 = HashMap::new();
        labels1.insert("tool".to_string(), "scraper".to_string());
        metrics.increment_counter("test_counter", 1, Some(labels1));

        let mut labels2 = HashMap::new();
        labels2.insert("tool".to_string(), "api".to_string());
        metrics.increment_counter("test_counter", 2, Some(labels2));

        let all_metrics = metrics.get_metrics();
        assert_eq!(all_metrics.len(), 2);
    }

    #[test]
    fn test_export_json() {
        let metrics = InMemoryMetrics::new();
        metrics.increment_counter("test_counter", 1, None);

        let json = metrics.export_json();
        assert!(json.contains("test_counter"));
        assert!(json.contains("counter"));
    }

    #[test]
    fn test_export_prometheus() {
        let metrics = InMemoryMetrics::new();
        metrics.register_metric("test_counter", "test help", MetricType::Counter);
        metrics.increment_counter("test_counter", 42, None);

        let prom = metrics.export_prometheus();
        assert!(prom.contains("# HELP test_counter test help"));
        assert!(prom.contains("# TYPE test_counter counter"));
        assert!(prom.contains("test_counter 42"));
    }

    #[test]
    fn test_register_standard_metrics() {
        let metrics = InMemoryMetrics::new();
        register_standard_metrics(&metrics);

        // 验证标准指标已注册（通过 increment 后检查 help）
        metrics.increment_counter(metrics_names::TOOL_EXECUTIONS_TOTAL, 1, None);
        let metric = metrics.get_metric(metrics_names::TOOL_EXECUTIONS_TOTAL).unwrap();
        assert_eq!(metric.help, "Total number of tool executions");
    }

    #[test]
    fn test_tool_execution_metrics_success() {
        let metrics = InMemoryMetrics::new();
        register_standard_metrics(&metrics);

        {
            let _m = ToolExecutionMetrics::new(&metrics, "test_tool")
                .with_label("transport", "http")
                .record_success();
        }

        let total = metrics.get_metric(metrics_names::TOOL_EXECUTIONS_TOTAL).unwrap();
        if let MetricValue::Int(v) = total.value {
            assert_eq!(v, 1);
        }

        let duration = metrics.get_metric(metrics_names::TOOL_EXECUTION_DURATION_SECONDS).unwrap();
        assert_eq!(duration.metric_type, MetricType::Histogram);
    }

    #[test]
    fn test_tool_execution_metrics_failure() {
        let metrics = InMemoryMetrics::new();
        register_standard_metrics(&metrics);

        {
            let _m = ToolExecutionMetrics::new(&metrics, "test_tool")
                .record_failure("timeout");
        }

        let failed = metrics.get_metric(metrics_names::TOOL_EXECUTIONS_FAILED_TOTAL).unwrap();
        if let MetricValue::Int(v) = failed.value {
            assert_eq!(v, 1);
        }
    }

    #[test]
    fn test_record_resource_usage() {
        let metrics = InMemoryMetrics::new();
        register_standard_metrics(&metrics);

        let usage = ResourceUsage {
            memory_used_bytes: 1024,
            memory_peak_bytes: 2048,
            cpu_time_used_ms: 100,
            fd_used: 5,
            output_bytes: 512,
            elapsed_ms: 200,
        };

        record_resource_usage(&metrics, &usage, None);

        let mem = metrics.get_metric(metrics_names::SANDBOX_MEMORY_USED_BYTES).unwrap();
        if let MetricValue::Int(v) = mem.value {
            assert_eq!(v, 1024);
        }

        let peak = metrics.get_metric(metrics_names::SANDBOX_MEMORY_PEAK_BYTES).unwrap();
        if let MetricValue::Int(v) = peak.value {
            assert_eq!(v, 2048);
        }
    }

    #[test]
    fn test_metric_key_deterministic() {
        let mut labels = HashMap::new();
        labels.insert("b".to_string(), "2".to_string());
        labels.insert("a".to_string(), "1".to_string());

        let key1 = InMemoryMetrics::metric_key("test", &Some(labels.clone()));
        let key2 = InMemoryMetrics::metric_key("test", &Some(labels));
        assert_eq!(key1, key2);
        assert!(key1.contains("a=1"));
        assert!(key1.contains("b=2"));
    }
}
