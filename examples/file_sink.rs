//! 文件输出目标和滚动文件输出目标演示

use bitlog::sink::SinkFactory;
use bitlog::{Formatter, LogLevel, LoggerBuilder, LoggerType, LOG_ERROR, LOG_INFO, LOG_WARN};
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== BitLog 文件输出示例 ===\n");

    // 创建带文件输出目标的日志器
    let file_logger = LoggerBuilder::new()
        .name("file_logger")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .formatter(Formatter::new(Some(
            "[%d{%Y-%m-%d %H:%M:%S}][%p][%c] %m%n".to_string(),
        )))
        .sink(SinkFactory::file("logs/bitlog_example.log"))
        .build();

    LOG_INFO!(file_logger, "写入文件：logs/bitlog_example.log");
    LOG_WARN!(file_logger, "这是文件中的一条警告");
    LOG_ERROR!(file_logger, "这是文件中的一条错误");

    // 创建带滚动文件输出目标的日志器（每个文件最大 1KB）
    let roll_logger = LoggerBuilder::new()
        .name("rolling_logger")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Async)
        .sink(SinkFactory::rolling("logs/bitlog_roll_", 1024))
        .build();

    println!("正在写入 100 条日志到滚动文件...");
    for i in 0..100 {
        LOG_INFO!(
            roll_logger,
            "滚动日志 #{} - 这是一些测试数据用于填充文件",
            i
        );
        thread::sleep(Duration::from_millis(5));
    }

    println!("等待异步日志器刷新...");

    // 给异步日志器时间刷新
    thread::sleep(Duration::from_secs(2));

    println!("\n文件日志写入成功！");
    println!("查看 logs/bitlog_example.log 获取文件输出目标结果");
    println!("查看 logs/bitlog_roll_*.log 获取滚动输出目标结果");
    println!("\n=== 文件输出示例完成 ===");
}
