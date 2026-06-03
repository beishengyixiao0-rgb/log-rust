# BitLog - Rust Version

一个同步和异步日志系统的 Rust 实现。


## 运行示例

# 基本用法
cargo run --example basic

# 异步日志
cargo run --example async_demo

# 文件输出
cargo run --example file_sink


## 文件结构

```
bitlog-rust/
├── Cargo.toml              # 项目配置和依赖
├── README.md               # 使用说明
├── PROJECT_SUMMARY.md      # 本文件
├── src/
│   ├── lib.rs              # 库入口和宏导出
│   ├── level.rs            # 日志级别定义
│   ├── message.rs          # 日志消息结构
│   ├── buffer.rs           # 内存缓冲区
│   ├── formatter.rs        # 日志格式化器
│   ├── sink.rs             # 日志输出 (stdout/文件/滚动)
│   ├── looper.rs           # 异步日志后台线程
│   └── logger.rs           # 日志器核心实现和宏
│   ├── stats.rs            # 实时统计
│   ├── panic_hook.rs       # panic保护
│   ├── compressor.rs       # gzip归档
│   ├── dashboard.rs        # Web Dashboard
│   ├── tui.rs              # 终端TUI
└── examples/
    ├── basic.rs            # 基本使用示例
    ├── async_demo.rs       # 异步日志示例
    └── file_sink.rs        # 文件输出示例
```

## 功能特性

- ✅ 同步日志器 (SyncLogger)
- ✅ 异步日志器 (AsyncLogger) - 使用后台线程
- ✅ 多种日志输出 (Sink)
  - 标准输出 (StdoutSink)
  - 文件输出 (FileSink)
  - 滚动文件 (RollSink) - 按大小自动切分
- ✅ 自定义日志格式
- ✅ 日志级别：DEBUG, INFO, WARN, ERROR, FATAL
- ✅ 线程安全
- ✅ 零成本抽象 - 使用 Rust 的所有权系统

## 快速开始

### 添加依赖

```toml
[dependencies]
bitlog = { path = "./bitlog-rust" }
```

### 基本使用

```rust
use bitlog::{debug, info, warn, error};

fn main() {
    // 使用根日志器
    debug!("调试信息");
    info!("用户 {} 登录", "Alice");
    warn!("警告信息");
    error!("错误：{}", " Something went wrong");
}
```

### 创建自定义日志器

```rust
use bitlog::{LoggerBuilder, LoggerType, LogLevel, Formatter};
use bitlog::sink::SinkFactory;

// 同步日志器 - 输出到文件
let logger = LoggerBuilder::new()
    .name("my_logger")
    .level(LogLevel::Info)
    .logger_type(LoggerType::Sync)
    .formatter(Formatter::new(Some(
        "[%d{%Y-%m-%d %H:%M:%S}][%p][%c] %m%n".to_string()
    )))
    .sink(SinkFactory::file("app.log"))
    .build();

// 异步日志器 - 高性能
let async_logger = LoggerBuilder::new()
    .name("async_logger")
    .logger_type(LoggerType::Async)
    .sink(SinkFactory::stdout())
    .build();
```

### 使用特定日志器

```rust
use bitlog::{LOG_INFO, LOG_DEBUG, get_logger};

let logger = get_logger("my_logger").unwrap();
LOG_INFO!(logger, "这是一条日志");
LOG_DEBUG!(logger, "调试：{}", 42);
```

## 日志格式

支持的格式说明符：

| 格式符 | 说明                                    |
| ------ | --------------------------------------- |
| `%d`   | 日期时间 (可用 `%d{format}` 自定义格式) |
| `%T`   | 制表符                                  |
| `%t`   | 线程 ID                                 |
| `%p`   | 日志级别                                |
| `%c`   | 日志器名称                              |
| `%f`   | 源文件                                  |
| `%l`   | 源行号                                  |
| `%m`   | 日志消息                                |
| `%n`   | 换行                                    |

默认格式：`[%d{%H:%M:%S}][%t][%p][%c][%f:%l] %m%n`

示例输出：
```
[14:30:45][1][INFO][root][main.rs:42] User Alice logged in
```

## 运行示例

```bash
cd bitlog-rust

# 基本示例
cargo run --example basic

# 异步日志示例
cargo run --example async_demo

# 文件输出示例
cargo run --example file_sink
```

# BitLog Rust 快速开始

## 1. 编译项目

```bash
cd /home/admin/openclaw/workspace/bitlog-rust
cargo build --release
```

## 2. 运行示例

```bash
# 基本用法
cargo run --example basic

# 异步日志
cargo run --example async_demo

# 文件输出
cargo run --example file_sink
```

## 3. 在你的项目中使用

### Cargo.toml
```toml
[dependencies]
bitlog = { path = "./bitlog-rust" }
```

### main.rs
```rust
use bitlog::{debug, info, warn, error};

fn main() {
    info!("Hello, BitLog!");
    debug!("调试：{}", 42);
    warn!("警告信息");
    error!("错误：{}", "something wrong");
}
```

## 4. 常用 API

### 日志宏 (使用根日志器)
```rust
debug!("格式：{}", 参数);
info!("格式：{}", 参数);
warn!("格式：{}", 参数);
error!("格式：{}", 参数);
fatal!("格式：{}", 参数);
```

### 创建自定义日志器
```rust
use bitlog::{LoggerBuilder, LoggerType, LogLevel};
use bitlog::sink::SinkFactory;

let logger = LoggerBuilder::new()
    .name("my_app")
    .level(LogLevel::Info)           // 设置级别
    .logger_type(LoggerType::Async)  // 同步/异步
    .sink(SinkFactory::file("app.log"))  // 输出到文件
    .build();
```

### 使用自定义日志器
```rust
use bitlog::LOG_INFO;

LOG_INFO!(logger, "消息内容");
```

## 5. 日志格式

默认格式：`[%d{%H:%M:%S}][%t][%p][%c][%f:%l] %m%n`

示例输出：
```
[14:30:45][1][INFO][root][main.rs:42] User Alice logged in
```

自定义格式：
```rust
use bitlog::Formatter;

let formatter = Formatter::new(Some(
    "%d{%Y-%m-%d %H:%M:%S} [%p] %m%n".to_string()
));
```

## 6. 日志级别

| 级别  | 说明 | 使用场景             |
| ----- | ---- | -------------------- |
| DEBUG | 调试 | 开发时详细调试信息   |
| INFO  | 信息 | 正常运行的关键信息   |
| WARN  | 警告 | 潜在问题但不影响运行 |
| ERROR | 错误 | 错误但程序继续运行   |
| FATAL | 致命 | 严重错误程序可能终止 |

## 7. 输出目标 (Sink)

```rust
// 标准输出
SinkFactory::stdout()

// 文件
SinkFactory::file("app.log")

// 滚动文件 (每个文件最大 10MB)
SinkFactory::rolling("logs/app_", 10 * 1024 * 1024)
```

## 8. 性能提示

- **高性能场景**：使用 `LoggerType::Async`
- **低延迟场景**：使用 `LoggerType::Sync`
- **多文件输出**：添加多个 sink
- **减少分配**：避免在日志宏中创建临时字符串
