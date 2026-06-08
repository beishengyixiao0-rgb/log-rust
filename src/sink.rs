use crate::message::LogMessage;
use chrono::{Local, TimeZone};
use mysql::prelude::Queryable;
use mysql::{params, Pool};
use parking_lot::Mutex;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use colored::*;

/// 日志输出目标 trait - 所有输出类型都实现此 trait
pub trait LogSink: Send + Sync {
    fn log(&self, data: &[u8]);

    fn log_message(&self, msg: &LogMessage, formatted: &str) {
        let _ = msg;
        self.log(formatted.as_bytes());
    }

    fn accepts_batch(&self) -> bool {
        true
    }
}

/// 标准输出目标 - 写入到 stdout
pub struct StdoutSink {
    _private: (), // 防止外部构造
}

impl Default for StdoutSink {
    fn default() -> Self {
        Self::new()
    }
}

impl StdoutSink {
    pub fn new() -> Self {
        StdoutSink { _private: () }
    }
}

// impl LogSink for StdoutSink {
//     fn log(&self, data: &[u8]) {
//         let stdout = std::io::stdout();
//         let mut handle = stdout.lock();
//         let _ = handle.write_all(data);
//         let _ = handle.flush();
//     }
// }

impl LogSink for StdoutSink {
    fn log(&self, data: &[u8]) {
        let text = String::from_utf8_lossy(data);

        let colored_text = if text.contains("[ERROR]") {
            text.red().to_string()
        } else if text.contains("[WARN]") {
            text.yellow().to_string()
        } else if text.contains("[INFO]") {
            text.green().to_string()
        } else if text.contains("[DEBUG]") {
            text.blue().to_string()
        } else {
            text.normal().to_string()
        };

        let stdout = std::io::stdout();
        let mut handle = stdout.lock();

        let _ = handle.write_all(colored_text.as_bytes());
        let _ = handle.flush();
    }
}

/// 文件输出目标 - 写入单个文件
pub struct FileSink {
    filename: String,
    writer: Arc<Mutex<BufWriter<File>>>,
}

pub struct MysqlSink {
    pool: Pool,
    table: String,
}

impl MysqlSink {
    pub fn new(url: &str, table: &str) -> Result<Self, mysql::Error> {
        Ok(Self {
            pool: Pool::new(url)?,
            table: table.to_string(),
        })
    }

    pub fn default_local() -> Result<Self, mysql::Error> {
        Self::new("mysql://root:484236@127.0.0.1:3306/bitlog", "logs")
    }

    fn insert_sql(&self) -> String {
        format!(
            "INSERT INTO {} \
             (logger_name, level, message, file, line, thread_id, created_at) \
             VALUES \
             (:logger_name, :level, :message, :file, :line, :thread_id, :created_at)",
            self.table
        )
    }
}

impl LogSink for MysqlSink {
    fn log(&self, data: &[u8]) {
        let fallback = LogMessage::new(
            "unknown".to_string(),
            "unknown".to_string(),
            0,
            String::from_utf8_lossy(data).to_string(),
            crate::level::LogLevel::Info,
        );
        self.log_message(&fallback, &fallback.payload);
    }

    fn log_message(&self, msg: &LogMessage, _formatted: &str) {
        let created_at = Local
            .timestamp_opt(msg.timestamp as i64, 0)
            .single()
            .unwrap_or_else(Local::now)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let mut conn = match self.pool.get_conn() {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Failed to get MySQL connection: {}", e);
                return;
            }
        };

        if let Err(e) = conn.exec_drop(
            self.insert_sql(),
            params! {
                "logger_name" => &msg.name,
                "level" => msg.level.as_str(),
                "message" => &msg.payload,
                "file" => &msg.file,
                "line" => msg.line,
                "thread_id" => msg.thread_id,
                "created_at" => created_at,
            },
        ) {
            eprintln!("Failed to write log to MySQL: {}", e);
        }
    }

    fn accepts_batch(&self) -> bool {
        false
    }
}

impl FileSink {
    /// 创建新的文件输出目标
    pub fn new(filename: &str) -> Self {
        // 如需则创建目录
        if let Some(parent) = Path::new(filename).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(filename)
            .expect("Failed to open log file");

        FileSink {
            filename: filename.to_string(),
            writer: Arc::new(Mutex::new(BufWriter::new(file))),
        }
    }

    /// 获取文件名
    pub fn filename(&self) -> &str {
        &self.filename
    }
}

impl LogSink for FileSink {
    fn log(&self, data: &[u8]) {
        let mut writer = self.writer.lock();
        if let Err(e) = writer.write_all(data) {
            eprintln!("Failed to write to log file: {}", e);
        }
        let _ = writer.flush();
    }
}

#[derive(Clone, Copy)]
pub enum TimeRollingPolicy {
    Hourly,
    Daily,
}

#[derive(Clone, Copy)]
pub enum RollingPolicy {
    Size(usize),
    Time(TimeRollingPolicy),
    SizeAndTime {
        max_size: usize,
        time: TimeRollingPolicy,
    },
}

#[derive(Clone, Copy, Default)]
pub struct CleanupPolicy {
    pub max_age: Option<Duration>,
    pub max_files: Option<usize>,
}

pub struct RollSink {
    basename: String,
    policy: RollingPolicy,
    cleanup: CleanupPolicy,
    current_file_size: Arc<Mutex<usize>>,
    current_file: Arc<Mutex<Option<BufWriter<File>>>>,
    current_filename: Arc<Mutex<Option<String>>>,
    current_period: Arc<Mutex<Option<String>>>,
    sequence: AtomicU64,
}

impl RollSink {
    /// 创建新的滚动文件输出目标
    pub fn new(basename: &str, max_file_size: usize) -> Self {
        Self::with_policy(basename, RollingPolicy::Size(max_file_size))
    }

    pub fn with_policy(basename: &str, policy: RollingPolicy) -> Self {
        Self::with_policy_and_cleanup(basename, policy, CleanupPolicy::default())
    }

    pub fn with_policy_and_cleanup(
        basename: &str,
        policy: RollingPolicy,
        cleanup: CleanupPolicy,
    ) -> Self {
        // 如需则创建目录
        if let Some(parent) = Path::new(basename).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        RollSink {
            basename: basename.to_string(),
            policy,
            cleanup,
            current_file_size: Arc::new(Mutex::new(0)),
            current_file: Arc::new(Mutex::new(None)),
            current_filename: Arc::new(Mutex::new(None)),
            current_period: Arc::new(Mutex::new(None)),
            sequence: AtomicU64::new(0),
        }
    }

    // 生成带时间戳的文件名
    fn create_filename(&self) -> String {
        let now = Local::now();
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed);
        format!(
            "{}{}_{}.log",
            self.basename,
            now.format("%Y%m%d%H%M%S%3f"),
            seq
        )
    }

    fn current_period(&self) -> Option<String> {
        let now = Local::now();
        match self.policy {
            RollingPolicy::Size(_) => None,
            RollingPolicy::Time(TimeRollingPolicy::Hourly)
            | RollingPolicy::SizeAndTime {
                time: TimeRollingPolicy::Hourly,
                ..
            } => Some(now.format("%Y%m%d%H").to_string()),
            RollingPolicy::Time(TimeRollingPolicy::Daily)
            | RollingPolicy::SizeAndTime {
                time: TimeRollingPolicy::Daily,
                ..
            } => Some(now.format("%Y%m%d").to_string()),
        }
    }

    fn should_roll(&self, file_open: bool, size: usize, current_period: Option<&str>) -> bool {
        if !file_open {
            return true;
        }

        let size_reached = match self.policy {
            RollingPolicy::Size(max_size) | RollingPolicy::SizeAndTime { max_size, .. } => {
                size >= max_size
            }
            RollingPolicy::Time(_) => false,
        };

        let time_reached = match self.current_period() {
            Some(now_period) => current_period != Some(now_period.as_str()),
            None => false,
        };

        size_reached || time_reached
    }

    fn cleanup_old_files(&self) {
        if self.cleanup.max_age.is_none() && self.cleanup.max_files.is_none() {
            return;
        }

        let basename = self.basename.clone();
        let cleanup = self.cleanup;

        thread::spawn(move || {
            let base_path = Path::new(&basename);
            let dir = base_path.parent().unwrap_or_else(|| Path::new("."));
            let prefix = base_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");

            let mut files: Vec<(PathBuf, SystemTime)> = match fs::read_dir(dir) {
                Ok(entries) => entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| {
                        let path = entry.path();
                        let name = path.file_name()?.to_str()?;
                        if !name.starts_with(prefix) {
                            return None;
                        }

                        let extension = path.extension()?.to_str()?;
                        if extension != "log" && extension != "gz" {
                            return None;
                        }

                        let modified = entry.metadata().ok()?.modified().ok()?;
                        Some((path, modified))
                    })
                    .collect(),
                Err(_) => return,
            };

            if let Some(max_age) = cleanup.max_age {
                let now = SystemTime::now();
                for (path, modified) in &files {
                    if now
                        .duration_since(*modified)
                        .map(|age| age > max_age)
                        .unwrap_or(false)
                    {
                        let _ = fs::remove_file(path);
                    }
                }
            }

            if let Some(max_files) = cleanup.max_files {
                files.retain(|(path, _)| path.exists());
                files.sort_by_key(|(_, modified)| *modified);

                let excess = files.len().saturating_sub(max_files);
                for (path, _) in files.into_iter().take(excess) {
                    let _ = fs::remove_file(path);
                }
            }
        });
    }

    fn init_log_file(&self) {
        let mut file_guard = self.current_file.lock();
        let mut size_guard = self.current_file_size.lock();
        let mut filename_guard = self.current_filename.lock();
        let mut period_guard = self.current_period.lock();

        let needs_new_file =
            self.should_roll(file_guard.is_some(), *size_guard, period_guard.as_deref());

        if needs_new_file {
            let old_filename = filename_guard.clone();

            if let Some(ref mut file) = *file_guard {
                let _ = file.flush();
            }

            let filename = self.create_filename();

            match OpenOptions::new()
                .create_new(true)
                .append(true)
                .open(&filename)
            {
                Ok(file) => {
                    *file_guard = Some(BufWriter::new(file));
                    *size_guard = 0;
                    *filename_guard = Some(filename.clone());
                    *period_guard = self.current_period();

                    if let Some(old_file) = old_filename {
                        crate::compressor::compress_file_async(old_file);
                    }

                    self.cleanup_old_files();
                }
                Err(e) => {
                    eprintln!("Failed to create rolling log file: {}", e);
                }
            }
        }
    }
}

impl LogSink for RollSink {
    fn log(&self, data: &[u8]) {
        self.init_log_file();

        let mut file_guard = self.current_file.lock();
        let mut size_guard = self.current_file_size.lock();

        if let Some(ref mut file) = *file_guard {
            if let Err(e) = file.write_all(data) {
                eprintln!("Failed to write to rolling log file: {}", e);
            }
            let _ = file.flush();
            *size_guard += data.len();
        }
    }
}

/// 输出目标工厂 - 用于创建各种输出目标
pub struct SinkFactory;

impl SinkFactory {
    /// 创建标准输出目标
    pub fn stdout() -> Arc<dyn LogSink> {
        Arc::new(StdoutSink::new())
    }

    /// 创建文件输出目标
    pub fn file(filename: &str) -> Arc<dyn LogSink> {
        Arc::new(FileSink::new(filename))
    }

    /// 创建滚动文件输出目标
    pub fn rolling(basename: &str, max_size: usize) -> Arc<dyn LogSink> {
        Arc::new(RollSink::new(basename, max_size))
    }

    pub fn rolling_with_policy(basename: &str, policy: RollingPolicy) -> Arc<dyn LogSink> {
        Arc::new(RollSink::with_policy(basename, policy))
    }

    pub fn rolling_with_cleanup(
        basename: &str,
        policy: RollingPolicy,
        cleanup: CleanupPolicy,
    ) -> Arc<dyn LogSink> {
        Arc::new(RollSink::with_policy_and_cleanup(basename, policy, cleanup))
    }

    pub fn mysql(url: &str, table: &str) -> Result<Arc<dyn LogSink>, mysql::Error> {
        Ok(Arc::new(MysqlSink::new(url, table)?))
    }

    pub fn mysql_default() -> Result<Arc<dyn LogSink>, mysql::Error> {
        Ok(Arc::new(MysqlSink::default_local()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn test_stdout_sink() {
        let sink = StdoutSink::new();
        sink.log(b"test message\n");
        // Visual verification - should print to stdout
    }

    #[test]
    fn test_file_sink() {
        let test_file = temp_path("bitlog_test.log");
        {
            let sink = FileSink::new(test_file.to_str().unwrap());
            sink.log(b"test message\n");
        }
        // Verify file was created and contains data
        let content = fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("test message"));
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_rolling_sink() {
        let test_base = temp_path("bitlog_roll_test_");
        let sink = RollSink::new(test_base.to_str().unwrap(), 1024);
        sink.log(b"test message\n");
        // Rolling sink should create a file
    }

    #[test]
    fn test_rolling_sink_generates_unique_names() {
        let test_base = temp_path("bitlog_roll_unique_");
        let sink = RollSink::new(test_base.to_str().unwrap(), 1);

        let first = sink.create_filename();
        let second = sink.create_filename();

        assert_ne!(first, second);
    }
}
