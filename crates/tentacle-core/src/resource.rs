//! tentacle-core — 统一资源限制模块
//!
//! P5-T2：为 WASM/JS 沙箱提供统一的内存/CPU/文件描述符配额管理。
//!
//! 设计原则：
//! - 极致解耦：ResourceLimiter trait 与具体沙箱实现分离
//! - 确定性优先：配额检查是固定路径，无分支判断
//! - 物理事实优先：基于实际资源使用量判断，非估算
//! - 渐进生长：当前软件实现用计数器模拟，未来可接入 cgroups/namespace

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ============================================================================
// 资源配额定义
// ============================================================================

/// 资源配额（Resource Quota）
///
/// 定义沙箱执行的资源上限。0 表示无限制。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// 内存限制（字节），0 表示无限制
    #[serde(default)]
    pub memory_limit_bytes: usize,
    /// CPU 时间限制（毫秒），0 表示无限制
    #[serde(default)]
    pub cpu_time_limit_ms: u64,
    /// 文件描述符限制（数量），0 表示无限制
    #[serde(default)]
    pub fd_limit: usize,
    /// 执行超时（毫秒），0 表示无限制
    #[serde(default)]
    pub timeout_ms: u64,
    /// 输出大小限制（字节），0 表示无限制
    #[serde(default)]
    pub output_limit_bytes: usize,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            memory_limit_bytes: 50 * 1024 * 1024, // 50MB
            cpu_time_limit_ms: 5000,                 // 5秒
            fd_limit: 64,
            timeout_ms: 10000, // 10秒
            output_limit_bytes: 1024 * 1024, // 1MB
        }
    }
}

impl ResourceQuota {
    /// 创建无限制配额（用于调试或可信工具）
    pub fn unlimited() -> Self {
        Self {
            memory_limit_bytes: 0,
            cpu_time_limit_ms: 0,
            fd_limit: 0,
            timeout_ms: 0,
            output_limit_bytes: 0,
        }
    }

    /// 创建轻量级配额（用于简单工具）
    pub fn lightweight() -> Self {
        Self {
            memory_limit_bytes: 16 * 1024 * 1024, // 16MB
            cpu_time_limit_ms: 1000,                // 1秒
            fd_limit: 16,
            timeout_ms: 3000, // 3秒
            output_limit_bytes: 256 * 1024, // 256KB
        }
    }

    /// 创建重量级配额（用于复杂工具）
    pub fn heavyweight() -> Self {
        Self {
            memory_limit_bytes: 256 * 1024 * 1024, // 256MB
            cpu_time_limit_ms: 30000,                 // 30秒
            fd_limit: 256,
            timeout_ms: 60000, // 60秒
            output_limit_bytes: 10 * 1024 * 1024, // 10MB
        }
    }

    /// 检查内存是否超限
    pub fn check_memory(&self, current_bytes: usize) -> bool {
        self.memory_limit_bytes == 0 || current_bytes <= self.memory_limit_bytes
    }

    /// 检查 CPU 时间是否超限
    pub fn check_cpu_time(&self, elapsed_ms: u64) -> bool {
        self.cpu_time_limit_ms == 0 || elapsed_ms <= self.cpu_time_limit_ms
    }

    /// 检查文件描述符是否超限
    pub fn check_fd(&self, current_count: usize) -> bool {
        self.fd_limit == 0 || current_count <= self.fd_limit
    }

    /// 检查超时
    pub fn check_timeout(&self, elapsed_ms: u64) -> bool {
        self.timeout_ms == 0 || elapsed_ms <= self.timeout_ms
    }

    /// 检查输出大小是否超限
    pub fn check_output(&self, current_bytes: usize) -> bool {
        self.output_limit_bytes == 0 || current_bytes <= self.output_limit_bytes
    }
}

// ============================================================================
// 资源使用统计
// ============================================================================

/// 资源使用统计（Resource Usage）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// 当前内存使用（字节）
    pub memory_used_bytes: usize,
    /// 峰值内存使用（字节）
    pub memory_peak_bytes: usize,
    /// CPU 时间使用（毫秒）
    pub cpu_time_used_ms: u64,
    /// 文件描述符使用数量
    pub fd_used: usize,
    /// 输出大小（字节）
    pub output_bytes: usize,
    /// 执行耗时（毫秒）
    pub elapsed_ms: u64,
}

impl ResourceUsage {
    /// 更新内存使用并记录峰值
    pub fn update_memory(&mut self, current_bytes: usize) {
        self.memory_used_bytes = current_bytes;
        if current_bytes > self.memory_peak_bytes {
            self.memory_peak_bytes = current_bytes;
        }
    }

    /// 计算资源使用率（0.0 - 1.0）
    pub fn usage_ratio(&self, quota: &ResourceQuota) -> f64 {
        let mut max_ratio = 0.0_f64;
        if quota.memory_limit_bytes > 0 {
            max_ratio = max_ratio.max(self.memory_used_bytes as f64 / quota.memory_limit_bytes as f64);
        }
        if quota.cpu_time_limit_ms > 0 {
            max_ratio = max_ratio.max(self.cpu_time_used_ms as f64 / quota.cpu_time_limit_ms as f64);
        }
        if quota.fd_limit > 0 {
            max_ratio = max_ratio.max(self.fd_used as f64 / quota.fd_limit as f64);
        }
        if quota.timeout_ms > 0 {
            max_ratio = max_ratio.max(self.elapsed_ms as f64 / quota.timeout_ms as f64);
        }
        max_ratio
    }
}

// ============================================================================
// 资源限制错误
// ============================================================================

/// 资源限制错误类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceLimitError {
    /// 内存超限
    MemoryLimitExceeded { current: usize, limit: usize },
    /// CPU 时间超限
    CpuTimeLimitExceeded { current_ms: u64, limit_ms: u64 },
    /// 文件描述符超限
    FdLimitExceeded { current: usize, limit: usize },
    /// 执行超时
    Timeout { elapsed_ms: u64, limit_ms: u64 },
    /// 输出大小超限
    OutputLimitExceeded { current: usize, limit: usize },
}

impl std::fmt::Display for ResourceLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MemoryLimitExceeded { current, limit } => {
                write!(f, "memory limit exceeded: {} bytes > {} bytes limit", current, limit)
            }
            Self::CpuTimeLimitExceeded { current_ms, limit_ms } => {
                write!(f, "CPU time limit exceeded: {}ms > {}ms limit", current_ms, limit_ms)
            }
            Self::FdLimitExceeded { current, limit } => {
                write!(f, "file descriptor limit exceeded: {} > {} limit", current, limit)
            }
            Self::Timeout { elapsed_ms, limit_ms } => {
                write!(f, "execution timeout: {}ms > {}ms limit", elapsed_ms, limit_ms)
            }
            Self::OutputLimitExceeded { current, limit } => {
                write!(f, "output limit exceeded: {} bytes > {} bytes limit", current, limit)
            }
        }
    }
}

impl std::error::Error for ResourceLimitError {}

// ============================================================================
// 资源限制器 trait
// ============================================================================

/// 资源限制器 trait（Resource Limiter）
///
/// 所有沙箱实现都应使用此 trait 进行资源配额管理。
pub trait ResourceLimiter: Send + Sync {
    /// 获取配额
    fn quota(&self) -> &ResourceQuota;

    /// 获取当前使用量
    fn usage(&self) -> ResourceUsage;

    /// 开始执行（记录开始时间）
    fn start(&self);

    /// 检查所有资源是否在配额内，返回第一个超限错误
    fn check_all(&self) -> Result<(), ResourceLimitError>;

    /// 检查内存
    fn check_memory(&self, current_bytes: usize) -> Result<(), ResourceLimitError>;

    /// 检查文件描述符
    fn check_fd(&self, current_count: usize) -> Result<(), ResourceLimitError>;

    /// 检查输出大小
    fn check_output(&self, current_bytes: usize) -> Result<(), ResourceLimitError>;

    /// 记录内存使用
    fn record_memory(&self, bytes: usize);

    /// 记录文件描述符使用
    fn record_fd(&self, count: usize);

    /// 记录输出大小
    fn record_output(&self, bytes: usize);

    /// 停止执行（记录结束时间）
    fn stop(&self) -> ResourceUsage;
}

// ============================================================================
// 原子资源限制器（默认实现）
// ============================================================================

/// 原子资源限制器（Atomic Resource Limiter）
///
/// 使用原子计数器实现线程安全的资源配额管理，适用于多线程沙箱环境。
#[derive(Debug)]
pub struct AtomicResourceLimiter {
    quota: ResourceQuota,
    memory_used: AtomicUsize,
    memory_peak: AtomicUsize,
    fd_used: AtomicUsize,
    output_bytes: AtomicUsize,
    cpu_time_ms: AtomicU64,
    start_time: Mutex<Option<Instant>>,
}

// 简单的 Mutex 封装（避免引入 parking_lot 依赖）
use std::sync::Mutex;

impl AtomicResourceLimiter {
    /// 创建新的资源限制器
    pub fn new(quota: ResourceQuota) -> Arc<Self> {
        Arc::new(Self {
            quota,
            memory_used: AtomicUsize::new(0),
            memory_peak: AtomicUsize::new(0),
            fd_used: AtomicUsize::new(0),
            output_bytes: AtomicUsize::new(0),
            cpu_time_ms: AtomicU64::new(0),
            start_time: Mutex::new(None),
        })
    }

    /// 获取已用时间（毫秒）
    fn elapsed_ms(&self) -> u64 {
        if let Ok(guard) = self.start_time.lock() {
            if let Some(start) = *guard {
                return start.elapsed().as_millis() as u64;
            }
        }
        0
    }
}

impl ResourceLimiter for AtomicResourceLimiter {
    fn quota(&self) -> &ResourceQuota {
        &self.quota
    }

    fn usage(&self) -> ResourceUsage {
        ResourceUsage {
            memory_used_bytes: self.memory_used.load(Ordering::Relaxed),
            memory_peak_bytes: self.memory_peak.load(Ordering::Relaxed),
            cpu_time_used_ms: self.cpu_time_ms.load(Ordering::Relaxed),
            fd_used: self.fd_used.load(Ordering::Relaxed),
            output_bytes: self.output_bytes.load(Ordering::Relaxed),
            elapsed_ms: self.elapsed_ms(),
        }
    }

    fn start(&self) {
        if let Ok(mut guard) = self.start_time.lock() {
            *guard = Some(Instant::now());
        }
    }

    fn check_all(&self) -> Result<(), ResourceLimitError> {
        let elapsed = self.elapsed_ms();

        // 检查超时
        if !self.quota.check_timeout(elapsed) {
            return Err(ResourceLimitError::Timeout {
                elapsed_ms: elapsed,
                limit_ms: self.quota.timeout_ms,
            });
        }

        // 检查 CPU 时间
        let cpu_used = self.cpu_time_ms.load(Ordering::Relaxed);
        if !self.quota.check_cpu_time(cpu_used) {
            return Err(ResourceLimitError::CpuTimeLimitExceeded {
                current_ms: cpu_used,
                limit_ms: self.quota.cpu_time_limit_ms,
            });
        }

        // 检查内存
        let mem_used = self.memory_used.load(Ordering::Relaxed);
        if !self.quota.check_memory(mem_used) {
            return Err(ResourceLimitError::MemoryLimitExceeded {
                current: mem_used,
                limit: self.quota.memory_limit_bytes,
            });
        }

        // 检查文件描述符
        let fd_used = self.fd_used.load(Ordering::Relaxed);
        if !self.quota.check_fd(fd_used) {
            return Err(ResourceLimitError::FdLimitExceeded {
                current: fd_used,
                limit: self.quota.fd_limit,
            });
        }

        // 检查输出大小
        let output_used = self.output_bytes.load(Ordering::Relaxed);
        if !self.quota.check_output(output_used) {
            return Err(ResourceLimitError::OutputLimitExceeded {
                current: output_used,
                limit: self.quota.output_limit_bytes,
            });
        }

        Ok(())
    }

    fn check_memory(&self, current_bytes: usize) -> Result<(), ResourceLimitError> {
        if !self.quota.check_memory(current_bytes) {
            Err(ResourceLimitError::MemoryLimitExceeded {
                current: current_bytes,
                limit: self.quota.memory_limit_bytes,
            })
        } else {
            Ok(())
        }
    }

    fn check_fd(&self, current_count: usize) -> Result<(), ResourceLimitError> {
        if !self.quota.check_fd(current_count) {
            Err(ResourceLimitError::FdLimitExceeded {
                current: current_count,
                limit: self.quota.fd_limit,
            })
        } else {
            Ok(())
        }
    }

    fn check_output(&self, current_bytes: usize) -> Result<(), ResourceLimitError> {
        if !self.quota.check_output(current_bytes) {
            Err(ResourceLimitError::OutputLimitExceeded {
                current: current_bytes,
                limit: self.quota.output_limit_bytes,
            })
        } else {
            Ok(())
        }
    }

    fn record_memory(&self, bytes: usize) {
        self.memory_used.store(bytes, Ordering::Relaxed);
        // 更新峰值
        let mut peak = self.memory_peak.load(Ordering::Relaxed);
        while bytes > peak {
            match self.memory_peak.compare_exchange_weak(
                peak,
                bytes,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => peak = current,
            }
        }
    }

    fn record_fd(&self, count: usize) {
        self.fd_used.store(count, Ordering::Relaxed);
    }

    fn record_output(&self, bytes: usize) {
        self.output_bytes.store(bytes, Ordering::Relaxed);
    }

    fn stop(&self) -> ResourceUsage {
        let usage = self.usage();
        if let Ok(mut guard) = self.start_time.lock() {
            *guard = None;
        }
        usage
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_quota_default() {
        let quota = ResourceQuota::default();
        assert_eq!(quota.memory_limit_bytes, 50 * 1024 * 1024);
        assert_eq!(quota.cpu_time_limit_ms, 5000);
        assert_eq!(quota.fd_limit, 64);
        assert_eq!(quota.timeout_ms, 10000);
        assert_eq!(quota.output_limit_bytes, 1024 * 1024);
    }

    #[test]
    fn test_resource_quota_unlimited() {
        let quota = ResourceQuota::unlimited();
        assert_eq!(quota.memory_limit_bytes, 0);
        assert!(quota.check_memory(usize::MAX));
        assert!(quota.check_cpu_time(u64::MAX));
        assert!(quota.check_fd(usize::MAX));
        assert!(quota.check_timeout(u64::MAX));
        assert!(quota.check_output(usize::MAX));
    }

    #[test]
    fn test_resource_quota_lightweight() {
        let quota = ResourceQuota::lightweight();
        assert_eq!(quota.memory_limit_bytes, 16 * 1024 * 1024);
        assert_eq!(quota.cpu_time_limit_ms, 1000);
        assert_eq!(quota.fd_limit, 16);
    }

    #[test]
    fn test_resource_quota_heavyweight() {
        let quota = ResourceQuota::heavyweight();
        assert_eq!(quota.memory_limit_bytes, 256 * 1024 * 1024);
        assert_eq!(quota.cpu_time_limit_ms, 30000);
        assert_eq!(quota.fd_limit, 256);
    }

    #[test]
    fn test_resource_quota_check_memory() {
        let quota = ResourceQuota {
            memory_limit_bytes: 100,
            ..Default::default()
        };
        assert!(quota.check_memory(50));
        assert!(quota.check_memory(100));
        assert!(!quota.check_memory(101));
    }

    #[test]
    fn test_resource_usage_update_memory() {
        let mut usage = ResourceUsage::default();
        usage.update_memory(50);
        assert_eq!(usage.memory_used_bytes, 50);
        assert_eq!(usage.memory_peak_bytes, 50);
        usage.update_memory(30);
        assert_eq!(usage.memory_used_bytes, 30);
        assert_eq!(usage.memory_peak_bytes, 50); // 峰值保持
    }

    #[test]
    fn test_resource_usage_ratio() {
        let quota = ResourceQuota {
            memory_limit_bytes: 100,
            cpu_time_limit_ms: 1000,
            ..Default::default()
        };
        let usage = ResourceUsage {
            memory_used_bytes: 50,
            cpu_time_used_ms: 200,
            ..Default::default()
        };
        let ratio = usage.usage_ratio(&quota);
        assert!((ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_resource_limit_error_display() {
        let err = ResourceLimitError::MemoryLimitExceeded {
            current: 200,
            limit: 100,
        };
        assert!(err.to_string().contains("memory limit exceeded"));
    }

    #[test]
    fn test_atomic_resource_limiter_basic() {
        let quota = ResourceQuota {
            memory_limit_bytes: 1024,
            fd_limit: 10,
            output_limit_bytes: 512,
            ..Default::default()
        };
        let limiter = AtomicResourceLimiter::new(quota);

        limiter.start();
        limiter.record_memory(512);
        limiter.record_fd(5);
        limiter.record_output(256);

        let usage = limiter.usage();
        assert_eq!(usage.memory_used_bytes, 512);
        assert_eq!(usage.memory_peak_bytes, 512);
        assert_eq!(usage.fd_used, 5);
        assert_eq!(usage.output_bytes, 256);

        assert!(limiter.check_all().is_ok());
        let final_usage = limiter.stop();
        assert!(final_usage.elapsed_ms >= 0);
    }

    #[test]
    fn test_atomic_resource_limiter_memory_exceeded() {
        let quota = ResourceQuota {
            memory_limit_bytes: 100,
            ..Default::default()
        };
        let limiter = AtomicResourceLimiter::new(quota);
        limiter.start();
        limiter.record_memory(200);

        let result = limiter.check_all();
        assert!(result.is_err());
        match result.unwrap_err() {
            ResourceLimitError::MemoryLimitExceeded { current, limit } => {
                assert_eq!(current, 200);
                assert_eq!(limit, 100);
            }
            _ => panic!("expected MemoryLimitExceeded"),
        }
    }

    #[test]
    fn test_atomic_resource_limiter_fd_exceeded() {
        let quota = ResourceQuota {
            fd_limit: 5,
            ..Default::default()
        };
        let limiter = AtomicResourceLimiter::new(quota);
        limiter.start();
        limiter.record_fd(10);

        let result = limiter.check_all();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ResourceLimitError::FdLimitExceeded { .. }
        ));
    }

    #[test]
    fn test_atomic_resource_limiter_memory_peak() {
        let quota = ResourceQuota::unlimited();
        let limiter = AtomicResourceLimiter::new(quota);
        limiter.record_memory(100);
        limiter.record_memory(50);
        limiter.record_memory(200);
        limiter.record_memory(150);

        let usage = limiter.usage();
        assert_eq!(usage.memory_used_bytes, 150);
        assert_eq!(usage.memory_peak_bytes, 200);
    }

    #[test]
    fn test_atomic_resource_limiter_check_memory() {
        let quota = ResourceQuota {
            memory_limit_bytes: 100,
            ..Default::default()
        };
        let limiter = AtomicResourceLimiter::new(quota);
        assert!(limiter.check_memory(50).is_ok());
        assert!(limiter.check_memory(150).is_err());
    }

    #[test]
    fn test_atomic_resource_limiter_check_output() {
        let quota = ResourceQuota {
            output_limit_bytes: 100,
            ..Default::default()
        };
        let limiter = AtomicResourceLimiter::new(quota);
        assert!(limiter.check_output(50).is_ok());
        assert!(limiter.check_output(150).is_err());
    }
}
