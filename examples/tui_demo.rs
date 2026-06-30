//! 终端 TUI 演示

use bitlog::{
    LogLevel, LoggerBuilder, LoggerType, LOG_DEBUG, LOG_ERROR, LOG_FATAL, LOG_INFO, LOG_WARN,
};
use std::thread;
use std::time::Duration;

fn main() {
    let logger = LoggerBuilder::new()
        .name("tui_demo")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .build();

    LOG_INFO!(logger, "tui demo starting");
    LOG_DEBUG!(logger, "loading terminal widgets");
    LOG_INFO!(logger, "collecting runtime metrics");

    for i in 0..20 {
        LOG_DEBUG!(logger, "background sample {} captured", i);

        if i % 3 == 0 {
            LOG_INFO!(logger, "worker {} completed normally", i);
        }

        if i % 5 == 0 {
            LOG_WARN!(logger, "queue depth is high at sample {}", i);
        }

        if i == 12 || i == 18 {
            LOG_ERROR!(logger, "simulated request failure at sample {}", i);
        }

        thread::sleep(Duration::from_millis(20));
    }

    LOG_FATAL!(logger, "simulated fatal event for dashboard statistics");
    bitlog::tui::start_tui();
    bitlog::stats::GLOBAL_STATS.print_report();

    drop(logger);
}
