use bitlog::message::LogMessage;
use bitlog::sink::{CleanupPolicy, LogSink, RollingPolicy, SinkFactory, TimeRollingPolicy};
use bitlog::{
    create_logger, get_logger, Formatter, LogLevel, LoggerBuilder, LoggerType, LOG_DEBUG,
    LOG_ERROR, LOG_INFO, LOG_WARN,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Default)]
struct MemorySink {
    entries: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Default)]
struct StructuredSink {
    entries: Arc<Mutex<Vec<(String, LogLevel, String, u32)>>>,
}

impl StructuredSink {
    fn entries(&self) -> Vec<(String, LogLevel, String, u32)> {
        self.entries.lock().unwrap().clone()
    }
}

impl LogSink for StructuredSink {
    fn log(&self, _data: &[u8]) {}

    fn log_message(&self, msg: &LogMessage, _formatted: &str) {
        self.entries.lock().unwrap().push((
            msg.name.clone(),
            msg.level,
            msg.file.clone(),
            msg.line,
        ));
    }

    fn accepts_batch(&self) -> bool {
        false
    }
}

impl MemorySink {
    fn entries(&self) -> Vec<String> {
        self.entries.lock().unwrap().clone()
    }
}

impl LogSink for MemorySink {
    fn log(&self, data: &[u8]) {
        self.entries
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(data).to_string());
    }
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("bitlog_acceptance_{}_{}", name, nanos))
}

#[test]
fn sync_logger_formats_and_filters_by_level() {
    let sink = MemorySink::default();
    let logger = LoggerBuilder::new()
        .name("sync_acceptance")
        .level(LogLevel::Info)
        .logger_type(LoggerType::Sync)
        .formatter(Formatter::new(Some("[%p][%c] %m%n".to_string())))
        .sink(Arc::new(sink.clone()))
        .build();

    LOG_DEBUG!(logger, "debug should be filtered");
    LOG_INFO!(logger, "user {} logged in", "Alice");
    LOG_WARN!(logger, "disk usage is {}%", 82);
    LOG_ERROR!(logger, "request failed: {}", 500);

    let entries = sink.entries();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0], "[INFO][sync_acceptance] user Alice logged in\n");
    assert_eq!(entries[1], "[WARN][sync_acceptance] disk usage is 82%\n");
    assert_eq!(entries[2], "[ERROR][sync_acceptance] request failed: 500\n");
}

#[test]
fn async_logger_writes_messages_from_multiple_threads() {
    let sink = MemorySink::default();
    let logger = LoggerBuilder::new()
        .name("async_acceptance")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Async)
        .formatter(Formatter::new(Some("[%p][%c] %m%n".to_string())))
        .sink(Arc::new(sink.clone()))
        .build();

    let mut handles = Vec::new();
    for thread_id in 0..4 {
        let logger = logger.clone();
        handles.push(thread::spawn(move || {
            for msg_id in 0..10 {
                LOG_INFO!(logger, "thread {} message {}", thread_id, msg_id);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    drop(logger);

    let output = sink.entries().join("");
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 40);
    assert!(output.contains("thread 0 message 0"));
    assert!(output.contains("thread 3 message 9"));
}

#[test]
fn async_logger_passes_structured_messages_to_database_style_sinks() {
    let sink = StructuredSink::default();
    let logger = LoggerBuilder::new()
        .name("structured_async_acceptance")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Async)
        .sink(Arc::new(sink.clone()))
        .build();

    LOG_ERROR!(logger, "database style sink keeps source metadata");
    drop(logger);

    let entries = sink.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "structured_async_acceptance");
    assert_eq!(entries[0].1, LogLevel::Error);
    assert!(entries[0].2.ends_with("acceptance.rs"));
    assert!(entries[0].3 > 0);
}

#[test]
fn create_logger_registers_named_logger_for_later_lookup() {
    let sink = MemorySink::default();
    let name = format!(
        "registered_acceptance_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let logger = create_logger(
        LoggerBuilder::new()
            .name(&name)
            .level(LogLevel::Debug)
            .logger_type(LoggerType::Sync)
            .formatter(Formatter::new(Some("%c:%m%n".to_string())))
            .sink(Arc::new(sink.clone())),
    );

    let found = get_logger(&name).expect("registered logger should be available by name");
    LOG_INFO!(found, "hello from registry");

    assert_eq!(logger.name(), name);
    assert_eq!(
        sink.entries(),
        vec![format!("{}:hello from registry\n", name)]
    );
}

#[test]
fn file_sink_appends_formatted_logs_to_disk() {
    let dir = unique_temp_dir("file");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("app.log");

    let logger = LoggerBuilder::new()
        .name("file_acceptance")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .formatter(Formatter::new(Some("[%p] %m%n".to_string())))
        .sink(SinkFactory::file(file.to_str().unwrap()))
        .build();

    LOG_INFO!(logger, "file sink writes info");
    LOG_ERROR!(logger, "file sink writes error");

    let content = fs::read_to_string(&file).unwrap();
    assert!(content.contains("[INFO] file sink writes info"));
    assert!(content.contains("[ERROR] file sink writes error"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rolling_sink_splits_and_compresses_old_log_files() {
    let dir = unique_temp_dir("rolling");
    fs::create_dir_all(&dir).unwrap();
    let base = dir.join("roll_");

    let logger = LoggerBuilder::new()
        .name("rolling_acceptance")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .formatter(Formatter::new(Some("%m%n".to_string())))
        .sink(SinkFactory::rolling(base.to_str().unwrap(), 128))
        .build();

    for i in 0..30 {
        LOG_INFO!(
            logger,
            "rolling log message {} with enough text to trigger file split",
            i
        );
    }

    thread::sleep(Duration::from_millis(300));

    let files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();

    assert!(files
        .iter()
        .any(|path| path.extension().is_some_and(|ext| ext == "log")));
    assert!(files
        .iter()
        .any(|path| path.extension().is_some_and(|ext| ext == "gz")));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn global_stats_records_logs_that_pass_level_filter() {
    let before_total = bitlog::stats::GLOBAL_STATS
        .total
        .load(std::sync::atomic::Ordering::Relaxed);
    let before_info = bitlog::stats::GLOBAL_STATS
        .info
        .load(std::sync::atomic::Ordering::Relaxed);

    let sink = MemorySink::default();
    let logger = LoggerBuilder::new()
        .name("stats_acceptance")
        .level(LogLevel::Info)
        .logger_type(LoggerType::Sync)
        .sink(Arc::new(sink))
        .build();

    LOG_DEBUG!(logger, "filtered debug");
    LOG_INFO!(logger, "counted info");
    LOG_INFO!(logger, "counted info again");

    let after_total = bitlog::stats::GLOBAL_STATS
        .total
        .load(std::sync::atomic::Ordering::Relaxed);
    let after_info = bitlog::stats::GLOBAL_STATS
        .info
        .load(std::sync::atomic::Ordering::Relaxed);

    assert!(after_total - before_total >= 2);
    assert!(after_info - before_info >= 2);
    assert!(bitlog::stats::GLOBAL_STATS.qps() >= 0.0);
}

#[test]
fn json_formatter_outputs_structured_json_lines() {
    let sink = MemorySink::default();
    let logger = LoggerBuilder::new()
        .name("json_acceptance")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .json_formatter()
        .sink(Arc::new(sink.clone()))
        .build();

    LOG_INFO!(logger, "json formatter works");

    let entries = sink.entries();
    let parsed: Value = serde_json::from_str(entries[0].trim()).unwrap();
    assert_eq!(parsed["logger_name"], "json_acceptance");
    assert_eq!(parsed["level"], "INFO");
    assert_eq!(parsed["message"], "json formatter works");
}

#[test]
fn logger_level_can_be_changed_after_registration() {
    let sink = MemorySink::default();
    let name = format!(
        "dynamic_level_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let logger = create_logger(
        LoggerBuilder::new()
            .name(&name)
            .level(LogLevel::Error)
            .logger_type(LoggerType::Sync)
            .formatter(Formatter::new(Some("%p:%m%n".to_string())))
            .sink(Arc::new(sink.clone())),
    );

    LOG_WARN!(logger, "filtered warn");
    assert!(bitlog::set_logger_level(&name, LogLevel::Warn));
    LOG_WARN!(logger, "visible warn");

    assert_eq!(sink.entries(), vec!["WARN:visible warn\n"]);
}

#[test]
fn config_loads_default_values_without_file() {
    let config = bitlog::config::BitLogConfig::load_or_default("missing-bitlog.toml");
    assert_eq!(config.logger_level(), LogLevel::Debug);
    assert_eq!(config.dashboard.port, 8080);
    assert!(config.mysql.enabled);
}

#[test]
fn recent_log_store_keeps_structured_entries_for_dashboard() {
    let sink = MemorySink::default();
    let logger = LoggerBuilder::new()
        .name("recent_acceptance")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .formatter(Formatter::new(Some("[%p] %m%n".to_string())))
        .sink(Arc::new(sink))
        .build();

    LOG_WARN!(logger, "dashboard searchable warning");

    let entries = bitlog::recent::recent(20, Some(LogLevel::Warn), Some("searchable"));
    assert!(entries.iter().any(|entry| {
        entry.logger_name == "recent_acceptance"
            && entry.level == LogLevel::Warn
            && entry.message.contains("dashboard searchable warning")
    }));
}

#[test]
fn rolling_sink_supports_time_policy_and_cleanup_options() {
    let dir = unique_temp_dir("rolling_policy");
    fs::create_dir_all(&dir).unwrap();
    let base = dir.join("policy_");

    let logger = LoggerBuilder::new()
        .name("rolling_policy_acceptance")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .formatter(Formatter::new(Some("%m%n".to_string())))
        .sink(SinkFactory::rolling_with_cleanup(
            base.to_str().unwrap(),
            RollingPolicy::SizeAndTime {
                max_size: 96,
                time: TimeRollingPolicy::Hourly,
            },
            CleanupPolicy {
                max_age: None,
                max_files: Some(3),
            },
        ))
        .build();

    for i in 0..12 {
        LOG_INFO!(
            logger,
            "rolling policy log {} with enough bytes to rotate",
            i
        );
    }

    thread::sleep(Duration::from_millis(400));

    let files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();

    assert!(!files.is_empty());
    assert!(files.len() <= 3);

    let _ = fs::remove_dir_all(&dir);
}
