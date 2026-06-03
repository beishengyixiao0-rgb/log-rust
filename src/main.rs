use bitlog::{
    Formatter, LogLevel, LoggerBuilder, LoggerType, LOG_DEBUG, LOG_ERROR, LOG_INFO, LOG_WARN,
};

use bitlog::sink::SinkFactory;

use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // ====================================
    // panic hook
    // ====================================
    bitlog::panic_hook::install_panic_hook();

    // ====================================
    // dashboard
    // ====================================
    tokio::spawn(async {
        bitlog::dashboard::start_dashboard().await;
    });

    tokio::spawn(async {
        bitlog::tui::start_tui();
    });
    // ====================================
    // logger
    // ====================================
    let logger = LoggerBuilder::new()
        .name("bitlog")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Async)
        .formatter(Formatter::new(Some(
            "[%d{%Y-%m-%d %H:%M:%S}]\
            [%t]\
            [%p]\
            [%c] \
            %m%n"
                .to_string(),
        )))
        .sink(SinkFactory::stdout())
        .build();

    // ====================================
    // test loop
    // ====================================

    for i in 0..50 {
        LOG_DEBUG!(logger, "debug message {}", i);

        LOG_INFO!(logger, "info message {}", i);

        LOG_WARN!(logger, "warn message {}", i);

        LOG_ERROR!(logger, "error message {}", i);

        thread::sleep(Duration::from_millis(100));
    }

    // 等待异步线程刷盘
    thread::sleep(Duration::from_secs(2));
    // 输出统计信息
    bitlog::stats::GLOBAL_STATS.print_report();
    println!("test finished");
    // let mut i = 0;
    // loop {
    //     LOG_DEBUG!(logger, "debug message {}", i);
    //     LOG_INFO!(logger, "info message {}", i);
    //     LOG_WARN!(logger, "warn message {}", i);
    //     LOG_ERROR!(logger, "error message {}", i);

    //     i += 1;

    //     thread::sleep(Duration::from_millis(100));

    //     // 测试 panic hook
    //     if i == 200 {
    //         panic!("test panic hook");
    //     }
    // }
}
