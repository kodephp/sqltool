/// 操作结果模块 - 统一的操作返回类型

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
    pub duration: Duration,
    pub details: Option<OperationDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationDetails {
    Transfer(TransferDetails),
    Backup(BackupDetails),
    Compare(CompareDetails),
    Sync(SyncDetails),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferDetails {
    pub rows_transferred: u64,
    pub rows_failed: u64,
    pub bytes_transferred: u64,
    pub tables_transferred: u32,
    pub verification_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupDetails {
    pub backup_path: String,
    pub size_bytes: u64,
    pub tables_backed_up: u32,
    pub compression_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareDetails {
    pub tables_compared: u32,
    pub differences_found: u32,
    pub differences_fixed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDetails {
    pub records_synced: u64,
    pub conflicts_resolved: u64,
    pub last_sync_time: Option<String>,
}

impl OperationResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            duration: Duration::from_secs(0),
            details: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            duration: Duration::from_secs(0),
            details: None,
        }
    }

    pub fn with_details(mut self, details: OperationDetails) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| r#"{"error":" serialization failed"}"#.to_string())
    }
}

pub struct OperationTimer {
    start: Instant,
    name: String,
}

impl OperationTimer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            name: name.into(),
        }
    }

    pub fn finish(self) -> Duration {
        self.start.elapsed()
    }

    pub fn finish_with_result(self) -> (Duration, OperationResult) {
        let duration = self.start.elapsed();
        let result = OperationResult {
            success: true,
            message: format!("{} completed", self.name),
            duration,
            details: None,
        };
        (duration, result)
    }
}

impl Drop for OperationTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        if elapsed > Duration::from_secs(5) {
            log::warn!("Operation {} took {:?}", self.name, elapsed);
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperationBuilder {
    name: String,
    start: Option<Instant>,
}

impl OperationBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start: None,
        }
    }

    pub fn start(&mut self) -> &mut Self {
        self.start = Some(Instant::now());
        self
    }

    pub fn finish(&self) -> OperationResult {
        let duration = self.start
            .map(|s| s.elapsed())
            .unwrap_or_default();

        OperationResult {
            success: true,
            message: format!("{} completed", self.name),
            duration,
            details: None,
        }
    }

    pub fn finish_with_error(&self, msg: impl Into<String>) -> OperationResult {
        OperationResult {
            success: false,
            message: msg.into(),
            duration: self.start.map(|s| s.elapsed()).unwrap_or_default(),
            details: None,
        }
    }
}

pub trait IntoOperationResult<T> {
    fn into_result(self, operation: &str) -> OperationResult;
}

impl<T, E: std::fmt::Display> IntoOperationResult<T> for std::result::Result<T, E> {
    fn into_result(self, operation: &str) -> OperationResult {
        match self {
            Ok(_) => OperationResult::success(operation),
            Err(e) => OperationResult::failure(format!("{} failed: {}", operation, e)),
        }
    }
}
