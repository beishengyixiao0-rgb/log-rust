use crate::message::LogMessage;
use chrono::{Local, TimeZone};
use serde_json::json;
use std::fmt::Write;
use std::sync::Arc;

/// 格式化项 trait - 每个格式说明符都实现此 trait
pub trait FormatItem: Send + Sync {
    fn format(&self, msg: &LogMessage, out: &mut String);
}

/// 消息内容格式化器
struct MsgFormatItem;
impl FormatItem for MsgFormatItem {
    fn format(&self, msg: &LogMessage, out: &mut String) {
        out.push_str(&msg.payload);
    }
}

/// 日志级别格式化器
struct LevelFormatItem;
impl FormatItem for LevelFormatItem {
    fn format(&self, msg: &LogMessage, out: &mut String) {
        out.push_str(msg.level.as_str());
    }
}

/// 日志器名称格式化器
struct NameFormatItem;
impl FormatItem for NameFormatItem {
    fn format(&self, msg: &LogMessage, out: &mut String) {
        out.push_str(&msg.name);
    }
}

/// 线程 ID 格式化器
struct ThreadFormatItem;
impl FormatItem for ThreadFormatItem {
    fn format(&self, msg: &LogMessage, out: &mut String) {
        write!(out, "{}", msg.thread_id).unwrap();
    }
}

/// 时间戳格式化器，支持自定义格式
struct TimeFormatItem {
    format: String,
}

impl TimeFormatItem {
    fn new(format: Option<String>) -> Self {
        TimeFormatItem {
            format: format.unwrap_or_else(|| "%H:%M:%S".to_string()),
        }
    }
}

impl FormatItem for TimeFormatItem {
    fn format(&self, msg: &LogMessage, out: &mut String) {
        let dt = Local.timestamp_opt(msg.timestamp as i64, 0).unwrap();
        out.push_str(&dt.format(&self.format).to_string());
    }
}

/// 源文件名格式化器
struct FileFormatItem;
impl FormatItem for FileFormatItem {
    fn format(&self, msg: &LogMessage, out: &mut String) {
        out.push_str(&msg.file);
    }
}

/// 源文件行号格式化器
struct LineFormatItem;
impl FormatItem for LineFormatItem {
    fn format(&self, msg: &LogMessage, out: &mut String) {
        write!(out, "{}", msg.line).unwrap();
    }
}

/// 制表符格式化器
struct TabFormatItem;
impl FormatItem for TabFormatItem {
    fn format(&self, _msg: &LogMessage, out: &mut String) {
        out.push('\t');
    }
}

/// 换行符格式化器
struct NewlineFormatItem;
impl FormatItem for NewlineFormatItem {
    fn format(&self, _msg: &LogMessage, out: &mut String) {
        out.push('\n');
    }
}

/// 纯文本格式化器
struct LiteralFormatItem {
    text: String,
}

impl FormatItem for LiteralFormatItem {
    fn format(&self, _msg: &LogMessage, out: &mut String) {
        out.push_str(&self.text);
    }
}

/// 日志格式化器 - 解析模式并格式化消息
///
/// 支持的格式说明符：
/// - `%d` - 日期/时间（可使用 `%d{format}` 自定义格式）
/// - `%T` - 制表符
/// - `%t` - 线程 ID
/// - `%p` - 日志级别
/// - `%c` - 日志器名称（类别）
/// - `%f` - 源文件
/// - `%l` - 源文件行号
/// - `%m` - 消息内容
/// - `%n` - 换行符
///
/// 示例模式：`[%d{%H:%M:%S}][%t][%p][%c][%f:%l] %m%n`
#[derive(Clone)]
pub struct Formatter {
    pattern: String,
    items: Vec<Arc<dyn FormatItem>>,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Formatter {
    /// 创建新的格式化器
    pub fn new(pattern: Option<String>) -> Self {
        let pattern =
            pattern.unwrap_or_else(|| "[%d{%H:%M:%S}][%t][%p][%c][%f:%l] %m%n".to_string());
        let items = Self::parse_pattern(&pattern);
        Formatter { pattern, items }
    }

    /// 获取当前模式
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// 格式化日志消息
    pub fn format(&self, msg: &LogMessage) -> String {
        let mut out = String::new();
        for item in &self.items {
            item.format(msg, &mut out);
        }
        out
    }

    // 解析模式字符串
    fn parse_pattern(pattern: &str) -> Vec<Arc<dyn FormatItem>> {
        let mut items: Vec<Arc<dyn FormatItem>> = Vec::new();
        let mut chars = pattern.chars().peekable();
        let mut literal = String::new();

        while let Some(c) = chars.next() {
            if c == '%' {
                // 输出之前累积的纯文本
                if !literal.is_empty() {
                    items.push(Arc::new(LiteralFormatItem {
                        text: literal.clone(),
                    }));
                    literal.clear();
                }

                // 检查转义的 %%
                if chars.peek() == Some(&'%') {
                    chars.next();
                    literal.push('%');
                    continue;
                }

                // 获取格式说明符
                if let Some(spec) = chars.next() {
                    // 检查子格式（如 %d{format}）
                    let sub_format = if chars.peek() == Some(&'{') {
                        chars.next(); // 消耗 '{'
                        let mut sub = String::new();
                        while let Some(&c) = chars.peek() {
                            if c == '}' {
                                chars.next();
                                break;
                            }
                            sub.push(c);
                            chars.next();
                        }
                        Some(sub)
                    } else {
                        None
                    };

                    items.push(Self::create_item(spec, sub_format));
                }
            } else {
                literal.push(c);
            }
        }

        // 输出剩余的纯文本
        if !literal.is_empty() {
            items.push(Arc::new(LiteralFormatItem { text: literal }));
        }

        items
    }

    // 创建格式化项
    fn create_item(spec: char, sub_format: Option<String>) -> Arc<dyn FormatItem> {
        match spec {
            'm' => Arc::new(MsgFormatItem),
            'p' => Arc::new(LevelFormatItem),
            'c' => Arc::new(NameFormatItem),
            't' => Arc::new(ThreadFormatItem),
            'n' => Arc::new(NewlineFormatItem),
            'd' => Arc::new(TimeFormatItem::new(sub_format)),
            'f' => Arc::new(FileFormatItem),
            'l' => Arc::new(LineFormatItem),
            'T' => Arc::new(TabFormatItem),
            _ => Arc::new(LiteralFormatItem {
                text: format!("%{}", spec),
            }),
        }
    }
}

#[derive(Clone, Default)]
pub struct JsonFormatter;

impl JsonFormatter {
    pub fn new() -> Self {
        Self
    }

    pub fn format(&self, msg: &LogMessage) -> String {
        json!({
            "logger_name": msg.name,
            "level": msg.level.as_str(),
            "message": msg.payload,
            "file": msg.file,
            "line": msg.line,
            "thread_id": msg.thread_id,
            "timestamp": msg.timestamp,
        })
        .to_string()
            + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogLevel;
    #[test]
    fn test_default_formatter() {
        let fmt = Formatter::default();
        let msg = LogMessage::new(
            "test".to_string(),
            "main.rs".to_string(),
            42,
            "hello".to_string(),
            LogLevel::Info,
        );
        let output = fmt.format(&msg);
        assert!(output.contains("INFO"));
        assert!(output.contains("test"));
        assert!(output.contains("hello"));
        assert!(output.contains("main.rs"));
    }

    #[test]
    fn test_custom_pattern() {
        let fmt = Formatter::new(Some("%p - %m%n".to_string()));
        let msg = LogMessage::new(
            "test".to_string(),
            "main.rs".to_string(),
            42,
            "hello".to_string(),
            LogLevel::Error,
        );
        let output = fmt.format(&msg);
        assert_eq!(output, "ERROR - hello\n");
    }

    #[test]
    fn test_formatter_clone() {
        let fmt1 = Formatter::new(Some("%p: %m%n".to_string()));
        let fmt2 = fmt1.clone();

        let msg = LogMessage::new(
            "test".to_string(),
            "test.rs".to_string(),
            1,
            "clone test".to_string(),
            LogLevel::Debug,
        );

        assert_eq!(fmt1.format(&msg), fmt2.format(&msg));
    }
}
