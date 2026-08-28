//! tentacle-js — Helix-Tentacle JS 沙箱运行时
//!
//! 白皮书 §6.3 / T06：JS 工具在独立 OS 线程中运行 QuickJS 沙箱，
//! 采用协变中断机制（Cooperative Interruption）实现超时控制。
//!
//! 核心原则：沙箱即契约，安全可验证。越权即拒绝，超时即终止。
//!
//! 安全模型：
//! - 独立线程执行：不阻塞主线程，线程级兜底超时
//! - 协变中断：`set_interrupt_handler` + `AtomicBool`，JS 执行到下一个安全点时检查取消信号
//! - 内存限制：`set_memory_limit`，防止 JS 代码耗尽内存
//! - 危险 API 移除：rquickjs 默认不提供 require/import/文件/网络 API（需显式启用 loader feature）
//!
//! 启用方式：cargo build -p tentacle-js --features runtime
//! （rquickjs 编译时间长，默认不启用，保持 workspace 轻量）

#![cfg_attr(not(feature = "runtime"), allow(dead_code, unused_imports))]

use std::sync::atomic::{AtomicBool, Ordering};
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
}

/// JS 沙箱运行时
///
/// 持有配置（内存限制），每次执行在独立线程中创建 Runtime + Context。
/// 符合"按需加载，执行即焚"原则：执行完毕后 Runtime/Context 立即销毁。
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
    /// 内存限制（字节）
    memory_limit: usize,
}

impl JsSandbox {
    /// 创建 JS 沙箱
    ///
    /// # 参数
    /// - `memory_limit`: 内存限制（字节），0 表示无限制
    pub fn new(memory_limit: usize) -> Self {
        Self { memory_limit }
    }

    /// 执行 JS 代码
    ///
    /// # 参数
    /// - `js_code`: JS 源代码
    /// - `_manifest`: 工具说明书（T4 阶段未使用，预留 P3 权限配置）
    /// - `timeout_ms`: 超时时间（毫秒），协变中断 + 线程兜底双重保障
    ///
    /// # 流程
    /// 1. 创建独立线程（不阻塞主线程）
    /// 2. 创建 QuickJS Runtime，设置内存限制
    /// 3. 设置协变中断处理器（`AtomicBool` 取消信号）
    /// 4. 创建 Context（含标准库，不含 require/import/文件/网络）
    /// 5. 启动超时定时器线程：到达超时后设置取消信号
    /// 6. 执行 JS 代码，获取最后一个表达式的字符串值
    /// 7. 通过 channel 返回结果
    ///
    /// # 错误
    /// - `Timeout`: 协变中断触发（JS 执行到安全点时检测到取消信号）
    /// - `MemoryLimit`: JS 代码耗尽内存（`set_memory_limit` 触发）
    /// - `Execute`: JS 语法错误或运行时错误
    /// - `Runtime`/`Context`: QuickJS 初始化失败
    pub fn execute(
        &self,
        js_code: &str,
        _manifest: &Manifest,
        timeout_ms: u64,
    ) -> Result<JsOutput, JsError> {
        #[cfg(feature = "runtime")]
        {
            let js_code = js_code.to_string();
            let memory_limit = self.memory_limit;

            // 结果通道
            let (tx, rx) = mpsc::channel::<Result<JsOutput, JsError>>();

            // 独立线程执行 JS
            let _handle = thread::spawn(move || {
                // 1. 创建 Runtime
                let runtime = match Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = tx.send(Err(JsError::Runtime(e.to_string())));
                        return;
                    }
                };

                // 2. 设置内存限制
                runtime.set_memory_limit(memory_limit);

                // 3. 协变中断：AtomicBool 取消信号
                let cancel = Arc::new(AtomicBool::new(false));
                let cancel_handler = cancel.clone();
                runtime.set_interrupt_handler(Some(Box::new(move || {
                    // 返回 true 时中断 JS 执行
                    cancel_handler.load(Ordering::Relaxed)
                })));

                // 4. 创建 Context（full 含标准库，不含 require/import/文件/网络）
                let context = match Context::full(&runtime) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        let _ = tx.send(Err(JsError::Context(e.to_string())));
                        return;
                    }
                };

                // 5. 超时定时器线程：到达超时后设置取消信号
                let cancel_timeout = cancel.clone();
                let timeout_handle = thread::spawn(move || {
                    thread::sleep(Duration::from_millis(timeout_ms));
                    cancel_timeout.store(true, Ordering::Relaxed);
                });

                // 6. 执行 JS 代码（在闭包内直接通过 tx 发送结果，闭包返回 ()，
                //    避免 Result<Value> 生命周期问题导致挂起）
                context.with(|ctx| {
                    match ctx.eval::<rquickjs::Value, _>(js_code.as_str()) {
                        Ok(value) => {
                            // 按类型转换为字符串
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
                            let _ = tx.send(Ok(JsOutput { value: string }));
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            // 区分错误类型（在闭包内判断，避免闭包外 cancel 状态变化）
                            if cancel.load(Ordering::Relaxed) {
                                let _ = tx.send(Err(JsError::Timeout));
                            } else if err_msg.contains("memory") || err_msg.contains("Memory") || err_msg.contains("out of memory") {
                                let _ = tx.send(Err(JsError::MemoryLimit));
                            } else {
                                let _ = tx.send(Err(JsError::Execute(err_msg)));
                            }
                        }
                    }
                });

                // 等待超时定时器结束（避免线程泄漏）
                let _ = timeout_handle.join();

                // 结果已在 context.with 闭包内通过 tx 发送
            });

            // 8. 等待结果（兜底超时 = timeout_ms + 1s，防止线程卡死）
            match rx.recv_timeout(Duration::from_millis(timeout_ms + 1000)) {
                Ok(result) => result,
                Err(_) => Err(JsError::Timeout),
            }
        }

        #[cfg(not(feature = "runtime"))]
        {
            let _ = (js_code, _manifest, timeout_ms);
            Err(JsError::Runtime(
                "runtime feature 未启用，请使用 --features runtime 编译".to_string(),
            ))
        }
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
        // 对象 toString 返回 [object Object]
        let output = sandbox.execute("({a: 1})", &manifest, 5000).unwrap();
        assert!(output.value.contains("object"));
    }

    #[test]
    fn test_js_timeout_infinite_loop() {
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        // 无限循环，100ms 超时
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
        let result = sandbox.execute("function broken( {", &manifest, 5000);
        match result {
            Err(JsError::Execute(_)) => {}
            other => panic!("expected Execute (syntax error), got {:?}", other),
        }
    }

    #[test]
    fn test_js_no_require_api() {
        // rquickjs 默认不提供 require/import，调用应报错
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        let result = sandbox.execute("require('fs')", &manifest, 5000);
        match result {
            Err(JsError::Execute(_)) => {}
            other => panic!("expected Execute (require not defined), got {:?}", other),
        }
    }

    #[test]
    fn test_js_no_file_api() {
        // rquickjs 默认不提供文件系统 API
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();
        let result = sandbox.execute("typeof readFile", &manifest, 5000);
        // readFile 未定义，typeof 返回 "undefined"（不报错）
        match result {
            Ok(output) => assert_eq!(output.value, "undefined"),
            Err(e) => panic!("expected Ok, got {:?}", e),
        }
    }

    #[test]
    fn test_js_memory_limit() {
        // 小内存限制 + 大数组分配 → 内存超限
        let sandbox = JsSandbox::new(1024 * 1024); // 1MB
        let manifest = Manifest::default();
        let result = sandbox.execute(
            "var a = []; for(var i=0; i<1000000; i++) a.push(new Array(1000));",
            &manifest,
            5000,
        );
        // 可能是 MemoryLimit 或 Execute（QuickJS 内存错误信息可能不同）
        match result {
            Err(JsError::MemoryLimit) | Err(JsError::Execute(_)) => {}
            other => panic!("expected MemoryLimit or Execute, got {:?}", other),
        }
    }

    #[test]
    fn test_js_sandbox_reusable() {
        // 同一个 JsSandbox 实例可执行多次（每次独立线程 + Runtime）
        let sandbox = JsSandbox::default();
        let manifest = Manifest::default();

        let output1 = sandbox.execute("1 + 1", &manifest, 5000).unwrap();
        assert_eq!(output1.value, "2");

        let output2 = sandbox.execute("2 + 2", &manifest, 5000).unwrap();
        assert_eq!(output2.value, "4");
    }
}
