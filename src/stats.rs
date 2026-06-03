use crate::level::LogLevel;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct LogStats {
    pub total: AtomicU64,
    pub debug: AtomicU64,
    pub info: AtomicU64,
    pub warn: AtomicU64,
    pub error: AtomicU64,
    pub fatal: AtomicU64,

    start_time: Instant,
}

impl LogStats {
    pub fn new() -> Self {
        Self {
            total: AtomicU64::new(0),
            debug: AtomicU64::new(0),
            info: AtomicU64::new(0),
            warn: AtomicU64::new(0),
            error: AtomicU64::new(0),
            fatal: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    /// 记录日志统计
    pub fn record(&self, level: LogLevel) {
        self.total.fetch_add(1, Ordering::Relaxed);

        match level {
            LogLevel::Debug => {
                self.debug.fetch_add(1, Ordering::Relaxed);
            }
            LogLevel::Info => {
                self.info.fetch_add(1, Ordering::Relaxed);
            }
            LogLevel::Warn => {
                self.warn.fetch_add(1, Ordering::Relaxed);
            }
            LogLevel::Error => {
                self.error.fetch_add(1, Ordering::Relaxed);
            }
            LogLevel::Fatal => {
                self.fatal.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// 获取QPS
    pub fn qps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();

        if elapsed <= 0.0 {
            return 0.0;
        }

        self.total.load(Ordering::Relaxed) as f64 / elapsed
    }

    /// 错误率
    pub fn error_rate(&self) -> f64 {
        let total = self.total.load(Ordering::Relaxed);

        if total == 0 {
            return 0.0;
        }

        let error_count = self.error.load(Ordering::Relaxed) + self.fatal.load(Ordering::Relaxed);

        error_count as f64 / total as f64
    }

    /// 输出统计信息
    pub fn print_report(&self) {
        println!();
        println!("========== BitLog Stats ==========");
        println!("Total Logs   : {}", self.total.load(Ordering::Relaxed));

        println!("DEBUG        : {}", self.debug.load(Ordering::Relaxed));

        println!("INFO         : {}", self.info.load(Ordering::Relaxed));

        println!("WARN         : {}", self.warn.load(Ordering::Relaxed));

        println!("ERROR        : {}", self.error.load(Ordering::Relaxed));

        println!("FATAL        : {}", self.fatal.load(Ordering::Relaxed));

        println!("QPS          : {:.2}", self.qps());

        println!("Error Rate   : {:.2}%", self.error_rate() * 100.0);

        println!("==================================");
        println!();
    }
}

/// 全局统计对象
pub static GLOBAL_STATS: Lazy<LogStats> = Lazy::new(LogStats::new);
