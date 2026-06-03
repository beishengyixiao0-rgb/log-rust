use chrono::{Datelike, Local, Timelike};
use parking_lot::Mutex;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use colored::*;

/// 日志输出目标 trait - 所有输出类型都实现此 trait
pub trait LogSink: Send + Sync {
    fn log(&self, data: &[u8]);
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

/// 滚动文件输出目标 - 根据文件大小限制创建新文件
// pub struct RollSink {
//     basename: String,
//     max_file_size: usize,
//     current_file_size: Arc<Mutex<usize>>,
//     current_file: Arc<Mutex<Option<BufWriter<File>>>>,
// }

pub struct RollSink {
    basename: String,

    max_file_size: usize,

    current_file_size: Arc<Mutex<usize>>,

    current_file: Arc<Mutex<Option<BufWriter<File>>>>,

    // =========================
    // 新增：
    // 当前日志文件名
    // =========================
    current_filename: Arc<Mutex<Option<String>>>,
}

impl RollSink {
    /// 创建新的滚动文件输出目标
    pub fn new(basename: &str, max_file_size: usize) -> Self {
        // 如需则创建目录
        if let Some(parent) = Path::new(basename).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        // RollSink {
        //     basename: basename.to_string(),
        //     max_file_size,
        //     current_file_size: Arc::new(Mutex::new(0)),
        //     current_file: Arc::new(Mutex::new(None)),
        // }

        RollSink {
            basename: basename.to_string(),

            max_file_size,

            current_file_size: Arc::new(Mutex::new(0)),

            current_file: Arc::new(Mutex::new(None)),

            // =========================
            // 新增
            // =========================
            current_filename: Arc::new(Mutex::new(None)),
        }
    }

    // 生成带时间戳的文件名
    fn create_filename(&self) -> String {
        let now = Local::now();
        format!(
            "{}{}{}{}{}{}{}.log",
            self.basename,
            now.year(),
            now.month(),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        )
    }

    // 初始化日志文件
    // fn init_log_file(&self) {
    //     let mut file_guard = self.current_file.lock();
    //     let mut size_guard = self.current_file_size.lock();

    //     let needs_new_file = file_guard.is_none() || *size_guard >= self.max_file_size;

    //     if needs_new_file {
    //         // 关闭当前文件（如果已打开）
    //         if let Some(ref mut file) = *file_guard {
    //             let _ = file.flush();
    //         }

    //         // 创建新文件
    //         let filename = self.create_filename();
    //         match OpenOptions::new().create(true).append(true).open(&filename) {
    //             Ok(file) => {
    //                 *file_guard = Some(BufWriter::new(file));
    //                 *size_guard = 0;
    //             }
    //             Err(e) => {
    //                 eprintln!("Failed to create rolling log file: {}", e);
    //             }
    //         }
    //     }
    // }

    fn init_log_file(&self) {
        let mut file_guard = self.current_file.lock();

        let mut size_guard = self.current_file_size.lock();

        // =========================
        // 新增：
        // 当前文件名锁
        // =========================
        let mut filename_guard = self.current_filename.lock();

        let needs_new_file = file_guard.is_none() || *size_guard >= self.max_file_size;

        if needs_new_file {
            // =========================
            // 保存旧文件名
            // 后面用于 gzip 压缩
            // =========================
            let old_filename = filename_guard.clone();

            // =========================
            // flush旧文件
            // =========================
            if let Some(ref mut file) = *file_guard {
                let _ = file.flush();
            }

            // =========================
            // 创建新文件名
            // =========================
            let filename = self.create_filename();

            // =========================
            // 打开新文件
            // =========================
            match OpenOptions::new().create(true).append(true).open(&filename) {
                Ok(file) => {
                    *file_guard = Some(BufWriter::new(file));

                    *size_guard = 0;

                    // =========================
                    // 更新当前文件名
                    // =========================
                    *filename_guard = Some(filename.clone());

                    // =========================
                    // 异步压缩旧文件
                    // =========================
                    if let Some(old_file) = old_filename {
                        crate::compressor::compress_file_async(old_file);
                    }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_stdout_sink() {
        let sink = StdoutSink::new();
        sink.log(b"test message\n");
        // Visual verification - should print to stdout
    }

    #[test]
    fn test_file_sink() {
        let test_file = "/tmp/bitlog_test.log";
        {
            let sink = FileSink::new(test_file);
            sink.log(b"test message\n");
        }
        // Verify file was created and contains data
        let content = fs::read_to_string(test_file).unwrap();
        assert!(content.contains("test message"));
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_rolling_sink() {
        let test_base = "/tmp/bitlog_roll_test_";
        let sink = RollSink::new(test_base, 1024);
        sink.log(b"test message\n");
        // Rolling sink should create a file
    }
}
