//! BitLog - 一个简单的同步和异步日志系统
//!
//!
//!
//! # 功能特性
//! - 同步和异步日志器
//! - 多种输出目标（标准输出、文件、滚动文件）
//! - 可定制的日志格式化
//! - 日志级别：DEBUG, INFO, WARN, ERROR, FATAL
//!
//! # 使用示例
//! ```
//! use bitlog::{debug, info, warn, error};
//!
//! // 使用根日志器和宏
//! info!("这是一条信息日志");
//! debug!("调试：{}", 42);
//!
//! // 或使用自定义日志器
// ! let logger = bitlog::get_logger("my_logger");
// ! LOGI!(logger, "来自 {} 的问候", "Rust");
//! ```

// 模块声明
pub mod buffer;
pub mod formatter;
pub mod level;
pub mod logger;
pub mod looper;
pub mod message;
pub mod sink;

// 公开主要类型
pub use formatter::{Formatter, JsonFormatter};
pub use level::LogLevel;
pub use logger::{
    create_logger, get_logger, logger_levels, root_logger, set_logger_level, Logger, LoggerBuilder,
    LoggerType,
};
pub use sink::{
    CleanupPolicy, FileSink, LogSink, MysqlSink, RollSink, RollingPolicy, SinkFactory, StdoutSink,
    TimeRollingPolicy,
};

pub mod compressor;
pub mod config;
pub mod dashboard;
pub mod panic_hook;
pub mod recent;
pub mod runtime;
pub mod stats;
pub mod tui;

// 宏通过 #[macro_export] 自动导出到 crate 根目录

/// 便捷的宏别名，匹配 C++ API
#[macro_export]
macro_rules! LOGD {
    ($($arg:tt)*) => { $crate::LOG_DEBUG($crate::root_logger(), $($arg)*) };
}

#[macro_export]
macro_rules! LOGI {
    ($($arg:tt)*) => { $crate::LOG_INFO($crate::root_logger(), $($arg)*) };
}

#[macro_export]
macro_rules! LOGW {
    ($($arg:tt)*) => { $crate::LOG_WARN($crate::root_logger(), $($arg)*) };
}

#[macro_export]
macro_rules! LOGE {
    ($($arg:tt)*) => { $crate::LOG_ERROR($crate::root_logger(), $($arg)*) };
}

#[macro_export]
macro_rules! LOGF {
    ($($arg:tt)*) => { $crate::LOG_FATAL($crate::root_logger(), $($arg)*) };
}
