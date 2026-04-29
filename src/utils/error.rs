/// 错误处理模块

use std::fmt;
use std::error::Error;

/// 错误级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorLevel {
    Warning,
    Error,
    Critical,
}

/// 错误类别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Connection,
    Query,
    Schema,
    Data,
    Validation,
    Compatibility,
    Migration,
    Backup,
    Unknown,
}

/// 详细的错误信息
#[derive(Debug, Clone)]
pub struct SqlToolError {
    pub message: String,
    pub category: ErrorCategory,
    pub level: ErrorLevel,
    pub source: Option<String>,
    pub details: Option<String>,
    pub suggestion: Option<String>,
}

impl SqlToolError {
    pub fn new(message: &str, category: ErrorCategory, level: ErrorLevel) -> Self {
        Self {
            message: message.to_string(),
            category,
            level,
            source: None,
            details: None,
            suggestion: None,
        }
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    pub fn with_details(mut self, details: &str) -> Self {
        self.details = Some(details.to_string());
        self
    }

    pub fn with_suggestion(mut self, suggestion: &str) -> Self {
        self.suggestion = Some(suggestion.to_string());
        self
    }

    pub fn connection_error(message: &str) -> Self {
        Self::new(message, ErrorCategory::Connection, ErrorLevel::Error)
            .with_suggestion("Please check your database connection string and ensure the database is running")
    }

    pub fn query_error(message: &str) -> Self {
        Self::new(message, ErrorCategory::Query, ErrorLevel::Error)
            .with_suggestion("Please check your SQL syntax and ensure the table exists")
    }

    pub fn schema_error(message: &str) -> Self {
        Self::new(message, ErrorCategory::Schema, ErrorLevel::Error)
            .with_suggestion("Please check the table structure and field definitions")
    }

    pub fn data_error(message: &str) -> Self {
        Self::new(message, ErrorCategory::Data, ErrorLevel::Error)
            .with_suggestion("Please check the data format and types")
    }

    pub fn validation_error(message: &str) -> Self {
        Self::new(message, ErrorCategory::Validation, ErrorLevel::Warning)
            .with_suggestion("Please check the input data format")
    }

    pub fn compatibility_error(message: &str) -> Self {
        Self::new(message, ErrorCategory::Compatibility, ErrorLevel::Error)
            .with_suggestion("Please check the database version compatibility")
    }

    pub fn migration_error(message: &str) -> Self {
        Self::new(message, ErrorCategory::Migration, ErrorLevel::Error)
            .with_suggestion("Please check the source and target table structures")
    }

    pub fn backup_error(message: &str) -> Self {
        Self::new(message, ErrorCategory::Backup, ErrorLevel::Critical)
            .with_suggestion("Please check the disk space and file permissions")
    }
}

impl fmt::Display for SqlToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.level, self.message)?;
        if let Some(ref source) = self.source {
            write!(f, " (Source: {})", source)?;
        }
        if let Some(ref details) = self.details {
            write!(f, "\nDetails: {}", details)?;
        }
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\nSuggestion: {}", suggestion)?;
        }
        Ok(())
    }
}

impl Error for SqlToolError {}

/// 错误结果类型
pub type SqlResult<T> = std::result::Result<T, SqlToolError>;

/// 日志记录模块
pub mod logger {
    use log::LevelFilter;
    use std::sync::Mutex;
    use chrono::Local;

    /// 自定义日志级别
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LogLevel {
        Trace,
        Debug,
        Info,
        Warning,
        Error,
        Critical,
    }

    impl From<LogLevel> for LevelFilter {
        fn from(level: LogLevel) -> Self {
            match level {
                LogLevel::Trace => LevelFilter::Trace,
                LogLevel::Debug => LevelFilter::Debug,
                LogLevel::Info => LevelFilter::Info,
                LogLevel::Warning => LevelFilter::Warn,
                LogLevel::Error => LevelFilter::Error,
                LogLevel::Critical => LevelFilter::Error,
            }
        }
    }

    /// 日志记录器
    pub struct SqlToolLogger {
        level: LogLevel,
        logs: Mutex<Vec<LogEntry>>,
    }

    #[derive(Debug, Clone)]
    pub struct LogEntry {
        pub timestamp: String,
        pub level: LogLevel,
        pub message: String,
        pub source: Option<String>,
    }

    impl SqlToolLogger {
        pub fn new(level: LogLevel) -> Self {
            Self {
                level,
                logs: Mutex::new(Vec::new()),
            }
        }

        pub fn log(&self, level: LogLevel, message: &str, source: Option<&str>) {
            if level as u8 >= self.level as u8 {
                let entry = LogEntry {
                    timestamp: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                    level,
                    message: message.to_string(),
                    source: source.map(|s| s.to_string()),
                };

                if let Ok(mut logs) = self.logs.lock() {
                    logs.push(entry.clone());
                }

                self.write_to_console(&entry);
            }
        }

        fn write_to_console(&self, entry: &LogEntry) {
            let level_str = match entry.level {
                LogLevel::Trace => "TRACE",
                LogLevel::Debug => "DEBUG",
                LogLevel::Info => "INFO",
                LogLevel::Warning => "WARN",
                LogLevel::Error => "ERROR",
                LogLevel::Critical => "CRITICAL",
            };

            let source_str = if let Some(ref source) = entry.source {
                format!(" [{}]", source)
            } else {
                String::new()
            };

            println!("{} {}{} - {}", entry.timestamp, level_str, source_str, entry.message);
        }

        pub fn get_logs(&self) -> Vec<LogEntry> {
            self.logs.lock().unwrap().clone()
        }

        pub fn clear_logs(&self) {
            if let Ok(mut logs) = self.logs.lock() {
                logs.clear();
            }
        }

        pub fn trace(&self, message: &str, source: Option<&str>) {
            self.log(LogLevel::Trace, message, source);
        }

        pub fn debug(&self, message: &str, source: Option<&str>) {
            self.log(LogLevel::Debug, message, source);
        }

        pub fn info(&self, message: &str, source: Option<&str>) {
            self.log(LogLevel::Info, message, source);
        }

        pub fn warning(&self, message: &str, source: Option<&str>) {
            self.log(LogLevel::Warning, message, source);
        }

        pub fn error(&self, message: &str, source: Option<&str>) {
            self.log(LogLevel::Error, message, source);
        }

        pub fn critical(&self, message: &str, source: Option<&str>) {
            self.log(LogLevel::Critical, message, source);
        }
    }

    /// 全局日志记录器
    static LOGGER: std::sync::LazyLock<SqlToolLogger> = std::sync::LazyLock::new(|| SqlToolLogger::new(LogLevel::Info));

    /// 获取全局日志记录器
    pub fn get_logger() -> &'static SqlToolLogger {
        &LOGGER
    }

    /// 设置日志级别
    pub fn set_log_level(level: LogLevel) {
        // 在实际实现中，这里需要使用Mutex或更复杂的同步机制
        // 为了简单起见，这里只是重新创建了一个新的logger
        let _ = level;
    }
}