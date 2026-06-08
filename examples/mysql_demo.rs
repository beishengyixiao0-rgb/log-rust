use bitlog::sink::SinkFactory;
use bitlog::{Formatter, LogLevel, LoggerBuilder, LoggerType, LOG_ERROR, LOG_INFO, LOG_WARN};

fn main() {
    let mysql_sink = SinkFactory::mysql_default()
        .expect("failed to connect MySQL at mysql://root:******@127.0.0.1:3306/bitlog");

    let logger = LoggerBuilder::new()
        .name("mysql_demo")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Sync)
        .formatter(Formatter::new(Some(
            "[%d{%Y-%m-%d %H:%M:%S}][%p][%c] %m%n".to_string(),
        )))
        .sink(SinkFactory::stdout())
        .sink(mysql_sink)
        .build();

    LOG_INFO!(logger, "mysql demo started");
    LOG_INFO!(logger, "insert structured info log");
    LOG_WARN!(logger, "insert structured warn log");
    LOG_ERROR!(logger, "insert structured error log");
    LOG_INFO!(logger, "mysql demo finished");

    println!("Inserted demo logs into bitlog.logs");
}
