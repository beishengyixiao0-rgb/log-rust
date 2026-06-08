use bitlog::sink::SinkFactory;
use bitlog::{
    Formatter, LogLevel, LoggerBuilder, LoggerType, LOG_DEBUG, LOG_ERROR, LOG_INFO, LOG_WARN,
};
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        bitlog::dashboard::start_dashboard().await;
    });

    let mut builder = LoggerBuilder::new()
        .name("dashboard_demo")
        .level(LogLevel::Debug)
        .logger_type(LoggerType::Async)
        .formatter(Formatter::new(Some(
            "[%d{%Y-%m-%d %H:%M:%S}][%p][%c] %m%n".to_string(),
        )))
        .sink(SinkFactory::stdout());

    match SinkFactory::mysql_default() {
        Ok(mysql_sink) => {
            println!("MySQL sink enabled: mysql://root:******@127.0.0.1:3306/bitlog.logs");
            builder = builder.sink(mysql_sink);
        }
        Err(e) => {
            eprintln!("MySQL sink disabled: {}", e);
            eprintln!("Dashboard still works with in-memory recent logs.");
        }
    }

    let logger = builder.build();

    println!("Dashboard demo running at http://127.0.0.1:8080/");
    println!("Press Ctrl+C to stop.");

    let mut i = 0;
    loop {
        if i % 2 == 0 {
            LOG_DEBUG!(logger, "debug heartbeat {}", i);
        }

        LOG_INFO!(logger, "dashboard event {} processed", i);

        if i % 4 == 0 {
            LOG_WARN!(logger, "slow request detected: {} ms", 120 + i * 3);
        }

        if i % 10 == 0 {
            LOG_ERROR!(logger, "simulated error for order {}", 10_000 + i);
        }

        i += 1;
        thread::sleep(Duration::from_secs(5));
    }
}
