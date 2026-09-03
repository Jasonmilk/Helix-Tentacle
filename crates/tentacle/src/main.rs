//! Helix-Tentacle 二进制入口
//!
//! P5-T5：STDIO 传输层实现（零配置本地使用）
//!
//! 用法：
//! - STDIO 模式（默认）：`echo '{"tool":"mock","params":{}}' | tentacle`
//! - HTTP 模式：`tentacle --transport http --port 3000`
//! - gRPC 模式：`tentacle --transport grpc --grpc-port 50051`
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
    /// 传输模式：stdio、http 或 grpc
    #[arg(long, default_value = "stdio")]
    transport: String,

    /// HTTP 监听端口（仅 http 模式）
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// gRPC 监听端口（仅 grpc 模式）
    #[arg(long, default_value_t = 50051)]
    grpc_port: u16,

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

    // 为每个已注册的 Manifest 实例化 ProcessTool（执行外部命令/脚本）
    // 这是懒加载的临时实现，生产环境应使用专门的 PluginLoader
    if let Some(ref dir) = args.plugins_dir {
        let manifests: Vec<tentacle_core::Manifest> = registry
            .index()
            .iter()
            .filter_map(|idx| registry.get_manifest(&idx.name).cloned())
            .collect();

        for manifest in manifests {
            let executable_path = std::path::Path::new(dir).join(&manifest.executable);
            if executable_path.exists() {
                let tool = ProcessTool::new(manifest, executable_path);
                if let Err(e) = registry.register_tool(std::sync::Arc::new(tool)) {
                    warn!("工具注册失败: {}", e);
                }
            } else {
                warn!("执行体不存在: {} (工具: {})", executable_path.display(), manifest.name);
            }
        }
        info!("已实例化工具数: {}", registry.len());
    }

    // 根据传输模式启动
    match args.transport.as_str() {
        "stdio" => run_stdio(registry),
        "http" => run_http(registry, args.port).await,
        "grpc" => run_grpc(registry, args.grpc_port).await,
        other => {
            error!("不支持的传输模式: {}（请使用 stdio、http 或 grpc）", other);
            std::process::exit(1);
        }
    }
}

/// gRPC 模式：启动 tonic gRPC 服务器（tentacle.v1 协议，对接 Anaphase-Helix）
async fn run_grpc(registry: ToolRegistry, port: u16) {
    use tentacle_transport_grpc::{GrpcState, TentacleGrpcService};

    let state = GrpcState::new(registry);
    let service = TentacleGrpcService::new(state).into_server();

    let addr = format!("0.0.0.0:{}", port);
    let addr: std::net::SocketAddr = match addr.parse() {
        Ok(a) => a,
        Err(e) => {
            error!("无效的 gRPC 端口 {}: {}", port, e);
            std::process::exit(1);
        }
    };
    info!("gRPC 服务器监听: {}", addr);
    info!("端点:");
    info!("  ListManifests   — 工具索引");
    info!("  GetManifest     — 工具说明书");
    info!("  ExecuteTool     — 执行工具");
    info!("  ExecuteToolStream — 流式执行");

    if let Err(e) = tonic::transport::Server::builder()
        .add_service(service)
        .serve(addr)
        .await
    {
        error!("gRPC 服务器错误: {}", e);
        std::process::exit(1);
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

/// ProcessTool — 通过外部进程执行工具（临时实现，用于联调验证）
///
/// 生产环境应使用专门的 PluginLoader（支持 WASM/JS 沙箱、资源限制等）。
/// 这个实现直接通过 std::process::Command 执行 executable 脚本。
struct ProcessTool {
    manifest: tentacle_core::Manifest,
    executable_path: std::path::PathBuf,
}

impl ProcessTool {
    fn new(manifest: tentacle_core::Manifest, executable_path: std::path::PathBuf) -> Self {
        Self { manifest, executable_path }
    }
}

impl tentacle_core::Tool for ProcessTool {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn manifest(&self) -> &tentacle_core::Manifest {
        &self.manifest
    }

    fn execute(&self, req: tentacle_core::ExecutionRequest) -> Result<tentacle_core::ToolOutput, tentacle_core::ToolError> {
        use std::process::Command;

        // 根据 executable 扩展名选择执行方式
        let output = if self.executable_path.extension().and_then(|e| e.to_str()) == Some("js") {
            // JS 文件用 node 执行
            Command::new("node")
                .arg(&self.executable_path)
                .arg(&req.tool)
                .arg(serde_json::to_string(&req.params).unwrap_or_default())
                .arg("{}") // server_config 占位
                .output()
        } else {
            // 其他文件直接执行
            Command::new(&self.executable_path)
                .arg(&req.tool)
                .arg(serde_json::to_string(&req.params).unwrap_or_default())
                .output()
        };

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    // 尝试解析 stdout 为 JSON
                    let data = serde_json::from_str(&stdout).unwrap_or(serde_json::json!({ "raw_output": stdout }));
                    Ok(tentacle_core::ToolOutput::success(data))
                } else {
                    Err(tentacle_core::ToolError::ExecutionFailed(format!(
                        "执行失败 (exit code: {:?}): {}",
                        output.status.code(),
                        stderr
                    )))
                }
            }
            Err(e) => Err(tentacle_core::ToolError::ExecutionFailed(format!(
                "无法启动进程: {}",
                e
            ))),
        }
    }
}
