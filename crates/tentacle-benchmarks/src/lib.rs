//! tentacle-benchmarks — 性能基准测试共享工具
//!
//! 提供 MockTool、各传输层服务器启动辅助函数，供基准测试复用。

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use tentacle_core::error::ToolError;
use tentacle_core::manifest::{Integrity, Manifest, SecurityLevel};
use tentacle_core::registry::ToolRegistry;
use tentacle_core::tool::{ExecutionRequest, Tool, ToolOutput};

/// Mock 工具：立即返回固定结果，用于基准测试（无实际工作，纯测量传输层开销）
pub struct MockTool {
    name: String,
    description: String,
    security_level: SecurityLevel,
    manifest: Manifest,
    latency_us: u64, // 模拟处理延迟（微秒），0 = 无延迟
}

impl MockTool {
    pub fn new(name: &str, security_level: SecurityLevel) -> Self {
        let manifest = Manifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: format!("Mock tool for benchmarking: {}", name),
            author: None,
            tags: vec!["benchmark".to_string()],
            executable: format!("{}.wasm", name),
            integrity: Integrity::sha256("00000000000000000000000000000000000000000000000000000000000000"),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "test input" }
                }
            }),
            examples: vec![],
            trigger_phrases: vec![],
            security_level: security_level.clone(),
            permissions: Default::default(),
            requires_identity: None,
            requires_external_signer: false,
            allowed_signing_domains: vec![],
            rate_limit_per_minute: None,
            timeout_ms: 5000,
            foraging_config: Default::default(),
        };
        Self {
            name: name.to_string(),
            description: format!("Mock tool for benchmarking: {}", name),
            security_level,
            manifest,
            latency_us: 0,
        }
    }

    pub fn with_latency(mut self, latency_us: u64) -> Self {
        self.latency_us = latency_us;
        self
    }
}

impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn execute(&self, request: ExecutionRequest) -> Result<ToolOutput, ToolError> {
        // 模拟处理延迟（同步阻塞）
        if self.latency_us > 0 {
            std::thread::sleep(std::time::Duration::from_micros(self.latency_us));
        }

        let input = request.params.get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        Ok(ToolOutput::success(json!({
            "result": format!("processed: {}", input),
            "tool": self.name,
            "trace_id": request.trace_id.unwrap_or_default(),
        })))
    }
}

/// 创建包含多个 MockTool 的 ToolRegistry
pub fn create_test_registry(tool_count: usize) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    for i in 0..tool_count {
        let level = if i % 2 == 0 { SecurityLevel::Normal } else { SecurityLevel::Critical };
        let tool = MockTool::new(&format!("mock_tool_{}", i), level);
        registry.register_tool(Arc::new(tool)).expect("failed to register tool");
    }
    registry
}

/// 基准测试结果汇总
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub transport: String,
    pub operation: String,
    pub concurrency: usize,
    pub total_requests: usize,
    pub total_duration_ms: u128,
    pub throughput_rps: f64,
    pub latency_p50_us: u128,
    pub latency_p95_us: u128,
    pub latency_p99_us: u128,
    pub latency_avg_us: u128,
}

impl BenchmarkResult {
    pub fn new(transport: &str, operation: &str, concurrency: usize) -> Self {
        Self {
            transport: transport.to_string(),
            operation: operation.to_string(),
            concurrency,
            total_requests: 0,
            total_duration_ms: 0,
            throughput_rps: 0.0,
            latency_p50_us: 0,
            latency_p95_us: 0,
            latency_p99_us: 0,
            latency_avg_us: 0,
        }
    }
}

/// 计算百分位延迟
pub fn percentile(latencies: &mut [u128], p: f64) -> u128 {
    if latencies.is_empty() {
        return 0;
    }
    latencies.sort();
    let idx = ((latencies.len() as f64 - 1.0) * p / 100.0).round() as usize;
    latencies[idx.min(latencies.len() - 1)]
}

/// 格式化基准测试结果为表格行
pub fn format_result_table(results: &[BenchmarkResult]) -> String {
    let mut table = String::new();
    table.push_str(&format!("| {:<12} | {:<15} | {:>5} | {:>10} | {:>12} | {:>10} | {:>10} | {:>10} |\n",
        "Transport", "Operation", "Conc", "Requests", "Throughput", "p50(us)", "p95(us)", "p99(us)"));
    table.push_str(&format!("|{}|{}|{}|{}|{}|{}|{}|{}|\n",
        "-".repeat(14), "-".repeat(17), "-".repeat(7), "-".repeat(12), "-".repeat(14), "-".repeat(12), "-".repeat(12), "-".repeat(12)));
    for r in results {
        table.push_str(&format!("| {:<12} | {:<15} | {:>5} | {:>10} | {:>10.1}/s | {:>10} | {:>10} | {:>10} |\n",
            r.transport, r.operation, r.concurrency, r.total_requests, r.throughput_rps,
            r.latency_p50_us, r.latency_p95_us, r.latency_p99_us));
    }
    table
}
