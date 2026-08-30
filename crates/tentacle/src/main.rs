//! Helix-Tentacle 二进制入口
//!
//! P5-T5：STDIO 传输层实现（零配置本地使用）
//!
//! 用法：
//! - STDIO 模式（默认）：`echo '{"tool":"mock","params":{}}' | tentacle`
//! - HTTP 模式：`tentacle --transport http --port 3000`
//! - 指定插件目录：`tentacle --plugins-dir ./plugins`

use clap::Parser;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tentacle_core::{
    consensus::{ConsensusHook, NoopConsensus},
    redact::Redactor,
    registry::ToolRegistry,
    tool::{ExecutionRequest, ToolOutput},
};
use tentacle_transport_http::{router, AppState};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

/// Helix-Tentacle — 工具执行引擎
#[derive(Parser, Debug)]
#[command(name = "tentacle", version, about = "Helix-Tentacle 工具执行引擎")]
struct Args {
    /// 传输模式：stdio 或 http
    #[arg(long, default_value = "stdio")]
    transport: String,

    /// HTTP 监听端口（仅 http 模式）
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// 插件目录（可选，启动时自动扫描）
    #[arg(long)]
    plugins_dir: Option<String>,

    /// 共识模式：noop 或 interactive（默认 noop）
    #[arg(long, default_value = "noop")]
    consensus: String,
}

#[tokio::main]
async fn main() {
    // 初始化日志（输出到 stderr，避免干扰 STDIO 模式的 JSON 输出）
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let args = Args::parse();

    info!("Helix-Tentacle v{} 启动", env!("CARGO_PKG_VERSION"));
    info!("传输模式: {}", args.transport);

    // 构建工具注册表
    let mut registry = if let Some(ref dir) = args.plugins_dir {
        info!("插件目录: {}", dir);
        ToolRegistry::with_plugin_dir(dir)
    } else {
        ToolRegistry::new()
    };

    // 扫描插件
    match registry.scan_plugins() {
        Ok(report) => {
            info!("插件扫描完成: 扫描 {} 个, 成功注册 {} 个, 跳过 {} 个",
                report.scanned, report.registered, report.skipped.len());
            if !report.skipped.is_empty() {
                for entry in &report.skipped {
                    warn!("插件跳过 [{}]: {}", entry.manifest_path.display(), entry.reason);
                }
            }
        }
        Err(e) => {
            warn!("插件扫描失败: {}（继续以空注册表运行）", e);
        }
    }

    info!("已注册工具数: {}", registry.len());

    // 根据传输模式启动
    match args.transport.as_str() {
        "stdio" => run_stdio(registry),
        "http" => run_http(registry, args.port).await,
        other => {
            error!("不支持的传输模式: {}（请使用 stdio 或 http）", other);
            std::process::exit(1);
        }
    }
}

/// STDIO 模式：从 stdin 读取 JSON 请求，执行工具，输出 JSON 结果到 stdout
///
/// 支持两种输入方式：
/// 1. 单行模式：每行一个 JSON 请求（适合管道和脚本）
/// 2. 批量模式：一次性输入多个 JSON 请求（用换行分隔）
///
/// 示例：
/// ```bash
/// echo '{"tool":"mock","params":{"input":"test"}}' | tentacle
/// ```
fn run_stdio(registry: ToolRegistry) {
    info!("STDIO 模式启动（每行一个 JSON 请求，Ctrl+D 结束）");

    let redactor = Arc::new(Redactor::default());
    let consensus = Arc::new(NoopConsensus);
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for (line_num, line) in stdin.lock().lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                error!("读取 stdin 失败: {}", e);
                break;
            }
        };

        // 跳过空行和注释行
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // 解析请求
        let req: ExecutionRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_output = ToolOutput::failure(format!("JSON 解析失败(行{}): {}", line_num + 1, e));
                writeln!(stdout, "{}", serde_json::to_string(&err_output).unwrap()).ok();
                continue;
            }
        };

        // 查找工具
        let tool = match registry.get_tool(&req.tool) {
            Some(t) => t.clone(),
            None => {
                let err_output = ToolOutput::failure(format!("工具未找到: {}", req.tool));
                writeln!(stdout, "{}", serde_json::to_string(&err_output).unwrap()).ok();
                continue;
            }
        };

        // Critical 级工具需共识审批（四修正4）
        let manifest = tool.manifest().clone();
        if matches!(manifest.security_level, tentacle_core::manifest::SecurityLevel::Critical) {
            match consensus.request_approval(&req.tool, &req.params) {
                Ok(approval) if !approval.approved => {
                    let err_output = ToolOutput::failure(format!("共识被拒绝: {}", approval.reason.unwrap_or_default()));
                    writeln!(stdout, "{}", serde_json::to_string(&err_output).unwrap()).ok();
                    continue;
                }
                Err(e) => {
                    let err_output = ToolOutput::failure(format!("共识不可用: {}", e));
                    writeln!(stdout, "{}", serde_json::to_string(&err_output).unwrap()).ok();
                    continue;
                }
                _ => {}
            }
        }

        // 执行工具
        match tool.execute(req) {
            Ok(mut output) => {
                // 脱敏输出（默认开启不可绕过）
                if let Some(ref mut data) = output.data {
                    redactor.redact(data);
                }
                writeln!(stdout, "{}", serde_json::to_string(&output).unwrap()).ok();
            }
            Err(e) => {
                let err_output = ToolOutput::failure(format!("执行失败: {}", e));
                writeln!(stdout, "{}", serde_json::to_string(&err_output).unwrap()).ok();
            }
        }

        // 刷新 stdout，确保输出立即可见
        stdout.flush().ok();
    }

    info!("STDIO 模式结束");
}

/// HTTP 模式：启动 axum RESTful 服务器
async fn run_http(registry: ToolRegistry, port: u16) {
    let state = AppState::new(registry);

    // 扫描插件（AppState 内部已封装）
    match state.scan_plugins() {
        Ok(report) => {
            info!("插件扫描完成: 扫描 {} 个, 成功注册 {} 个", report.scanned, report.registered);
        }
        Err(e) => {
            warn!("插件扫描失败: {}", e);
        }
    }

    let app = router(state);
    let addr = format!("0.0.0.0:{}", port);
    info!("HTTP 服务器监听: http://{}", addr);
    info!("端点:");
    info!("  GET  /v1/manifest              — 工具索引");
    info!("  GET  /v1/tools/{{name}}/manifest — 工具说明书");
    info!("  POST /v1/tools/{{name}}/execute  — 执行工具");
    info!("  GET  /metrics                    — Prometheus 指标");

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("无法绑定端口 {}: {}", port, e);
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        error!("HTTP 服务器错误: {}", e);
        std::process::exit(1);
    }
}
