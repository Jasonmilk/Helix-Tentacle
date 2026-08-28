//! tentacle-wasm — Helix-Tentacle WASM 沙箱运行时
//!
//! 白皮书 §6.3 / T05：WASM 工具在 wasmtime 沙箱中运行，
//! 通过 epoch_deadline 机制实现超时强制终止。
//!
//! T3 阶段：纯 WASM 沙箱（不链接 WASI，模块零外部访问）。
//! - WASI stdout 捕获、文件系统权限 → P3 工具集成时实现
//! - 最小权限：模块只能调用自身导出的函数，无法访问网络/文件/环境
//!
//! 核心原则：沙箱即契约，安全可验证。越权即拒绝，超时即终止。
//!
//! 启用方式：cargo build -p tentacle-wasm --features runtime
//! （wasmtime 编译时间长，默认不启用，保持 workspace 轻量）

#![cfg_attr(not(feature = "runtime"), allow(dead_code, unused_imports))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tentacle_core::Manifest;

#[cfg(feature = "runtime")]
use wasmtime::{Engine, Linker, Module, Store, TypedFunc};

/// Epoch 递增间隔（毫秒）
///
/// wasmtime 的 epoch 中断需要外部定期递增 epoch。
/// 每 10ms 递增一次，超时时间 = epoch_ticks * 10ms。
const EPOCH_INTERVAL_MS: u64 = 10;

/// WASM 执行结果
#[derive(Debug, Clone)]
pub struct WasmOutput {
    /// 退出码（WASM 模块 `call` 函数返回值）
    pub exit_code: i32,
}

/// WASM 沙箱错误
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    #[error("WASM 编译失败: {0}")]
    Compile(String),
    #[error("WASM 实例化失败: {0}")]
    Instantiate(String),
    #[error("WASM 执行超时（epoch_deadline 触发）")]
    Timeout,
    #[error("WASM 执行失败: {0}")]
    Execute(String),
    #[error("WASM 模块缺少 `call` 导出函数")]
    MissingEntry,
}

/// WASM 沙箱运行时
///
/// 持有 wasmtime Engine 和 epoch 递增线程。
/// 可复用：一个 WasmSandbox 实例可执行多个 WASM 模块。
///
/// # 安全模型（T3 阶段：纯 WASM，零外部访问）
/// - 不链接 WASI：模块无法访问文件系统、网络、环境变量、标准输出
/// - 模块只能调用自身导出的函数和 WASM 原语
/// - epoch_deadline：超时强制终止，防止无限循环
/// - 内存隔离：每个 Store 独立内存，模块间互不影响
///
/// # 示例
/// ```no_run
/// use tentacle_wasm::WasmSandbox;
///
/// let sandbox = WasmSandbox::new().unwrap();
/// let wasm_bytes = b"\0asm\x01\0\0\0"; // 替换为实际 WASM 字节
/// let manifest = tentacle_core::Manifest::default();
/// let output = sandbox.execute(wasm_bytes, &manifest, 5000).unwrap();
/// assert_eq!(output.exit_code, 42);
/// ```
pub struct WasmSandbox {
    #[cfg(feature = "runtime")]
    engine: Engine,
    /// epoch 递增线程句柄
    epoch_handle: Option<JoinHandle<()>>,
    /// 停止 epoch 线程的标志
    stop_flag: Arc<AtomicBool>,
}

impl WasmSandbox {
    /// 创建 WASM 沙箱运行时
    ///
    /// 启动后台 epoch 递增线程（每 10ms 递增一次），
    /// 用于 epoch_deadline 超时机制。
    pub fn new() -> Result<Self, WasmError> {
        #[cfg(feature = "runtime")]
        {
            let mut config = wasmtime::Config::new();
            config.epoch_interruption(true);
            // 禁用所有 WASI 相关功能（T3 阶段：纯 WASM）
            let engine = Engine::new(&config).map_err(|e| WasmError::Compile(e.to_string()))?;

            // 启动 epoch 递增线程
            let stop_flag = Arc::new(AtomicBool::new(false));
            let engine_clone = engine.clone();
            let stop_clone = stop_flag.clone();
            let epoch_handle = thread::spawn(move || {
                while !stop_clone.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(EPOCH_INTERVAL_MS));
                    engine_clone.increment_epoch();
                }
            });

            Ok(Self {
                engine,
                epoch_handle: Some(epoch_handle),
                stop_flag,
            })
        }

        #[cfg(not(feature = "runtime"))]
        {
            Ok(Self {
                epoch_handle: None,
                stop_flag: Arc::new(AtomicBool::new(false)),
            })
        }
    }

    /// 执行 WASM 模块
    ///
    /// # 参数
    /// - `wasm_bytes`: WASM 二进制或 WAT 文本（wasmtime 自动检测）
    /// - `manifest`: 工具说明书（T3 阶段未使用，预留 P3 权限配置）
    /// - `timeout_ms`: 超时时间（毫秒），epoch_deadline 强制终止
    ///
    /// # 流程
    /// 1. 编译 WASM 模块
    /// 2. 创建 Store（独立内存空间）
    /// 3. 设置 epoch_deadline（超时强制终止）
    /// 4. 实例化模块（不链接 WASI，零外部访问）
    /// 5. 获取 `call` 导出函数并执行
    ///
    /// # 错误
    /// - `Timeout`: epoch_deadline 触发，执行被强制终止
    /// - `MissingEntry`: 模块缺少 `call` 导出函数
    /// - `Compile`/`Instantiate`/`Execute`: WASM 编译/实例化/执行失败
    pub fn execute(
        &self,
        wasm_bytes: &[u8],
        _manifest: &Manifest,
        timeout_ms: u64,
    ) -> Result<WasmOutput, WasmError> {
        #[cfg(feature = "runtime")]
        {
            // 1. 编译模块（支持 WAT 文本自动检测）
            let module = Module::new(&self.engine, wasm_bytes)
                .map_err(|e| WasmError::Compile(e.to_string()))?;

            // 2. 创建 Store（独立内存空间，零状态）
            let mut store = Store::new(&self.engine, ());

            // 3. 设置 epoch_deadline（每 10ms 一个 epoch）
            let epoch_ticks = (timeout_ms / EPOCH_INTERVAL_MS).max(1);
            store.set_epoch_deadline(epoch_ticks);

            // 4. 实例化模块（不链接 WASI，零外部访问）
            //    模块只能调用自身导出的函数，无法访问文件/网络/环境
            let linker = Linker::new(&self.engine);
            let instance = linker
                .instantiate(&mut store, &module)
                .map_err(|e| WasmError::Instantiate(e.to_string()))?;

            // 5. 获取 `call` 导出函数并执行
            let func: TypedFunc<(), i32> = instance
                .get_typed_func(&mut store, "call")
                .map_err(|_| WasmError::MissingEntry)?;

            let result = func.call(&mut store, ()).map_err(|e| {
                // epoch_deadline 触发时，wasmtime 返回 Trap
                // T3 阶段纯 WASM 模块（零外部访问）唯一的 Trap 来源是 epoch 超时
                // 后续 P3 接入 WASI 后需细化 Trap 类型区分
                if e.downcast_ref::<wasmtime::Trap>().is_some() {
                    WasmError::Timeout
                } else {
                    WasmError::Execute(e.to_string())
                }
            })?;

            Ok(WasmOutput { exit_code: result })
        }

        #[cfg(not(feature = "runtime"))]
        {
            let _ = (wasm_bytes, _manifest, timeout_ms);
            Err(WasmError::Compile(
                "runtime feature 未启用，请使用 --features runtime 编译".to_string(),
            ))
        }
    }
}

impl Default for WasmSandbox {
    fn default() -> Self {
        Self::new().expect("WasmSandbox::new 失败")
    }
}

impl Drop for WasmSandbox {
    fn drop(&mut self) {
        // 停止 epoch 递增线程
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.epoch_handle.take() {
            let _ = handle.join();
        }
    }
}

// ===== 测试（仅 runtime feature 启用时编译）=====

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use super::*;

    /// 简单 WASM 模块（WAT 文本）：返回 42
    const SIMPLE_WAT: &str = r#"
(module
  (func (export "call") (result i32)
    i32.const 42
  )
)
"#;

    /// 无限循环 WASM 模块（WAT 文本）：测试超时终止
    const INFINITE_LOOP_WAT: &str = r#"
(module
  (func (export "call") (result i32)
    (loop $loop
      (br $loop)
    )
    i32.const 0
  )
)
"#;

    /// 缺少 `call` 导出的模块
    const NO_ENTRY_WAT: &str = r#"
(module
  (func (export "other") (result i32)
    i32.const 1
  )
)
"#;

    /// 递归调用模块（测试栈溢出保护）
    const RECURSION_WAT: &str = r#"
(module
  (func $recurse (export "call") (result i32)
    call $recurse
  )
)
"#;

    #[test]
    fn test_wasm_simple_execution() {
        let sandbox = WasmSandbox::new().unwrap();
        let manifest = Manifest::default();
        let output = sandbox.execute(SIMPLE_WAT.as_bytes(), &manifest, 5000).unwrap();
        assert_eq!(output.exit_code, 42);
    }

    #[test]
    fn test_wasm_timeout_epoch_deadline() {
        let sandbox = WasmSandbox::new().unwrap();
        let manifest = Manifest::default();
        // 100ms 超时，无限循环应被 epoch_deadline 强制终止
        let result = sandbox.execute(INFINITE_LOOP_WAT.as_bytes(), &manifest, 100);
        match result {
            Err(WasmError::Timeout) => {}
            other => panic!("expected Timeout, got {:?}", other),
        }
    }

    #[test]
    fn test_wasm_missing_entry() {
        let sandbox = WasmSandbox::new().unwrap();
        let manifest = Manifest::default();
        let result = sandbox.execute(NO_ENTRY_WAT.as_bytes(), &manifest, 5000);
        match result {
            Err(WasmError::MissingEntry) => {}
            other => panic!("expected MissingEntry, got {:?}", other),
        }
    }

    #[test]
    fn test_wasm_sandbox_reusable() {
        // 同一个 WasmSandbox 实例可执行多个模块
        let sandbox = WasmSandbox::new().unwrap();
        let manifest = Manifest::default();

        let output1 = sandbox.execute(SIMPLE_WAT.as_bytes(), &manifest, 5000).unwrap();
        assert_eq!(output1.exit_code, 42);

        let output2 = sandbox.execute(SIMPLE_WAT.as_bytes(), &manifest, 5000).unwrap();
        assert_eq!(output2.exit_code, 42);
    }

    #[test]
    fn test_wasm_no_wasi_access() {
        // T3 阶段：不链接 WASI，模块无法访问外部资源
        // 尝试导入 WASI 函数的模块应实例化失败
        let wasi_import_wat = r#"
(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (func (export "call") (result i32)
    i32.const 0
  )
)
"#;
        let sandbox = WasmSandbox::new().unwrap();
        let manifest = Manifest::default();
        let result = sandbox.execute(wasi_import_wat.as_bytes(), &manifest, 5000);
        // 模块导入了 WASI 函数，但 linker 未提供 → 实例化失败
        match result {
            Err(WasmError::Instantiate(_)) => {}
            other => panic!("expected Instantiate error (WASI not linked), got {:?}", other),
        }
    }

    #[test]
    fn test_wasm_recursion_timeout() {
        // 递归调用应被 epoch_deadline 终止（或栈溢出 trap）
        let sandbox = WasmSandbox::new().unwrap();
        let manifest = Manifest::default();
        let result = sandbox.execute(RECURSION_WAT.as_bytes(), &manifest, 100);
        // 递归要么超时（Timeout），要么栈溢出（Execute）
        match result {
            Err(WasmError::Timeout) | Err(WasmError::Execute(_)) => {}
            other => panic!("expected Timeout or Execute (stack overflow), got {:?}", other),
        }
    }
}
