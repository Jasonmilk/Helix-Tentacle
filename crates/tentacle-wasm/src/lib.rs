//! tentacle-wasm — Helix-Tentacle WASM 沙箱运行时
//!
//! 白皮书 §6.3 / T05：WASM 工具在 wasmtime 沙箱中运行，
//! 通过 epoch_deadline 机制实现超时强制终止。
//!
//! P3-T1 阶段：WASI 细化——stdout 捕获到内存，文件系统权限按 Manifest 配置。
//! - stdout：自定义 CaptureStdout（Arc<Mutex<Vec<u8>>>），执行后提取内容
//! - 文件系统：preopened_dir 按 manifest.permissions.filesystem 配置
//! - 最小权限：未声明的目录无法访问，越权即拒绝
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

#[cfg(feature = "runtime")]
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};
#[cfg(feature = "runtime")]
use wasmtime_wasi::pipe::{MemoryInputPipe, MemoryOutputPipe};
#[cfg(feature = "runtime")]
use wasmtime_wasi::preview1::add_to_linker_sync;

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
    /// 标准输出（WASI fd_write 捕获到内存）
    pub stdout: String,
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
    #[error("WASI 预打开目录失败: {0}")]
    PreopenDir(String),
}

/// 解析 Manifest filesystem 权限配置
///
/// 支持格式：
/// - "/tmp" → 只读（DirPerms::READ | FilePerms::READ）
/// - "read:/tmp" → 只读
/// - "write:/tmp" → 读写（DirPerms::READ|MUTATE | FilePerms::READ|WRITE）
/// - "rw:/tmp" → 读写
#[cfg(feature = "runtime")]
fn parse_filesystem_perm(entry: &str) -> (String, DirPerms, FilePerms) {
    let (path, dir_perms, file_perms) = if let Some(rest) = entry.strip_prefix("write:") {
        (
            rest.to_string(),
            DirPerms::READ | DirPerms::MUTATE,
            FilePerms::READ | FilePerms::WRITE,
        )
    } else if let Some(rest) = entry.strip_prefix("rw:") {
        (
            rest.to_string(),
            DirPerms::READ | DirPerms::MUTATE,
            FilePerms::READ | FilePerms::WRITE,
        )
    } else if let Some(rest) = entry.strip_prefix("read:") {
        (rest.to_string(), DirPerms::READ, FilePerms::READ)
    } else {
        // 默认只读
        (entry.to_string(), DirPerms::READ, FilePerms::READ)
    };

    (path, dir_perms, file_perms)
}

/// WASM 沙箱运行时
///
/// 持有 wasmtime Engine 和 epoch 递增线程。
/// 可复用：一个 WasmSandbox 实例可执行多个 WASM 模块。
///
/// # 安全模型（P3-T1：WASI 细化）
/// - stdout 捕获到内存（自定义 CaptureStdout），不泄露到宿主终端
/// - 文件系统：preopened_dir 按 manifest.permissions.filesystem 配置
/// - 未声明的目录无法访问，越权即拒绝
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
/// println!("stdout: {}", output.stdout);
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
            // 启用 WASI 支持（P3-T1：WASI 细化）
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
    /// - `manifest`: 工具说明书（permissions.filesystem 控制文件系统访问）
    /// - `timeout_ms`: 超时时间（毫秒），epoch_deadline 强制终止
    ///
    /// # 流程
    /// 1. 编译 WASM 模块
    /// 2. 构建 WASI 上下文（stdout 捕获 + 文件系统权限）
    /// 3. 创建 Store（data = WasiP1Ctx，独立内存空间）
    /// 4. 设置 epoch_deadline（超时强制终止）
    /// 5. 实例化模块（链接 WASI 函数）
    /// 6. 获取 `call` 导出函数并执行
    /// 7. 提取 stdout 内容
    ///
    /// # 错误
    /// - `Timeout`: epoch_deadline 触发，执行被强制终止
    /// - `MissingEntry`: 模块缺少 `call` 导出函数
    /// - `PreopenDir`: 预打开目录失败（路径不存在或权限不足）
    /// - `Compile`/`Instantiate`/`Execute`: WASM 编译/实例化/执行失败
    pub fn execute(
        &self,
        wasm_bytes: &[u8],
        manifest: &Manifest,
        timeout_ms: u64,
    ) -> Result<WasmOutput, WasmError> {
        #[cfg(feature = "runtime")]
        {
            // 1. 编译模块（支持 WAT 文本自动检测）
            let module = Module::new(&self.engine, wasm_bytes)
                .map_err(|e| WasmError::Compile(e.to_string()))?;

            // 2. 构建 WASI 上下文
            // stdout：MemoryOutputPipe（可 clone，执行后通过 contents() 获取）
            let stdout_pipe = MemoryOutputPipe::new(4096);
            let stdout_handle = stdout_pipe.clone(); // 保存引用，执行后提取内容
            let mut wasi_builder = WasiCtxBuilder::new();
            wasi_builder.stdout(stdout_pipe);
            // stdin：空（WASM 工具不需要交互输入）
            wasi_builder.stdin(MemoryInputPipe::new(bytes::Bytes::new()));
            // stderr：也用 MemoryOutputPipe（暂不单独提取）
            wasi_builder.stderr(MemoryOutputPipe::new(4096));

            // 3. 配置文件系统权限（按 manifest.permissions.filesystem）
            for entry in &manifest.permissions.filesystem {
                let (path, dir_perms, file_perms) = parse_filesystem_perm(entry);
                // guest_path：模块内看到的路径（使用 host_path 的 basename 或完整路径）
                let guest_path = path.clone();
                wasi_builder
                    .preopened_dir(&path, &guest_path, dir_perms, file_perms)
                    .map_err(|e| WasmError::PreopenDir(format!("{}: {}", path, e)))?;
            }

            let wasi = wasi_builder.build_p1();

            // 4. 创建 Store（data = WasiP1Ctx，独立内存空间）
            let mut store = Store::new(&self.engine, wasi);

            // 5. 设置 epoch_deadline（每 10ms 一个 epoch）
            let epoch_ticks = (timeout_ms / EPOCH_INTERVAL_MS).max(1);
            store.set_epoch_deadline(epoch_ticks);

            // 6. 创建 Linker，添加 WASI preview1 函数
            //    Store data = WasiP1Ctx，闭包 |s| s 返回 &mut WasiP1Ctx
            let mut linker = Linker::new(&self.engine);
            add_to_linker_sync(&mut linker, |s| s)
                .map_err(|e| WasmError::Instantiate(format!("WASI linker: {}", e)))?;

            // 7. 实例化模块
            let instance = linker
                .instantiate(&mut store, &module)
                .map_err(|e| WasmError::Instantiate(e.to_string()))?;

            // 8. 获取 `call` 导出函数并执行
            let func: TypedFunc<(), i32> = instance
                .get_typed_func(&mut store, "call")
                .map_err(|_| WasmError::MissingEntry)?;

            let result = func.call(&mut store, ()).map_err(|e| {
                // epoch_deadline 触发时，wasmtime 返回 Trap
                if e.downcast_ref::<wasmtime::Trap>().is_some() {
                    WasmError::Timeout
                } else {
                    WasmError::Execute(e.to_string())
                }
            })?;

            // 9. 提取 stdout 内容（通过 clone 的 MemoryOutputPipe 引用）
            let stdout_bytes = stdout_handle.contents();
            let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();

            Ok(WasmOutput {
                exit_code: result,
                stdout,
            })
        }

        #[cfg(not(feature = "runtime"))]
        {
            let _ = (wasm_bytes, manifest, timeout_ms);
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
    use std::fs;

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

    /// WASI stdout 写入模块（WAT 文本）：调用 fd_write 写入 "hello\n"
    ///
    /// WASI preview1 fd_write(fd, iovs, iovs_len, nwritten)
    /// fd=1 是 stdout
    /// 注意：WAT 字符串中转义字符用 \0a（不是 \n）
    const WASI_STDOUT_WAT: &str = r#"
(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))

  (memory (export "memory") 1)

  ;; 数据段："hello\n" 在偏移 0（\0a 是换行符）
  (data (i32.const 0) "hello\0a")

  ;; iov 结构在偏移 16：{buf=0, buf_len=6}
  ;; nwritten 指针在偏移 24
  (func (export "call") (result i32)
    ;; iov.buf = 0
    (i32.store (i32.const 16) (i32.const 0))
    ;; iov.buf_len = 6
    (i32.store (i32.const 20) (i32.const 6))
    ;; fd_write(1, 16, 1, 24)
    (call $fd_write (i32.const 1) (i32.const 16) (i32.const 1) (i32.const 24))
    drop
    ;; 返回 0（成功）
    i32.const 0
  )
)
"#;

    /// WASI path_open 测试模块（WAT 文本）：尝试打开预打开目录中的文件
    ///
    /// WASI preview1 path_open(dirfd, dirflags, path, path_len, oflags, fs_rights_base, fs_rights_inheriting, fdflags, fd_out) -> errno
    /// 简化版本：只调用 path_open，不读取文件内容
    const WASI_PATH_OPEN_WAT: &str = r#"
(module
  (import "wasi_snapshot_preview1" "path_open" (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))

  (memory (export "memory") 1)

  ;; 文件路径 "test.txt" 在偏移 0
  (data (i32.const 0) "test.txt")

  (func (export "call") (result i32)
    ;; path_open(dirfd=3, dirflags=0, path=0, path_len=8, oflags=0, rights_base=0, rights_inheriting=0, fdflags=0, fd_out=100)
    (call $path_open
      (i32.const 3) (i32.const 0) (i32.const 0) (i32.const 8)
      (i32.const 0) (i64.const 0) (i64.const 0) (i32.const 0) (i32.const 100))
    ;; 返回 errno（0=成功，非0=失败）
  )
)
"#;

    #[test]
    fn test_wasm_simple_execution() {
        let sandbox = WasmSandbox::new().unwrap();
        let manifest = Manifest::default();
        let output = sandbox.execute(SIMPLE_WAT.as_bytes(), &manifest, 5000).unwrap();
        assert_eq!(output.exit_code, 42);
        assert!(output.stdout.is_empty()); // 简单模块不写 stdout
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
    fn test_wasm_wasi_stdout_capture() {
        // P3-T1：WASI stdout 写入应被捕获到内存，不泄露到宿主终端
        let sandbox = WasmSandbox::new().unwrap();
        let manifest = Manifest::default();
        let output = sandbox
            .execute(WASI_STDOUT_WAT.as_bytes(), &manifest, 5000)
            .unwrap();
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "hello\n");
    }

    #[test]
    fn test_wasm_wasi_file_read_with_permission() {
        // P3-T1：预打开目录后，模块可以打开目录中的文件
        let sandbox = WasmSandbox::new().unwrap();

        // 创建临时目录和测试文件
        let temp_dir = std::env::temp_dir().join("tentacle_wasm_test_");
        fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("test.txt");
        fs::write(&test_file, "file content here").unwrap();

        // 配置 Manifest：预打开 temp_dir（只读）
        let mut manifest = Manifest::default();
        manifest
            .permissions
            .filesystem
            .push(format!("read:{}", temp_dir.to_string_lossy()));

        let output = sandbox
            .execute(WASI_PATH_OPEN_WAT.as_bytes(), &manifest, 5000)
            .unwrap();

        // path_open 成功，返回 errno=0
        assert_eq!(output.exit_code, 0, "expected errno=0 (success), got {}", output.exit_code);

        // 清理
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_wasm_wasi_file_read_without_permission() {
        // P3-T1：未预打开目录时，模块无法访问文件系统
        let sandbox = WasmSandbox::new().unwrap();

        // 创建临时目录和测试文件（但不在 Manifest 中声明权限）
        let temp_dir = std::env::temp_dir().join("tentacle_wasm_test_noperm_");
        fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("test.txt");
        fs::write(&test_file, "secret content").unwrap();

        // Manifest 不声明 filesystem 权限（默认无权限）
        let manifest = Manifest::default();

        let output = sandbox
            .execute(WASI_PATH_OPEN_WAT.as_bytes(), &manifest, 5000)
            .unwrap();

        // 未预打开目录，dirfd=3 不存在，path_open 应返回非 0 errno
        assert_ne!(output.exit_code, 0, "expected non-zero errno (path_open failed), got 0");

        // 清理
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_wasm_wasi_preopen_nonexistent_dir() {
        // P3-T1：预打开不存在的目录应返回 PreopenDir 错误
        let sandbox = WasmSandbox::new().unwrap();
        let mut manifest = Manifest::default();
        manifest
            .permissions
            .filesystem
            .push("read:/nonexistent/path/12345".to_string());

        let result = sandbox.execute(SIMPLE_WAT.as_bytes(), &manifest, 5000);
        match result {
            Err(WasmError::PreopenDir(_)) => {}
            other => panic!("expected PreopenDir error, got {:?}", other),
        }
    }

    #[test]
    fn test_wasm_parse_filesystem_perm() {
        // 测试权限解析函数
        let (path, dir_perms, file_perms) = parse_filesystem_perm("/tmp");
        assert_eq!(path, "/tmp");
        assert!(dir_perms.contains(DirPerms::READ));
        assert!(!dir_perms.contains(DirPerms::MUTATE));
        assert!(file_perms.contains(FilePerms::READ));
        assert!(!file_perms.contains(FilePerms::WRITE));

        let (path, dir_perms, file_perms) = parse_filesystem_perm("write:/tmp");
        assert_eq!(path, "/tmp");
        assert!(dir_perms.contains(DirPerms::READ));
        assert!(dir_perms.contains(DirPerms::MUTATE));
        assert!(file_perms.contains(FilePerms::READ));
        assert!(file_perms.contains(FilePerms::WRITE));

        let (path, dir_perms, file_perms) = parse_filesystem_perm("rw:/tmp");
        assert_eq!(path, "/tmp");
        assert!(dir_perms.contains(DirPerms::MUTATE));
        assert!(file_perms.contains(FilePerms::WRITE));

        let (path, _, _) = parse_filesystem_perm("read:/data");
        assert_eq!(path, "/data");
    }
}
