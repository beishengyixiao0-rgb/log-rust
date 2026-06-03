//! 异步日志器演示

use bitlog::{LoggerBuilder, LoggerType, LogLevel, Formatter, LOG_INFO, LOG_DEBUG};
use bitlog::sink::SinkFactory;
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== BitLog 异步日志器示例 ===\n");

    // 创建带自定义配置的异步日志器
    let logger = LoggerBuilder::new()
        .name("async_logger")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Async)
        .formatter(Formatter::new(Some(
            "[%d{%Y-%m-%d %H:%M:%S}][%t][%p][%c] %m%n".to_string()
        )))
        .sink(SinkFactory::stdout())
        .build();

    println!("异步日志器已创建：{}\n", logger.name());

    // 从主线程写入日志
    LOG_INFO!(logger, "开始异步日志演示");
    LOG_DEBUG!(logger, "这是来自主线程的调试");

    // 从多个线程写入日志
    let logger_clone = logger.clone();
    let handle1 = thread::spawn(move || {
        for i in 0..5 {
            LOG_INFO!(logger_clone, "线程 1 - 消息 {}", i);
            thread::sleep(Duration::from_millis(10));
        }
    });

    let logger_clone = logger.clone();
    let handle2 = thread::spawn(move || {
        for i in 0..5 {
            LOG_INFO!(logger_clone, "线程 2 - 消息 {}", i);
            thread::sleep(Duration::from_millis(15));
        }
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    LOG_INFO!(logger, "所有线程已完成");

    // 给异步日志器时间刷新
    thread::sleep(Duration::from_millis(100));

    println!("\n=== 异步示例完成 ===");
}
