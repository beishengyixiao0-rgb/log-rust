//! 滚动文件输出示例

use bitlog::sink::SinkFactory;
use bitlog::{LogLevel, LoggerBuilder, LoggerType, LOG_INFO};
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== BitLog 滚动文件策略演示 ===");
    println!("依次演示按大小、按小时、按日期切分。");

    std::env::set_var("BITLOG_DEMO_FORCE_PERIOD_SPLIT", "1");

    println!("[1/3] 按大小切分：文件超过 1KB 后自动滚动，并压缩旧文件。");
    let size_logger = LoggerBuilder::new()
        .name("rolling_size_demo")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .sink(SinkFactory::rolling("logs/bitlog_size_", 1024))
        .build();

    for i in 0..100 {
        LOG_INFO!(size_logger, "size rolling log {}", i);
    }

    println!("\n[2/3] 按小时切分：演示模式下每次写入模拟进入下一个小时。");
    let hourly_logger = LoggerBuilder::new()
        .name("rolling_hourly_demo")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .sink(SinkFactory::rolling_with_policy(
            "logs/bitlog_hour_",
            bitlog::RollingPolicy::Time(bitlog::TimeRollingPolicy::Hourly),
        ))
        .build();

    for i in 0..5 {
        LOG_INFO!(hourly_logger, "hourly rolling demo log {}", i);
        thread::sleep(Duration::from_millis(10));
    }

    println!("\n[3/3] 按日期切分：演示模式下每次写入模拟进入下一天。");
    let daily_logger = LoggerBuilder::new()
        .name("rolling_daily_demo")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .sink(SinkFactory::rolling_with_policy(
            "logs/bitlog_day_",
            bitlog::RollingPolicy::Time(bitlog::TimeRollingPolicy::Daily),
        ))
        .build();

    for i in 0..5 {
        LOG_INFO!(daily_logger, "daily rolling demo log {}", i);
        thread::sleep(Duration::from_millis(10));
    }

    thread::sleep(Duration::from_millis(500));
}
