//! panic 捕获和统计输出演示

use bitlog::sink::SinkFactory;
use bitlog::{info, warn, Formatter, LogLevel, LoggerBuilder, LoggerType, LOG_ERROR, LOG_INFO};
use std::fs;
use std::fs::File;

fn main() {
    bitlog::panic_hook::install_panic_hook();

    let logger = LoggerBuilder::new()
        .name("panic_stats_demo")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .formatter(Formatter::new(Some("[%p][%c] %m%n".to_string())))
        .sink(SinkFactory::stdout())
        .build();

    LOG_INFO!(logger, "starting panic/stats demo");
    info!("root logger also works");
    warn!("warnings are counted by GLOBAL_STATS");
    LOG_ERROR!(logger, "an error that contributes to the error rate");

    bitlog::stats::GLOBAL_STATS.print_report();

    println!("Now triggering a controlled file write failure...");
    trigger_file_write_failure();
}

fn trigger_file_write_failure() {
    let target = "logs/panic_stats_write_failure_target.log";
    fs::create_dir_all("logs").expect("failed to prepare logs directory");
    fs::create_dir_all(target).expect("failed to prepare file write failure demo directory");

    let _file = File::open(target).expect("file open failure demo");
}
