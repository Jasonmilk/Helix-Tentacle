//! transport_benchmarks — HTTP/gRPC/MCP 三传输层性能基准测试
//!
//! 覆盖：
//! - 单次执行延迟（p50/p95/p99）
//! - 吞吐量（每秒请求数）
//! - 并发性能（10/50/100 并发）
//!
//! 运行方式：cargo bench --package tentacle-benchmarks

use std::sync::Arc;
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tokio::runtime::Runtime;

use tentacle_benchmarks::*;
use tentacle_core::manifest::SecurityLevel;
use tentacle_core::tool::ExecutionRequest;

// ============================================================================
// 核心层基准测试（无传输层开销，作为基线）
// ============================================================================

/// 核心层：工具执行延迟（基线，无传输层开销）
fn bench_core_execute(c: &mut Criterion) {
    let tool = Arc::new(MockTool::new("bench_core", SecurityLevel::Normal));

    let mut group = c.benchmark_group("core_execute");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("direct", |b| {
        b.iter(|| {
            let request = ExecutionRequest {
                tool: "bench_core".to_string(),
                params: serde_json::json!({"input": "core bench"}),
                identity_labels: Default::default(),
                trace_id: Some("core-bench-001".to_string()),
                seen_entropy_bloom: None,
            };
            let _output = tool.execute(request).unwrap();
        });
    });

    group.finish();
}

/// 核心层：注册表查找延迟
fn bench_core_registry(c: &mut Criterion) {
    let registry = create_test_registry(100);

    let mut group = c.benchmark_group("core_registry");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("get_manifest", |b| {
        b.iter(|| {
            let _m = registry.get_manifest("mock_tool_50");
        });
    });

    group.bench_function("index", |b| {
        b.iter(|| {
            let _idx = registry.index();
        });
    });

    group.finish();
}

// ============================================================================
// HTTP 传输层基准测试
// ============================================================================

/// HTTP 传输层：单次执行延迟
fn bench_http_execute(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = create_test_registry(10);
    let state = tentacle_transport_http::AppState::new(registry);
    state.register_tool(Arc::new(MockTool::new("bench_http", SecurityLevel::Normal))).unwrap();

    let app = tentacle_transport_http::router(state);
    let listener = rt.block_on(async {
        tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap()
    });
    let addr = listener.local_addr().unwrap();

    rt.spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // 等待服务器启动
    std::thread::sleep(Duration::from_millis(100));

    let client = reqwest::Client::new();
    let url = format!("http://{}/v1/tools/bench_http/execute", addr);

    let mut group = c.benchmark_group("http_execute");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("single", |b| {
        b.to_async(&rt).iter(|| async {
            let resp = client.post(&url)
                .json(&serde_json::json!({
                    "tool": "bench_http",
                    "params": {"input": "benchmark test"},
                    "trace_id": "bench-001"
                }))
                .send()
                .await
                .unwrap();
            let _body: serde_json::Value = resp.json().await.unwrap();
        });
    });

    group.finish();
}

/// HTTP 传输层：并发吞吐量
fn bench_http_concurrency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = create_test_registry(10);
    let state = tentacle_transport_http::AppState::new(registry);
    state.register_tool(Arc::new(MockTool::new("bench_conc", SecurityLevel::Normal))).unwrap();

    let app = tentacle_transport_http::router(state);
    let listener = rt.block_on(async {
        tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap()
    });
    let addr = listener.local_addr().unwrap();

    rt.spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    std::thread::sleep(Duration::from_millis(100));

    let client = Arc::new(reqwest::Client::new());
    let url = format!("http://{}/v1/tools/bench_conc/execute", addr);

    let mut group = c.benchmark_group("http_concurrency");
    group.measurement_time(Duration::from_secs(15));

    for concurrency in [10, 50, 100].iter() {
        let client = client.clone();
        let url = url.clone();
        group.bench_with_input(BenchmarkId::from_parameter(concurrency), concurrency, |b, &conc| {
            b.to_async(&rt).iter(|| async {
                let mut handles = Vec::with_capacity(conc);
                for i in 0..conc {
                    let client = client.clone();
                    let url = url.clone();
                    handles.push(tokio::spawn(async move {
                        let resp = client.post(&url)
                            .json(&serde_json::json!({
                                "tool": "bench_conc",
                                "params": {"input": format!("conc-{}", i)},
                                "trace_id": format!("req-{}", i)
                            }))
                            .send()
                            .await
                            .unwrap();
                        let _body: serde_json::Value = resp.json().await.unwrap();
                    }));
                }
                for h in handles {
                    h.await.unwrap();
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// gRPC 传输层基准测试
// ============================================================================

/// gRPC 传输层：单次执行延迟
fn bench_grpc_execute(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // 启动 gRPC 服务器
    let registry = create_test_registry(10);
    let state = tentacle_transport_grpc::GrpcState::new(registry);
    rt.block_on(async {
        state.register_tool(Arc::new(MockTool::new("bench_grpc", SecurityLevel::Normal))).await.unwrap();
    });

    let service = tentacle_transport_grpc::TentacleGrpcService::new(state).into_server();
    let listener = rt.block_on(async {
        tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap()
    });
    let addr = listener.local_addr().unwrap();

    rt.spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    std::thread::sleep(Duration::from_millis(200));

    // 创建 gRPC 客户端
    let channel = rt.block_on(async {
        tonic::transport::Endpoint::from_shared(format!("http://{}", addr))
            .unwrap()
            .connect()
            .await
            .unwrap()
    });
    let mut client = tentacle_transport_grpc::proto::tentacle_service_client::TentacleServiceClient::new(channel);

    let mut group = c.benchmark_group("grpc_execute");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("single", |b| {
        b.to_async(&rt).iter(|| async {
            let request = tonic::Request::new(tentacle_transport_grpc::proto::ExecuteToolRequest {
                tool: "bench_grpc".to_string(),
                params: Some(serde_json::json!({"input": "grpc bench"}).to_string()),
                request_id: "grpc-bench-001".to_string(),
                identity_labels: Default::default(),
                timeout_ms: 5000,
            });
            let _response = client.execute_tool(request).await.unwrap();
        });
    });

    group.finish();
}

// ============================================================================
// MCP 传输层基准测试
// ============================================================================

/// MCP 传输层：协议处理延迟（直接调用 handle_message，测量协议解析+执行开销）
fn bench_mcp_protocol(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = create_test_registry(10);
    let server = tentacle_transport_mcp::McpServer::new(registry);
    rt.block_on(async {
        server.register_tool(Arc::new(MockTool::new("bench_mcp", SecurityLevel::Normal))).await.unwrap();
    });

    let server = Arc::new(server);

    // tools/call 请求
    let call_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "bench_mcp",
            "arguments": {
                "input": "mcp benchmark"
            }
        }
    }).to_string();

    let mut group = c.benchmark_group("mcp_protocol");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("tools_call", |b| {
        b.to_async(&rt).iter(|| async {
            let server = server.clone();
            let req = call_request.clone();
            let _response = server.handle_message(&req).await.unwrap();
        });
    });

    // tools/list 请求
    let list_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }).to_string();

    group.bench_function("tools_list", |b| {
        b.to_async(&rt).iter(|| async {
            let server = server.clone();
            let req = list_request.clone();
            let _response = server.handle_message(&req).await.unwrap();
        });
    });

    group.finish();
}

// ============================================================================
// Criterion 组配置
// ============================================================================

criterion_group!(
    benches,
    bench_core_execute,
    bench_core_registry,
    bench_http_execute,
    bench_http_concurrency,
    bench_grpc_execute,
    bench_mcp_protocol,
);
criterion_main!(benches);
