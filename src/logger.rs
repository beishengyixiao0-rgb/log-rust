use crate::formatter::Formatter;
use crate::level::LogLevel;
use crate::looper::AsyncLooper;
use crate::message::LogMessage;
use crate::sink::LogSink;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// 日志器类型 - 同步或异步
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoggerType {
    Sync,
    Async,
}

/// 日志器 trait - 所有日志器类型的通用接口
pub trait LoggerTrait: Send + Sync {
    fn name(&self) -> &str;
    fn level(&self) -> LogLevel;
    fn set_level(&self, level: LogLevel);
    fn log(&self, level: LogLevel, file: &str, line: u32, payload: &str);
}

/// 日志器内部实现
struct LoggerInner {
    name: String,
    level: Mutex<LogLevel>,
    formatter: Formatter,
    sinks: Vec<Arc<dyn LogSink>>,
}

/// 同步日志器
pub struct SyncLogger {
    inner: LoggerInner,
}

impl SyncLogger {
    /// 创建新的同步日志器
    pub fn new(
        name: String,
        formatter: Option<Formatter>,
        sinks: Vec<Arc<dyn LogSink>>,
        level: LogLevel,
    ) -> Self {
        let formatter = formatter.unwrap_or_default();

        SyncLogger {
            inner: LoggerInner {
                name,
                level: Mutex::new(level),
                formatter,
                sinks,
            },
        }
    }

    // 执行日志写入
    fn log_it(&self, msg: &LogMessage) {
        let formatted = self.inner.formatter.format(msg);
        for sink in &self.inner.sinks {
            sink.log(formatted.as_bytes());
        }
    }
}

impl LoggerTrait for SyncLogger {
    fn name(&self) -> &str {
        &self.inner.name
    }

    fn level(&self) -> LogLevel {
        *self.inner.level.lock()
    }

    fn set_level(&self, level: LogLevel) {
        *self.inner.level.lock() = level;
    }

    fn log(&self, level: LogLevel, file: &str, line: u32, payload: &str) {
        // 检查日志级别是否允许
        if level >= self.level() {
            crate::stats::GLOBAL_STATS.record(level);
            let msg = LogMessage::new(
                self.inner.name.clone(),
                file.to_string(),
                line,
                payload.to_string(),
                level,
            );
            self.log_it(&msg);
        }
    }
}

/// 异步日志器
pub struct AsyncLogger {
    inner: LoggerInner,
    looper: Arc<AsyncLooper>,
}

impl AsyncLogger {
    /// 创建新的异步日志器
    pub fn new(
        name: String,
        formatter: Option<Formatter>,
        sinks: Vec<Arc<dyn LogSink>>,
        level: LogLevel,
    ) -> Self {
        let formatter = formatter.unwrap_or_default();

        // 创建共享的 sinks 用于回调
        let sinks_clone = sinks.clone();

        let looper = AsyncLooper::new(move |buf| {
            let data = buf.to_string();
            if !data.is_empty() {
                for sink in &sinks_clone {
                    sink.log(data.as_bytes());
                }
            }
        });

        AsyncLogger {
            inner: LoggerInner {
                name,
                level: Mutex::new(level),
                formatter,
                sinks,
            },
            looper: Arc::new(looper),
        }
    }

    // 执行日志写入（推送到异步队列）
    fn log_it(&self, msg: &LogMessage) {
        let formatted = self.inner.formatter.format(msg);
        self.looper.push(&formatted);
    }
}

impl LoggerTrait for AsyncLogger {
    fn name(&self) -> &str {
        &self.inner.name
    }

    fn level(&self) -> LogLevel {
        *self.inner.level.lock()
    }

    fn set_level(&self, level: LogLevel) {
        *self.inner.level.lock() = level;
    }

    fn log(&self, level: LogLevel, file: &str, line: u32, payload: &str) {
        // 检查日志级别是否允许
        if level >= self.level() {
            crate::stats::GLOBAL_STATS.record(level);
            let msg = LogMessage::new(
                self.inner.name.clone(),
                file.to_string(),
                line,
                payload.to_string(),
                level,
            );
            self.log_it(&msg);
        }
    }
}

impl Drop for AsyncLogger {
    fn drop(&mut self) {
        // 日志器销毁时停止 looper
        // 注意：无法通过 Arc 直接访问 looper.stop()
        // 这由 AsyncLooper 的 Drop 实现处理
    }
}

/// 公共日志器包装器
pub struct Logger {
    inner: Arc<dyn LoggerTrait>,
}

impl Clone for Logger {
    fn clone(&self) -> Self {
        Logger {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Logger {
    /// 获取日志器名称
    pub fn name(&self) -> &str {
        self.inner.name()
    }

    /// 获取当前日志级别
    pub fn level(&self) -> LogLevel {
        self.inner.level()
    }

    /// 设置日志级别
    pub fn set_level(&self, level: LogLevel) {
        self.inner.set_level(level);
    }

    /// 写入日志
    pub fn log(&self, level: LogLevel, file: &str, line: u32, payload: &str) {
        self.inner.log(level, file, line, payload);
    }

    /// 调试级别日志
    pub fn debug(&self, file: &str, line: u32, payload: &str) {
        self.log(LogLevel::Debug, file, line, payload);
    }

    /// 信息级别日志
    pub fn info(&self, file: &str, line: u32, payload: &str) {
        self.log(LogLevel::Info, file, line, payload);
    }

    /// 警告级别日志
    pub fn warn(&self, file: &str, line: u32, payload: &str) {
        self.log(LogLevel::Warn, file, line, payload);
    }

    /// 错误级别日志
    pub fn error(&self, file: &str, line: u32, payload: &str) {
        self.log(LogLevel::Error, file, line, payload);
    }

    /// 致命级别日志
    pub fn fatal(&self, file: &str, line: u32, payload: &str) {
        self.log(LogLevel::Fatal, file, line, payload);
    }
}

/// 日志器构建器，用于流式 API
pub struct LoggerBuilder {
    name: Option<String>,
    level: LogLevel,
    logger_type: LoggerType,
    formatter: Option<Formatter>,
    sinks: Vec<Arc<dyn LogSink>>,
}

impl Default for LoggerBuilder {
    fn default() -> Self {
        LoggerBuilder {
            name: None,
            level: LogLevel::Debug,
            logger_type: LoggerType::Sync,
            formatter: None,
            sinks: Vec::new(),
        }
    }
}

impl LoggerBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置日志器名称
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// 设置日志级别
    pub fn level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// 设置日志器类型（同步/异步）
    pub fn logger_type(mut self, logger_type: LoggerType) -> Self {
        self.logger_type = logger_type;
        self
    }

    /// 设置格式化器
    pub fn formatter(mut self, formatter: Formatter) -> Self {
        self.formatter = Some(formatter);
        self
    }

    /// 添加输出目标
    pub fn sink(mut self, sink: Arc<dyn LogSink>) -> Self {
        self.sinks.push(sink);
        self
    }

    /// 构建日志器
    pub fn build(self) -> Logger {
        let name = self.name.unwrap_or_else(|| "unnamed".to_string());

        let mut sinks = self.sinks;
        if sinks.is_empty() {
            // 默认使用标准输出
            sinks.push(Arc::new(crate::sink::StdoutSink::new()));
        }

        let inner: Arc<dyn LoggerTrait> = match self.logger_type {
            LoggerType::Sync => Arc::new(SyncLogger::new(name, self.formatter, sinks, self.level)),
            LoggerType::Async => {
                Arc::new(AsyncLogger::new(name, self.formatter, sinks, self.level))
            }
        };

        Logger { inner }
    }
}

/// 日志器管理器 - 单例模式管理所有日志器
struct LoggerManager {
    root_logger: Logger,
    loggers: Mutex<HashMap<String, Logger>>,
}

impl LoggerManager {
    fn new() -> Self {
        let root_logger = LoggerBuilder::new()
            .name("root")
            .logger_type(LoggerType::Sync)
            .build();

        let mut loggers = HashMap::new();
        loggers.insert("root".to_string(), root_logger.clone());

        LoggerManager {
            root_logger,
            loggers: Mutex::new(loggers),
        }
    }

    fn instance() -> &'static Self {
        static INSTANCE: once_cell::sync::Lazy<LoggerManager> =
            once_cell::sync::Lazy::new(LoggerManager::new);
        &INSTANCE
    }

    #[allow(dead_code)]
    fn has_logger(&self, name: &str) -> bool {
        self.loggers.lock().contains_key(name)
    }

    fn add_logger(&self, name: &str, logger: Logger) {
        self.loggers.lock().insert(name.to_string(), logger);
    }

    fn get_logger(&self, name: &str) -> Option<Logger> {
        self.loggers.lock().get(name).cloned()
    }

    fn root_logger(&self) -> Logger {
        self.root_logger.clone()
    }
}

/// 根据名称获取日志器
pub fn get_logger(name: &str) -> Option<Logger> {
    LoggerManager::instance().get_logger(name)
}

/// 获取根日志器
pub fn root_logger() -> Logger {
    LoggerManager::instance().root_logger()
}

/// 创建并注册新的日志器
pub fn create_logger(builder: LoggerBuilder) -> Logger {
    let logger = builder.build();
    let name = logger.name().to_string();
    LoggerManager::instance().add_logger(&name, logger.clone());
    logger
}

// ============================================================================
// 日志宏 - 匹配 C++ API
// ============================================================================

/// 调试级别日志宏
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::logger::root_logger().debug(file!(), line!(), &msg);
    }};
}

/// 信息级别日志宏
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::logger::root_logger().info(file!(), line!(), &msg);
    }};
}

/// 警告级别日志宏
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::logger::root_logger().warn(file!(), line!(), &msg);
    }};
}

/// 错误级别日志宏
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::logger::root_logger().error(file!(), line!(), &msg);
    }};
}

/// 致命级别日志宏
#[macro_export]
macro_rules! fatal {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::logger::root_logger().fatal(file!(), line!(), &msg);
    }};
}

/// 向日志器写入 DEBUG 级别日志
#[macro_export]
macro_rules! LOG_DEBUG {
    ($logger:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $logger.debug(file!(), line!(), &msg);
    }};
}

/// 向日志器写入 INFO 级别日志
#[macro_export]
macro_rules! LOG_INFO {
    ($logger:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $logger.info(file!(), line!(), &msg);
    }};
}

/// 向日志器写入 WARN 级别日志
#[macro_export]
macro_rules! LOG_WARN {
    ($logger:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $logger.warn(file!(), line!(), &msg);
    }};
}

/// 向日志器写入 ERROR 级别日志
#[macro_export]
macro_rules! LOG_ERROR {
    ($logger:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $logger.error(file!(), line!(), &msg);
    }};
}

/// 向日志器写入 FATAL 级别日志
#[macro_export]
macro_rules! LOG_FATAL {
    ($logger:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $logger.fatal(file!(), line!(), &msg);
    }};
}
