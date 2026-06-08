use crate::level::LogLevel;
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// 日志消息结构体，包含所有日志元数据
#[derive(Clone, Serialize)]
pub struct LogMessage {
    pub name: String,    // 日志器名称
    pub file: String,    // 源文件
    pub line: u32,       // 源文件行号
    pub payload: String, // 日志内容
    pub level: LogLevel, // 日志级别
    pub timestamp: u64,  // Unix 时间戳
    pub thread_id: u64,  // 线程 ID
}

impl LogMessage {
    /// 创建新的日志消息
    pub fn new(name: String, file: String, line: u32, payload: String, level: LogLevel) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 使用稳定方法获取线程 ID 并转换为 u64
        let id = thread::current().id();
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        let thread_id = hasher.finish();

        LogMessage {
            name,
            file,
            line,
            payload,
            level,
            timestamp,
            thread_id,
        }
    }
}

/// 获取当前时间戳的辅助函数（匹配 C++ util::date::now()）
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_message_creation() {
        let msg = LogMessage::new(
            "test".to_string(),
            "test.rs".to_string(),
            42,
            "hello".to_string(),
            LogLevel::Info,
        );
        assert_eq!(msg.name, "test");
        assert_eq!(msg.file, "test.rs");
        assert_eq!(msg.line, 42);
        assert_eq!(msg.payload, "hello");
        assert_eq!(msg.level, LogLevel::Info);
    }
}
