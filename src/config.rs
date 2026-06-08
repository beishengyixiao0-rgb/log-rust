use crate::level::LogLevel;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct BitLogConfig {
    pub logger: LoggerConfig,
    pub dashboard: DashboardConfig,
    pub mysql: MysqlConfig,
    pub rolling: RollingConfig,
    pub alert: AlertConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggerConfig {
    pub level: String,
    pub async_logger: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashboardConfig {
    pub host: String,
    pub port: u16,
    pub recent_limit: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MysqlConfig {
    pub enabled: bool,
    pub url: String,
    pub table: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RollingConfig {
    pub max_size: usize,
    pub max_files: Option<usize>,
    pub max_age_days: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertConfig {
    pub error_count_threshold: u64,
    pub error_rate_threshold: f64,
}

impl Default for BitLogConfig {
    fn default() -> Self {
        Self {
            logger: LoggerConfig {
                level: "DEBUG".to_string(),
                async_logger: true,
            },
            dashboard: DashboardConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                recent_limit: 1000,
            },
            mysql: MysqlConfig {
                enabled: true,
                url: "mysql://root:484236@127.0.0.1:3306/bitlog".to_string(),
                table: "logs".to_string(),
            },
            rolling: RollingConfig {
                max_size: 10 * 1024 * 1024,
                max_files: Some(30),
                max_age_days: Some(7),
            },
            alert: AlertConfig {
                error_count_threshold: 10,
                error_rate_threshold: 0.10,
            },
        }
    }
}

impl BitLogConfig {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Self::load_from_file(path).unwrap_or_default()
    }

    pub fn logger_level(&self) -> LogLevel {
        LogLevel::from_str(&self.logger.level).unwrap_or(LogLevel::Debug)
    }

    pub fn max_age(&self) -> Option<Duration> {
        self.rolling
            .max_age_days
            .map(|days| Duration::from_secs(days * 24 * 60 * 60))
    }
}
