/// 性能优化模块

use crate::utils::error::SqlResult;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};

/// 批处理配置
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub parallel_batches: usize,
    pub commit_interval: usize,
    pub timeout: Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            parallel_batches: 4,
            commit_interval: 100,
            timeout: Duration::from_secs(300),
        }
    }
}

impl BatchConfig {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            ..Default::default()
        }
    }

    pub fn with_parallel_batches(mut self, parallel_batches: usize) -> Self {
        self.parallel_batches = parallel_batches;
        self
    }

    pub fn with_commit_interval(mut self, commit_interval: usize) -> Self {
        self.commit_interval = commit_interval;
        self
    }
}

/// 进度跟踪器
pub struct ProgressTracker {
    total_rows: usize,
    processed_rows: Arc<Mutex<usize>>,
    start_time: Instant,
    batch_size: usize,
}

impl ProgressTracker {
    pub fn new(total_rows: usize, batch_size: usize) -> Self {
        Self {
            total_rows,
            processed_rows: Arc::new(Mutex::new(0)),
            start_time: Instant::now(),
            batch_size,
        }
    }

    pub async fn increment(&self, count: usize) {
        let mut processed = self.processed_rows.lock().await;
        *processed += count;
    }

    pub async fn get_progress(&self) -> ProgressInfo {
        let processed = *self.processed_rows.lock().await;
        let elapsed = self.start_time.elapsed();
        let rate = if elapsed.as_secs() > 0 {
            processed as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };
        let remaining = if rate > 0.0 {
            Duration::from_secs_f64((self.total_rows - processed) as f64 / rate)
        } else {
            Duration::from_secs(0)
        };

        ProgressInfo {
            total: self.total_rows,
            processed,
            percentage: if self.total_rows > 0 {
                (processed as f64 / self.total_rows as f64) * 100.0
            } else {
                100.0
            },
            elapsed,
            remaining,
            rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub total: usize,
    pub processed: usize,
    pub percentage: f64,
    pub elapsed: Duration,
    pub remaining: Duration,
    pub rate: f64,
}

impl ProgressInfo {
    pub fn format(&self) -> String {
        format!(
            "Progress: {:.2}% ({}/{}) | Rate: {:.2} rows/s | Elapsed: {:?} | Remaining: {:?}",
            self.percentage, self.processed, self.total, self.rate, self.elapsed, self.remaining
        )
    }
}

/// 性能监控器
pub struct PerformanceMonitor {
    start_time: Instant,
    metrics: Arc<Mutex<PerformanceMetrics>>,
}

#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub total_rows: usize,
    pub successful_rows: usize,
    pub failed_rows: usize,
    pub total_bytes: usize,
    pub queries_executed: usize,
    pub retries: usize,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            metrics: Arc::new(Mutex::new(PerformanceMetrics::default())),
        }
    }

    pub async fn record_success(&self, rows: usize, bytes: usize) {
        let mut metrics = self.metrics.lock().await;
        metrics.total_rows += rows;
        metrics.successful_rows += rows;
        metrics.total_bytes += bytes;
    }

    pub async fn record_failure(&self, rows: usize) {
        let mut metrics = self.metrics.lock().await;
        metrics.total_rows += rows;
        metrics.failed_rows += rows;
    }

    pub async fn record_query(&self) {
        let mut metrics = self.metrics.lock().await;
        metrics.queries_executed += 1;
    }

    pub async fn record_retry(&self) {
        let mut metrics = self.metrics.lock().await;
        metrics.retries += 1;
    }

    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.lock().await.clone()
    }

    pub async fn get_summary(&self) -> PerformanceSummary {
        let metrics = self.get_metrics().await;
        let elapsed = self.start_time.elapsed();
        let metrics_clone = metrics.clone();
        let rate = if elapsed.as_secs() > 0 {
            metrics_clone.successful_rows as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        PerformanceSummary {
            metrics,
            elapsed,
            success_rate: if metrics_clone.total_rows > 0 {
                (metrics_clone.successful_rows as f64 / metrics_clone.total_rows as f64) * 100.0
            } else {
                100.0
            },
            throughput: rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub metrics: PerformanceMetrics,
    pub elapsed: Duration,
    pub success_rate: f64,
    pub throughput: f64,
}

impl PerformanceSummary {
    pub fn format(&self) -> String {
        format!(
            "Performance Summary:\n\
             - Total Rows: {}\n\
             - Successful: {} ({:.2}%)\n\
             - Failed: {}\n\
             - Retries: {}\n\
             - Total Bytes: {}\n\
             - Queries Executed: {}\n\
             - Elapsed Time: {:?}\n\
             - Throughput: {:.2} rows/s",
            self.metrics.total_rows,
            self.metrics.successful_rows,
            self.success_rate,
            self.metrics.failed_rows,
            self.metrics.retries,
            self.metrics.total_bytes,
            self.metrics.queries_executed,
            self.elapsed,
            self.throughput
        )
    }
}

/// 批量处理器
pub struct BatchProcessor {
    config: BatchConfig,
    progress: Option<ProgressTracker>,
    monitor: PerformanceMonitor,
}

impl BatchProcessor {
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            progress: None,
            monitor: PerformanceMonitor::new(),
        }
    }

    pub fn with_progress(mut self, total_rows: usize) -> Self {
        self.progress = Some(ProgressTracker::new(total_rows, self.config.batch_size));
        self
    }

    pub async fn process_batch<F, Fut>(&self, items: Vec<serde_json::Value>, processor: F) -> SqlResult<()>
    where
        F: Fn(Vec<serde_json::Value>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let _total_items = items.len();
        
        for chunk in items.chunks(self.config.batch_size) {
            let chunk_vec: Vec<serde_json::Value> = chunk.to_vec();
            let chunk_size = chunk_vec.len();
            
            match processor(chunk_vec).await {
                Ok(_) => {
                    self.monitor.record_success(chunk_size, 0).await;
                    if let Some(ref progress) = self.progress {
                        progress.increment(chunk_size).await;
                    }
                }
                Err(e) => {
                    self.monitor.record_failure(chunk_size).await;
                    return Err(crate::utils::SqlToolError::data_error(&format!(
                        "Batch processing failed: {}",
                        e
                    )));
                }
            }
        }
        
        Ok(())
    }

    pub async fn get_summary(&self) -> Option<PerformanceSummary> {
        Some(self.monitor.get_summary().await)
    }

    pub async fn get_progress(&self) -> Option<ProgressInfo> {
        if let Some(ref progress) = self.progress {
            Some(progress.get_progress().await)
        } else {
            None
        }
    }
}

/// 并行批处理器
pub struct ParallelBatchProcessor {
    config: BatchConfig,
    monitor: PerformanceMonitor,
}

impl ParallelBatchProcessor {
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            monitor: PerformanceMonitor::new(),
        }
    }

    pub async fn process_parallel<F, Fut>(&self, items: Vec<serde_json::Value>, processor: F) -> SqlResult<()>
    where
        F: Fn(Vec<serde_json::Value>) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let chunk_size = (items.len() / self.config.parallel_batches).max(1);
        let chunks: Vec<_> = items.chunks(chunk_size).map(|c| c.to_vec()).collect();
        let processor = Arc::new(processor);
        
        let results: Vec<Result<()>> = {
            let mut handles = vec![];
            
            for chunk in chunks {
                let processor = processor.clone();
                let handle = tokio::spawn(async move {
                    processor(chunk).await
                });
                handles.push(handle);
            }
            
            let mut results = vec![];
            for handle in handles {
                match handle.await {
                    Ok(result) => results.push(result),
                    Err(e) => results.push(Err(anyhow::anyhow!("Task join error: {}", e))),
                }
            }
            results
        };
        
        let mut total_success = 0;
        let mut total_failure = 0;
        
        for result in results {
            match result {
                Ok(_) => total_success += 1,
                Err(_) => total_failure += 1,
            }
        }
        
        if total_failure > 0 {
            return Err(crate::utils::SqlToolError::data_error(&format!(
                "{} out of {} batches failed",
                total_failure,
                total_success + total_failure
            )));
        }
        
        Ok(())
    }
}

/// 连接池优化器
pub struct ConnectionPoolOptimizer {
    min_connections: usize,
    max_connections: usize,
    idle_timeout: Duration,
    max_lifetime: Duration,
}

impl Default for ConnectionPoolOptimizer {
    fn default() -> Self {
        Self {
            min_connections: 2,
            max_connections: 10,
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(3600),
        }
    }
}

impl ConnectionPoolOptimizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_min_connections(mut self, min: usize) -> Self {
        self.min_connections = min;
        self
    }

    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    pub fn with_max_lifetime(mut self, lifetime: Duration) -> Self {
        self.max_lifetime = lifetime;
        self
    }
}