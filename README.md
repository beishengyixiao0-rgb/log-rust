# BitLog - Rust 日志系统

BitLog 是一个用 Rust 实现的日志系统。它可以在程序运行时记录不同级别的日志，并把日志输出到终端、文件、滚动文件和 MySQL 数据库。同时，项目提供了一个 Web Dashboard，用来查看日志统计、实时日志和数据库历史日志。

这个项目适合命令行程序、服务端程序和需要本地日志文件的 Rust 应用。它同时支持同步写日志和异步写日志：同步模式更直接，适合调试；异步模式使用后台线程写出日志，适合日志量较大的场景。

## 环境要求

- Rust 2021 edition
- Cargo
- MySQL 8.x 或兼容版本，用于数据库日志写入和历史日志查询
- 浏览器，用于访问 Web Dashboard

## 文件结构

```text
bitlog-rust/
├── Cargo.toml              # 项目配置和依赖
├── README.md               # 使用说明
├── src/
│   ├── lib.rs              # 库入口和宏导出
│   ├── level.rs            # 日志级别定义
│   ├── message.rs          # 日志消息结构
│   ├── buffer.rs           # 内存缓冲区
│   ├── formatter.rs        # 日志格式化器
│   ├── sink.rs             # 日志输出目标：终端、文件、滚动文件、MySQL
│   ├── looper.rs           # 异步日志后台线程
│   ├── logger.rs           # 日志器核心实现和日志宏
│   ├── stats.rs            # 日志统计
│   ├── panic_hook.rs       # panic 捕获和统计输出
│   ├── compressor.rs       # gzip 压缩归档
│   ├── dashboard.rs        # Web Dashboard
│   ├── recent.rs           # 最近日志缓存和查询
│   └── tui.rs              # 终端 TUI
├── examples/
    ├── basic.rs            # 基本使用示例
    ├── async_demo.rs       # 异步日志示例
    ├── file_sink.rs        # 文件和滚动文件示例
    ├── dashboard_demo.rs   # Dashboard + MySQL 示例
    └── mysql_demo.rs       # MySQL 写入验证示例
└── tests/
    └── acceptance.rs       # 集成测试
```

## 功能概览

BitLog 已实现这些功能：

- 同步日志器 `SyncLogger`
- 异步日志器 `AsyncLogger`
- 根日志器和命名日志器
- 日志宏：`debug!`、`info!`、`warn!`、`error!`、`fatal!`
- 指定日志器宏：`LOG_DEBUG!`、`LOG_INFO!`、`LOG_WARN!`、`LOG_ERROR!`、`LOG_FATAL!`
- 日志级别过滤：`DEBUG`、`INFO`、`WARN`、`ERROR`、`FATAL`、`OFF`
- 自定义日志格式
- JSON 结构化日志格式
- 标准输出 sink
- 文件 sink
- 滚动文件 sink
- 按大小、小时、日期滚动日志
- 旧日志 gzip 压缩归档
- 旧日志自动清理
- MySQL 结构化日志写入
- 最近日志内存缓存
- Web Dashboard 可视化页面
- Dashboard 实时日志查询
- Dashboard MySQL 历史日志查询
- Dashboard 手动生成日志
- Dashboard 动态修改 logger 级别
- Dashboard 导出当前日志为 JSON/TXT
- Dashboard 展示异步处理批次
- `bitlog.toml` 配置文件读取
- 全局日志统计：总数、各级别数量、QPS、错误率
- panic hook
- 终端 TUI

## 快速开始

### 添加依赖

如果在其他 Rust 项目中使用本地 BitLog：

```toml
[dependencies]
bitlog = { path = "./bitlog-rust" }
```

### 使用根日志器

```rust
use bitlog::{debug, info, warn, error};

fn main() {
    debug!("调试信息");
    info!("用户 {} 登录", "Alice");
    warn!("磁盘使用率较高");
    error!("请求失败：{}", 500);
}
```

根日志器默认输出到标准输出。

### 创建自定义日志器

```rust
use bitlog::{Formatter, LogLevel, LoggerBuilder, LoggerType};
use bitlog::sink::SinkFactory;

let logger = LoggerBuilder::new()
    .name("app")
    .level(LogLevel::Info)
    .logger_type(LoggerType::Sync)
    .formatter(Formatter::new(Some(
        "[%d{%Y-%m-%d %H:%M:%S}][%p][%c] %m%n".to_string()
    )))
    .sink(SinkFactory::stdout())
    .sink(SinkFactory::file("logs/app.log"))
    .build();
```

### 使用指定日志器写日志

```rust
use bitlog::{LOG_INFO, LOG_ERROR};

LOG_INFO!(logger, "服务启动成功");
LOG_ERROR!(logger, "处理订单 {} 失败", 10001);
```

### 注册和获取命名日志器

如果希望在程序不同位置按名称获取同一个日志器，需要用 `create_logger` 注册：

```rust
use bitlog::{create_logger, get_logger, LogLevel, LoggerBuilder, LoggerType};
use bitlog::sink::SinkFactory;

let logger = create_logger(
    LoggerBuilder::new()
        .name("database")
        .level(LogLevel::Info)
        .logger_type(LoggerType::Async)
        .sink(SinkFactory::stdout())
);

let same_logger = get_logger("database").unwrap();
```

## 同步日志和异步日志

`LoggerType::Sync` 表示同步日志。调用日志宏时，日志会立即格式化并写入 sink。它适合调试、命令行工具、单元测试和需要写入后马上可见的场景。

```rust
let logger = LoggerBuilder::new()
    .name("sync_logger")
    .logger_type(LoggerType::Sync)
    .sink(SinkFactory::stdout())
    .build();
```

`LoggerType::Async` 表示异步日志。调用日志宏时，日志会先进入内存缓冲区，再由后台线程批量写出。它适合日志量比较大的场景，可以减少业务线程被 I/O 阻塞的时间。

```rust
let logger = LoggerBuilder::new()
    .name("async_logger")
    .logger_type(LoggerType::Async)
    .sink(SinkFactory::stdout())
    .build();
```

异步日志器对普通文本 sink 使用后台批量写入；对 MySQL 这类结构化 sink 会直接传递完整日志字段，保证数据库中能保存 logger 名称、级别、文件、行号和线程 ID。

## 日志级别

支持的日志级别如下：

| 级别  | 用途                         |
| ----- | ---------------------------- |
| DEBUG | 开发和排查问题时的详细信息   |
| INFO  | 程序正常运行过程中的关键信息 |
| WARN  | 潜在问题或需要关注的异常状态 |
| ERROR | 已发生错误，但程序仍可继续   |
| FATAL | 严重错误或不可恢复问题       |
| OFF   | 关闭日志输出                 |

日志器可以设置最小输出级别。低于该级别的日志不会写入 sink，也不会进入统计。

```rust
let logger = LoggerBuilder::new()
    .level(LogLevel::Warn)
    .sink(SinkFactory::stdout())
    .build();
```

上面的 logger 只会输出 `WARN`、`ERROR`、`FATAL`。

## 日志格式

默认格式：

```text
[%d{%H:%M:%S}][%t][%p][%c][%f:%l] %m%n
```

示例输出：

```text
[14:30:45][1][INFO][root][main.rs:42] User Alice logged in
```

支持的格式符：

| 格式符 | 说明                                   |
| ------ | -------------------------------------- |
| `%d`   | 日期时间，可用 `%d{format}` 自定义格式 |
| `%T`   | 制表符                                 |
| `%t`   | 线程 ID                                |
| `%p`   | 日志级别                               |
| `%c`   | 日志器名称                             |
| `%f`   | 源文件                                 |
| `%l`   | 源行号                                 |
| `%m`   | 日志消息                               |
| `%n`   | 换行                                   |

自定义格式：

```rust
let formatter = Formatter::new(Some(
    "%d{%Y-%m-%d %H:%M:%S} [%p] %m%n".to_string()
));
```

JSON 格式日志：

```rust
let logger = LoggerBuilder::new()
    .name("json_logger")
    .json_formatter()
    .sink(SinkFactory::file("logs/app.jsonl"))
    .build();
```

输出示例：

```json
{"file":"main.rs","level":"INFO","line":42,"logger_name":"json_logger","message":"hello","thread_id":123,"timestamp":1710000000}
```

## 输出到终端和文件

输出到终端：

```rust
SinkFactory::stdout()
```

输出到文件：

```rust
SinkFactory::file("logs/app.log")
```

一个 logger 可以添加多个 sink：

```rust
let logger = LoggerBuilder::new()
    .name("multi_sink")
    .sink(SinkFactory::stdout())
    .sink(SinkFactory::file("logs/app.log"))
    .build();
```

## 滚动日志和旧日志清理

按大小滚动：

```rust
SinkFactory::rolling("logs/app_", 10 * 1024 * 1024)
```

按小时滚动：

```rust
use bitlog::{RollingPolicy, TimeRollingPolicy};

SinkFactory::rolling_with_policy(
    "logs/app_",
    RollingPolicy::Time(TimeRollingPolicy::Hourly),
);
```

按日期滚动：

```rust
SinkFactory::rolling_with_policy(
    "logs/app_",
    RollingPolicy::Time(TimeRollingPolicy::Daily),
);
```

大小和时间组合滚动，任一条件满足就切分：

```rust
SinkFactory::rolling_with_policy(
    "logs/app_",
    RollingPolicy::SizeAndTime {
        max_size: 10 * 1024 * 1024,
        time: TimeRollingPolicy::Daily,
    },
);
```

配置旧日志清理：

```rust
use bitlog::{CleanupPolicy, RollingPolicy, TimeRollingPolicy};
use std::time::Duration;

SinkFactory::rolling_with_cleanup(
    "logs/app_",
    RollingPolicy::SizeAndTime {
        max_size: 10 * 1024 * 1024,
        time: TimeRollingPolicy::Daily,
    },
    CleanupPolicy {
        max_age: Some(Duration::from_secs(7 * 24 * 60 * 60)),
        max_files: Some(30),
    },
);
```

滚动时，旧日志文件会异步压缩为 `.gz` 归档。

## 输出到 MySQL

BitLog 可以把结构化日志写入 MySQL。默认连接信息是：

```text
地址：127.0.0.1:3306
数据库：bitlog
用户名：root
密码：484236
表名：logs
```

先创建数据库：

```sql
CREATE DATABASE IF NOT EXISTS bitlog DEFAULT CHARSET utf8mb4;
USE bitlog;
```

表结构：

```sql
CREATE TABLE IF NOT EXISTS logs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    logger_name VARCHAR(128),
    level VARCHAR(16),
    message TEXT,
    file VARCHAR(255),
    line INT,
    thread_id BIGINT,
    created_at DATETIME
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

使用默认 MySQL sink：

```rust
let mysql_sink = SinkFactory::mysql_default().unwrap();

let logger = LoggerBuilder::new()
    .name("mysql_logger")
    .level(LogLevel::Debug)
    .logger_type(LoggerType::Sync)
    .sink(mysql_sink)
    .build();
```

自定义连接：

```rust
let mysql_sink = SinkFactory::mysql(
    "mysql://root:484236@127.0.0.1:3306/bitlog",
    "logs"
).unwrap();
```

写入后可以用 SQL 查询：

```sql
SELECT id, logger_name, level, message, file, line, thread_id, created_at
FROM logs
ORDER BY id DESC
LIMIT 20;
```

单独验证 MySQL 写入：

```bash
cargo run --example mysql_demo
```

这个示例会写入 5 条日志后退出。

## Web Dashboard

Dashboard 是一个内置 HTML 页面，不需要 React、Vue 或 npm。启动后访问浏览器即可查看。

启动 Dashboard：

```rust
#[tokio::main]
async fn main() {
    bitlog::dashboard::start_dashboard().await;
}
```

运行示例：

```bash
cargo run --example dashboard_demo
```

浏览器打开：

```text
http://127.0.0.1:8080/
```

Dashboard 页面包含：

- 总日志数
- QPS
- WARN 数量
- ERROR 数量
- 错误率
- 运行状态
- 异步处理批次数
- 错误告警
- 各级别日志数量柱状图
- 实时日志列表
- MySQL 历史日志列表
- 实时日志和 MySQL 历史的当前显示条数
- 日志列表序号
- 级别筛选
- 关键字搜索
- 条数限制
- 暂停和继续刷新
- 手动生成 DEBUG / INFO / WARN / ERROR / FATAL 日志
- 动态修改 logger 日志级别
- 导出当前查询结果为 JSON
- 导出当前查询结果为 TXT

Dashboard API：

```text
GET /metrics
GET /logs/recent?limit=100&level=ERROR&q=order
GET /logs/mysql?limit=100&level=WARN&q=request
POST /logs/generate
POST /loggers/level
```

手动生成日志接口请求体：

```json
{"level":"WARN","message":"manual warning"}
```

动态修改 logger 级别接口请求体：

```json
{"logger":"root","level":"INFO"}
```

`dashboard_demo` 会每 5 秒生成一轮日志，并尝试连接 MySQL。如果连接成功，日志会同时写入终端、Dashboard 内存缓存和 MySQL；如果连接失败，控制台会打印错误，Dashboard 仍然可以展示实时日志。

## 配置文件

可以使用 `bitlog.toml` 保存常用配置。没有配置文件时，BitLog 会使用默认值。

示例：

```toml
[logger]
level = "DEBUG"
async_logger = true

[dashboard]
host = "0.0.0.0"
port = 8080
recent_limit = 1000

[mysql]
enabled = true
url = "mysql://root:484236@127.0.0.1:3306/bitlog"
table = "logs"

[rolling]
max_size = 10485760
max_files = 30
max_age_days = 7

[alert]
error_count_threshold = 10
error_rate_threshold = 0.10
```

读取配置：

```rust
let config = bitlog::config::BitLogConfig::load_or_default("bitlog.toml");
```

## 运行统计

BitLog 会统计已经通过级别过滤的日志：

- 日志总数
- DEBUG / INFO / WARN / ERROR / FATAL 各级别数量
- QPS
- 错误率

打印统计报告：

```rust
bitlog::stats::GLOBAL_STATS.print_report();
```

## panic hook

安装 panic hook：

```rust
bitlog::panic_hook::install_panic_hook();
```

程序 panic 时，会输出 panic 位置、payload 和当前日志统计。

## 终端 TUI

启动 TUI：

```rust
fn main() {
    bitlog::tui::start_tui();
}
```

当前 TUI 会显示基础运行指标，例如 QPS。

## 运行示例

```bash
# 基本用法
cargo run --example basic

# 异步日志
cargo run --example async_demo

# 文件输出和滚动文件
cargo run --example file_sink

# Dashboard 页面和 MySQL 写入
cargo run --example dashboard_demo

# 单独验证 MySQL 写入
cargo run --example mysql_demo
```

## 测试流程

运行全部测试：

```bash
cargo test
```

测试覆盖内容：

- 同步日志
- 异步多线程日志
- 结构化 sink
- 日志级别过滤
- 自定义格式
- 命名日志器注册和查找
- 文件输出
- 滚动文件压缩归档
- 增强滚动策略
- 最近日志查询
- 全局统计
- JSON formatter
- 配置文件默认加载
- 动态修改 logger 级别

普通测试不会强依赖 MySQL 服务是否启动。MySQL 写入可以通过下面命令单独验证：

```bash
cargo run --example mysql_demo
```

## 清理编译产物

如果需要清理 `target/` 下的编译缓存和可执行文件：

```bash
cargo clean
```

清理后源码不会受影响，但下一次 `cargo build`、`cargo test` 或 `cargo run` 会重新编译。

## 性能提示

- 高吞吐场景使用 `LoggerType::Async`。
- 需要写入后立即可见或便于调试时使用 `LoggerType::Sync`。
- 当前日志宏会先执行 `format!`，再进入日志级别过滤；如果日志被过滤，仍会产生格式化开销。
