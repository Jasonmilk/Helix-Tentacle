//! tentacle-js — Helix-Tentacle JS 沙箱运行时
//!
//! 白皮书 §6.3 / T06：JS 工具在固定 worker 池中运行 QuickJS 沙箱，
//! 采用协变中断机制（Cooperative Interruption）实现超时控制。
//!
//! P3-T2 升级：从"每次独立线程"升级为"固定 worker 池 + 可复用 Runtime/Context"
//! - worker 池大小默认与物理核数绑定（available_parallelism），可配置
//! - 每个 worker 线程持有 1 个 Runtime + 1 个 Context（可复用，不每次创建）
//! - 任务通过 round-robin 分配给 worker，多 worker 并行利用多核
//! - 每个 worker 串行执行任务（JS 单线程模型），任务间互不影响
//! - 协变中断：每个任务独立的 AtomicBool 取消信号，任务开始时更新中断处理器
//!
//! 核心原则：沙箱即契约，安全可验证。越权即拒绝，超时即终止。
//!
//! 安全模型：
//! - worker 池执行：固定线程数，不阻塞主线程，线程级兜底超时
//! - 协变中断：`set_interrupt_handler` + `AtomicBool`，JS 执行到下一个安全点时检查取消信号
//! - 内存限制：`set_memory_limit`，防止 JS 代码耗尽内存
//! - 危险 API 移除：rquickjs 默认不提供 require/import/文件/网络 API（需显式启用 loader feature）
//!
//! 启用方式：cargo build -p tentacle-js --features runtime
//! （rquickjs 编译时间长，默认不启用，保持 workspace 轻量）

#![cfg_attr(not(feature = "runtime"), allow(dead_code, unused_imports))]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tentacle_core::Manifest;

#[cfg(feature = "runtime")]
use rquickjs::{Context, Runtime};

/// 默认内存限制（50MB）
const DEFAULT_MEMORY_LIMIT: usize = 50 * 1024 * 1024;

/// JS 执行结果
#[derive(Debug, Clone)]
pub struct JsOutput {
    /// JS 代码最后一个表达式的字符串表示
    pub value: String,
}

/// JS 沙箱错误
#[derive(Debug, thiserror::Error)]
pub enum JsError {
    #[error("JS 运行时创建失败: {0}")]
    Runtime(String),
    #[error("JS 上下文创建失败: {0}")]
    Context(String),
    #[error("JS 执行超时（协变中断触发）")]
    Timeout,
    #[error("JS 执行失败: {0}")]
    Execute(String),
    #[error("JS 内存超限")]
    MemoryLimit,
    #[error("JS worker 线程已停止")]
    WorkerStopped,
    #[error("资源限制触发: {0}")]
    ResourceLimit(String),
}

/// 发送给 worker 的任务
#[cfg(feature = "runtime")]
struct JsTask {
    code: String,
    timeout_ms: u64,
    result_tx: mpsc::Sender<Result<JsOutput, JsError>>,
    cancel: Arc<AtomicBool>,
}

/// worker 线程句柄
#[cfg(feature = "runtime")]
struct WorkerHandle {
    task_tx: mpsc::Sender<JsTask>,
    _thread_handle: thread::JoinHandle<()>,
}

/// JS 沙箱运行时（worker 池版本）
///
/// 持有固定数量的 worker 线程，每个线程持有 1 个 Runtime + 1 个 Context（可复用）。
/// 任务通过 round-robin 分配给 worker，多 worker 并行利用多核。
///
/// # 示例
/// ```no_run
/// use tentacle_js::JsSandbox;
///
/// let sandbox = JsSandbox::default();
/// let manifest = tentacle_core::Manifest::default();
/// let output = sandbox.execute("1 + 2", &manifest, 5000).unwrap();
/// assert_eq!(output.value, "3");
/// ```
pub struct JsSandbox {
    #[cfg(feature = "runtime")]
    workers: Vec<WorkerHandle>,
    #[cfg(feature = "runtime")]
    next_worker: AtomicUsize,
    /// 内存限制（字节）
    memory_limit: usize,
    /// worker 池大小
    worker_count: usize,
    /// 默认资源配额（P5-T2：统一资源限制）
    default_quota: tentacle_core::ResourceQuota,
}

impl JsSandbox {
    /// 创建 JS 沙箱（默认 worker 池大小 = 物理核数）
    ///
    /// # 参数
    /// - `memory_limit`: 内存限制（字节），0 表示无限制
    pub fn new(memory_limit: usize) -> Self {
        let worker_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2);
        let mut quota = tentacle_core::ResourceQuota::default();
        quota.memory_limit_bytes = memory_limit;
        Self::with_quota(quota, worker_count)
    }

    /// 创建带自定义资源配额的 JS 沙箱（P5-T2）
    #[cfg(feature = "runtime")]
    pub fn with_quota(quota: tentacle_core::ResourceQuota, worker_count: usize) -> Self {
        {
            let worker_count = worker_count.max(1);
            let mut workers = Vec::with_capacity(worker_count);
            let mem_limit = quota.memory_limit_bytes;

            for _ in 0..worker_count {
                let (task_tx, task_rx) = mpsc::channel::<JsTask>();

                let thread_handle = thread::spawn(move || {
                    // worker 线程：创建 Runtime + Context（可复用）
                    let runtime = match Runtime::new() {
                        Ok(rt) => rt,
                        Err(_) => return, // Runtime 创建失败，worker 线程退出
                    };

                    if mem_limit > 0 {
                        runtime.set_memory_limit(mem_limit);
                    }

                    let context = match Context::full(&runtime) {
                        Ok(ctx) => ctx,
                        Err(_) => return, // Context 创建失败，worker 线程退出
                    };

                    // 循环接收任务
                    while let Ok(task) = task_rx.recv() {
                        // 重置取消信号
                        task.cancel.store(false, Ordering::Relaxed);
    
                        // 更新中断处理器（绑定当前任务的取消信号）
                        let cancel_handler = task.cancel.clone();
                        runtime.set_interrupt_handler(Some(Box::new(move || {
                            cancel_handler.load(Ordering::Relaxed)
                        })));
    
                        // 启动超时定时器线程（分段 sleep + done 标志，任务完成后快速退出）
                        let cancel_timeout = task.cancel.clone();
                        let done = Arc::new(AtomicBool::new(false));
                        let done_timeout = done.clone();
                        let timeout_ms = task.timeout_ms;
                        let timeout_handle = thread::spawn(move || {
                            // 分 10ms 小段 sleep，检查 done 标志
                            let total_ms = timeout_ms;
                            let step_ms = 10u64;
                            let mut elapsed_ms = 0u64;
                            while elapsed_ms < total_ms && !done_timeout.load(Ordering::Relaxed) {
                                thread::sleep(Duration::from_millis(step_ms));
                                elapsed_ms += step_ms;
                            }
                            // 只有任务未完成时才设置取消信号
                            if !done_timeout.load(Ordering::Relaxed) {
                                cancel_timeout.store(true, Ordering::Relaxed);
                            }
                        });
    
                        // 执行 JS 代码（在闭包内直接通过 result_tx 发送结果）
                        let cancel = task.cancel.clone();
                        let result_tx = task.result_tx.clone();
                        context.with(|ctx| {
                            match ctx.eval::<rquickjs::Value, _>(task.code.as_str()) {
                                Ok(value) => {
                                    let string = if let Some(s) = value.as_string() {
                                        s.to_string().unwrap_or_default()
                                    } else if let Some(b) = value.as_bool() {
                                        b.to_string()
                                    } else if let Some(i) = value.as_int() {
                                        i.to_string()
                                    } else if let Some(f) = value.as_float() {
                                        if f.fract() == 0.0 && f.abs() < 1e15 {
                                            format!("{}", f as i64)
                                        } else {
                                            f.to_string()
                                        }
                                    } else if value.is_null() {
                                        "null".to_string()
                                    } else if value.is_undefined() {
                                        "undefined".to_string()
                                    } else {
                                        "[object Object]".to_string()
                                    };
                                    let _ = result_tx.send(Ok(JsOutput { value: string }));
                                }
                                Err(e) => {
                                    let err_msg = e.to_string();
                                    if cancel.load(Ordering::Relaxed) {
                                        let _ = result_tx.send(Err(JsError::Timeout));
                                    } else if err_msg.contains("memory")
                                        || err_msg.contains("Memory")
                                        || err_msg.contains("out of memory")
                                    {
                                        let _ = result_tx.send(Err(JsError::MemoryLimit));
                                    } else {
                                        let _ = result_tx.send(Err(JsError::Execute(err_msg)));
                                    }
                                }
                            }
                        });
    
                        // JS 执行完毕，设置 done 标志（让超时定时器线程快速退出）
                        done.store(true, Ordering::Relaxed);
    
                        // 等待超时定时器结束（最多等待 10ms，避免线程泄漏）
                        let _ = timeout_handle.join();
                    }
    
                    // task_rx 断开（JsSandbox 被 drop），worker 线程退出
                });

                workers.push(WorkerHandle {
                    task_tx,
                    _thread_handle: thread_handle,
                });
            }

            Self {
                workers,
                next_worker: AtomicUsize::new(0),
                memory_limit: quota.memory_limit_bytes,
                worker_count,
                default_quota: quota,
            }
        }
    }

    /// 创建 JS 沙箱（指定 worker 池大小）
    ///
    /// # 参数
    /// - `memory_limit`: 内存限制（字节），0 表示无限制
    /// - `worker_count`: worker 池大小（线程数），至少 1
    #[cfg(feature = "runtime")]
    pub fn with_workers(memory_limit: usize, worker_count: usize) -> Self {
        let mut quota = tentacle_core::ResourceQuota::default();
        quota.memory_limit_bytes = memory_limit;
        Self::with_quota(quota, worker_count)
    }

    /// 创建 JS 沙箱（指定 worker 池大小，无 runtime feature 时的占位实现）
    #[cfg(not(feature = "runtime"))]
    pub fn with_workers(memory_limit: usize, worker_count: usize) -> Self {
        let mut quota = tentacle_core::ResourceQuota::default();
        quota.memory_limit_bytes = memory_limit;
        Self {
            memory_limit,
            worker_count: worker_count.max(1),
            default_quota: quota,
        }
    }

    /// 创建带自定义资源配额的 JS 沙箱（无 runtime feature 时的占位实现，P5-T2）
    #[cfg(not(feature = "runtime"))]
    pub fn with_quota(quota: tentacle_core::ResourceQuota, worker_count: usize) -> Self {
        Self {
            memory_limit: quota.memory_limit_bytes,
            worker_count: worker_count.max(1),
            default_quota: quota,
        }
    }

    /// 执行 JS 代码（round-robin 分配给 worker）
    ///
    /// # 参数
    /// - `js_code`: JS 源代码
    /// - `_manifest`: 工具说明书（预留权限配置）
    /// - `timeout_ms`: 超时时间（毫秒），协变中断 + 线程兜底双重保障
    ///
    /// # 流程
    /// 1. round-robin 选择一个 worker
    /// 2. 创建任务（含取消信号和结果通道）
    /// 3. 发送任务给 worker
    /// 4. 等待结果（兜底超时 = timeout_ms + 1s，防止 worker 卡死）
    ///
    /// # 错误
    /// - `Timeout`: 协变中断触发
    /// - `MemoryLimit`: JS 代码耗尽内存
    /// - `Execute`: JS 语法错误或运行时错误
    /// - `WorkerStopped`: worker 线程已停止（发送失败）
    pub fn execute(
        &self,
        js_code: &str,
        _manifest: &Manifest,
        timeout_ms: u64,
    ) -> Result<JsOutput, JsError> {
        // 使用默认配额，超时时间由参数覆盖
        let mut quota = self.default_quota.clone();
        if timeout_ms > 0 {
            quota.timeout_ms = timeout_ms;
        }
        self.execute_with_quota(js_code, _manifest, &quota)
    }

    /// 执行 JS 代码（带自定义资源配额，P5-T2）
    pub fn execute_with_quota(
        &self,
        js_code: &str,
        _manifest: &Manifest,
        quota: &tentacle_core::ResourceQuota,
    ) -> Result<JsOutput, JsError> {
        #[cfg(feature = "runtime")]
        {
            use tentacle_core::{AtomicResourceLimiter, ResourceLimiter};

            // 启动资源限制器
            let limiter = AtomicResourceLimiter::new(quota.clone());
            limiter.start();

            // round-robin 选择 worker
            let worker_idx = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.workers.len();
            let worker = &self.workers[worker_idx];

            // 创建任务
            let (result_tx, result_rx) = mpsc::channel::<Result<JsOutput, JsError>>();
            let cancel = Arc::new(AtomicBool::new(false));
            let effective_timeout = if quota.timeout_ms > 0 {
                quota.timeout_ms
            } else {
                u64::MAX
            };
            let task = JsTask {
                code: js_code.to_string(),
                timeout_ms: effective_timeout,
                result_tx,
                cancel: cancel.clone(),
            };

            // 发送任务给 worker
            worker.task_tx.send(task).map_err(|_| JsError::WorkerStopped)?;

            // 等待结果（兜底超时 = timeout_ms + 1s）
            let result = match result_rx.recv_timeout(Duration::from_millis(effective_timeout + 1000)) {
                Ok(result) => result,
                Err(_) => {
                    // 兜底超时：设置取消信号，返回 Timeout
                    cancel.store(true, Ordering::Relaxed);
                    Err(JsError::Timeout)
                }
            };

            // 检查输出大小限制（P5-T2）
            if let Ok(ref output) = result {
                let output_len = output.value.len();
                limiter.record_output(output_len);
                if let Err(e) = limiter.check_output(output_len) {
                    return Err(JsError::ResourceLimit(e.to_string()));
                }
            }

            // 停止资源限制器
            let _usage = limiter.stop();

            result
        }

        #[cfg(not(feature = "runtime"))]
        {
            let _ = (js_code, _manifest, quota);
            Err(JsError::Runtime(
                "runtime feature 未启用，请使用 --features runtime 编译".to_string(),
            ))
        }
    }

    /// 获取 worker 池大小
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// 获取内存限制（字节）
    pub fn memory_limit(&self) -> usize {
        self.memory_limit
    }
}

impl Default for JsSandbox {
    fn default() -> Self {
        Self::new(DEFAULT_MEMORY_LIMIT)
    }
}

// ===== 测试（仅 runtime feature 启用时编译）=====

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use super::*;

    #[test]
    fn test_js_simple_execution() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        let output = sandbox.execute("1 + 2", &manifest, 5000).unwrap();
        assert_eq!(output.value, "3");
    }

    #[test]
    fn test_js_string_result() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        let output = sandbox.execute("'hello' + ' ' + 'world'", &manifest, 5000).unwrap();
        assert_eq!(output.value, "hello world");
    }

    #[test]
    fn test_js_object_result() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        let output = sandbox.execute("({a: 1})", &manifest, 5000).unwrap();
        assert_eq!(output.value, "[object Object]");
    }

    #[test]
    fn test_js_timeout_infinite_loop() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        // 100ms 超时，无限循环应被协变中断终止
        let result = sandbox.execute("while(true) {}", &manifest, 100);
        match result {
            Err(JsError::Timeout) => {}
            other => panic!("expected Timeout, got {:?}", other),
        }
    }

    #[test]
    fn test_js_syntax_error() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        let result = sandbox.execute("function {", &manifest, 5000);
        match result {
            Err(JsError::Execute(_)) => {}
            other => panic!("expected Execute error, got {:?}", other),
        }
    }

    #[test]
    fn test_js_no_require_api() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        // require 未定义，调用应报错
        let result = sandbox.execute("require('fs')", &manifest, 5000);
        match result {
            Err(JsError::Execute(_)) => {}
            other => panic!("expected Execute error (require not defined), got {:?}", other),
        }
    }

    #[test]
    fn test_js_no_file_api() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        // readFile 未定义，typeof 返回 "undefined"（不报错）
        let output = sandbox.execute("typeof readFile", &manifest, 5000).unwrap();
        assert_eq!(output.value, "undefined");
    }

    #[test]
    fn test_js_memory_limit() {
        // 1MB 内存限制，大数组分配应触发 MemoryLimit 或 Execute
        let sandbox = JsSandbox::new(1 * 1024 * 1024);
        let manifest = Manifest::default();
        let result = sandbox.execute(
            "let a = []; for(let i=0; i<1000000; i++) a.push(new Array(1000).fill('x')); a.length",
            &manifest,
            5000,
        );
        match result {
            Err(JsError::MemoryLimit) | Err(JsError::Execute(_)) => {}
            other => panic!("expected MemoryLimit or Execute error, got {:?}", other),
        }
    }

    #[test]
    fn test_js_sandbox_reusable() {
        // 同一个 JsSandbox 实例可执行多次（worker 池可复用）
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();

        let output1 = sandbox.execute("1 + 1", &manifest, 5000).unwrap();
        assert_eq!(output1.value, "2");

        let output2 = sandbox.execute("2 + 2", &manifest, 5000).unwrap();
        assert_eq!(output2.value, "4");

        let output3 = sandbox.execute("3 + 3", &manifest, 5000).unwrap();
        assert_eq!(output3.value, "6");
    }

    #[test]
    fn test_js_worker_pool_size() {
        // worker 池大小应 >= 1
        let sandbox = JsSandbox::default();
        assert!(sandbox.worker_count() >= 1);
        assert_eq!(sandbox.memory_limit(), DEFAULT_MEMORY_LIMIT);
    }

    #[test]
    fn test_js_custom_worker_count() {
        // 指定 worker 池大小
        let sandbox = JsSandbox::with_workers(DEFAULT_MEMORY_LIMIT, 4);
        assert_eq!(sandbox.worker_count(), 4);
    }

    #[test]
    fn test_js_concurrent_execution() {
        // 多任务并发执行（worker 池并行处理）
        let sandbox = Arc::new(JsSandbox::with_workers(DEFAULT_MEMORY_LIMIT, 4));
        let manifest = Arc::new(Manifest::default());
        let mut handles = Vec::new();

        for i in 0..10 {
            let sandbox = sandbox.clone();
            let manifest = manifest.clone();
            handles.push(thread::spawn(move || {
                let code = format!("{} * {}", i, i);
                let output = sandbox.execute(&code, &manifest, 5000).unwrap();
                let expected = (i * i).to_string();
                assert_eq!(output.value, expected);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_js_boolean_result() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        let output = sandbox.execute("true", &manifest, 5000).unwrap();
        assert_eq!(output.value, "true");
    }

    #[test]
    fn test_js_null_result() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        let output = sandbox.execute("null", &manifest, 5000).unwrap();
        assert_eq!(output.value, "null");
    }
}
