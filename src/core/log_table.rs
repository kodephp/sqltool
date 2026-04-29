use crate::databases::DatabaseConnection;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTableConfig {
    pub table_name: String,
    pub partition_type: PartitionType,
    pub retention_days: u32,
    pub max_partition_size_gb: f64,
    pub auto_create_partition: bool,
    pub compression_enabled: bool,
    pub indexed_fields: Vec<String>,
    pub partition_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionType {
    Daily,
    Weekly,
    Monthly,
    Hourly,
    SizeBased { max_size_gb: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPartition {
    pub partition_name: String,
    pub table_name: String,
    pub start_date: String,
    pub end_date: Option<String>,
    pub row_count: u64,
    pub size_mb: f64,
    pub is_active: bool,
    pub compressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    pub table_name: String,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub level_filter: Vec<String>,
    pub keyword_filter: Option<String>,
    pub limit: u64,
    pub offset: u64,
    pub order_desc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQueryResult {
    pub total_matched: u64,
    pub results: Vec<serde_json::Value>,
    pub partitions_queried: Vec<String>,
    pub query_time_ms: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStats {
    pub table_name: String,
    pub total_partitions: usize,
    pub active_partitions: usize,
    pub total_size_gb: f64,
    pub total_rows: u64,
    pub compression_ratio: f64,
    pub oldest_partition: Option<String>,
    pub newest_partition: Option<String>,
}

pub struct LogTableManager {
    connection: Box<dyn DatabaseConnection>,
    configs: HashMap<String, LogTableConfig>,
    partitions: HashMap<String, Vec<LogPartition>>,
}

impl LogTableManager {
    pub fn new(connection: Box<dyn DatabaseConnection>) -> Self {
        Self {
            connection,
            configs: HashMap::new(),
            partitions: HashMap::new(),
        }
    }

    pub fn register_log_table(&mut self, config: LogTableConfig) {
        self.configs.insert(config.table_name.clone(), config);
    }

    pub async fn create_log_table(&mut self, table_name: &str) -> Result<()> {
        let create_sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id BIGINT AUTO_INCREMENT PRIMARY KEY,
                timestamp DATETIME NOT NULL,
                level VARCHAR(20) NOT NULL,
                message TEXT,
                context JSON,
                source VARCHAR(255),
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_timestamp (timestamp),
                INDEX idx_level (level),
                INDEX idx_created_at (created_at)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
            "#,
            table_name
        );

        self.connection.execute(&create_sql).await?;

        let initial_partition = LogPartition {
            partition_name: format!("{}_p0", table_name),
            table_name: table_name.to_string(),
            start_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            end_date: None,
            row_count: 0,
            size_mb: 0.0,
            is_active: true,
            compressed: false,
        };

        self.partitions
            .entry(table_name.to_string())
            .or_insert_with(Vec::new)
            .push(initial_partition);

        Ok(())
    }

    pub async fn insert_log(&mut self, table_name: &str, level: &str, message: &str, context: Option<serde_json::Value>, source: Option<&str>) -> Result<()> {
        let _config = self.configs.get(table_name)
            .ok_or_else(|| anyhow!("Log table {} not registered", table_name))?;

        let active_partition = self.get_active_partition(table_name).await?;

        let context_json = context.map(|c| serde_json::to_string(&c).unwrap_or_default())
            .unwrap_or_else(|| "null".to_string());

        let insert_sql = format!(
            "INSERT INTO {} (timestamp, level, message, context, source) VALUES (NOW(), '{}', '{}', '{}', '{}')",
            active_partition,
            level,
            message.replace('\'', "''"),
            context_json.replace('\'', "''"),
            source.unwrap_or("unknown").replace('\'', "''")
        );

        self.connection.execute(&insert_sql).await?;

        self.update_partition_stats(table_name, &active_partition).await?;

        self.check_and_create_partition(table_name).await?;

        Ok(())
    }

    pub async fn batch_insert_logs(&mut self, table_name: &str, logs: Vec<LogEntry>) -> Result<u64> {
        let _config = self.configs.get(table_name)
            .ok_or_else(|| anyhow!("Log table {} not registered", table_name))?;

        let active_partition = self.get_active_partition(table_name).await?;
        let mut inserted = 0u64;

        for log in logs {
            let context_json = log.context.map(|c| serde_json::to_string(&c).unwrap_or_default())
                .unwrap_or_else(|| "null".to_string());

            let insert_sql = format!(
                "INSERT INTO {} (timestamp, level, message, context, source) VALUES (NOW(), '{}', '{}', '{}', '{}')",
                active_partition,
                log.level,
                log.message.replace('\'', "''"),
                context_json.replace('\'', "''"),
                log.source.unwrap_or_else(|| "unknown".to_string()).replace('\'', "''")
            );

            if self.connection.execute(&insert_sql).await.is_ok() {
                inserted += 1;
            }
        }

        self.update_partition_stats(table_name, &active_partition).await?;
        self.check_and_create_partition(table_name).await?;

        Ok(inserted)
    }

    async fn get_active_partition(&self, table_name: &str) -> Result<String> {
        if let Some(partitions) = self.partitions.get(table_name) {
            if let Some(active) = partitions.iter().find(|p| p.is_active) {
                return Ok(active.partition_name.clone());
            }
        }
        Ok(format!("{}_p0", table_name))
    }

    async fn check_and_create_partition(&mut self, table_name: &str) -> Result<()> {
        let config = self.configs.get(table_name)
            .ok_or_else(|| anyhow!("Log table {} not registered", table_name))?;

        let active_partition = self.get_active_partition(table_name).await?;
        let partition_stats = self.get_partition_stats(table_name, &active_partition).await?;

        let should_create = match &config.partition_type {
            PartitionType::Daily => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                partition_stats.start_date != today
            }
            PartitionType::Hourly => {
                let hour = chrono::Utc::now().format("%Y-%m-%d %H:00:00").to_string();
                partition_stats.start_date != hour
            }
            PartitionType::SizeBased { max_size_gb } => {
                partition_stats.size_mb as f64 / 1024.0 >= *max_size_gb
            }
            _ => false,
        };

        if should_create && config.auto_create_partition {
            self.create_new_partition(table_name).await?;
        }

        Ok(())
    }

    async fn get_partition_stats(&self, table_name: &str, partition_name: &str) -> Result<LogPartition> {
        let sql = format!(
            "SELECT COUNT(*) as cnt, AVG(LENGTH(message)) as avg_size FROM {}",
            partition_name
        );

        let rows = self.connection.query(&sql).await?;
        let row_count = rows.first()
            .and_then(|r| r.get("cnt").and_then(|v| v.as_u64()))
            .unwrap_or(0);

        Ok(LogPartition {
            partition_name: partition_name.to_string(),
            table_name: table_name.to_string(),
            start_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            end_date: None,
            row_count,
            size_mb: 0.0,
            is_active: true,
            compressed: false,
        })
    }

    async fn create_new_partition(&mut self, table_name: &str) -> Result<()> {
        let config = self.configs.get(table_name)
            .ok_or_else(|| anyhow!("Log table {} not registered", table_name))?;

        let partitions = self.partitions.get(table_name);
        let partition_num = partitions.map(|p| p.len()).unwrap_or(0);

        let _new_partition_name = format!("{}_{}", config.partition_prefix, partition_num);

        let date_pattern = match &config.partition_type {
            PartitionType::Daily => chrono::Utc::now().format("%Y%m%d").to_string(),
            PartitionType::Weekly => chrono::Utc::now().format("%Y-W%V").to_string(),
            PartitionType::Monthly => chrono::Utc::now().format("%Y%m").to_string(),
            PartitionType::Hourly => chrono::Utc::now().format("%Y%m%d%H").to_string(),
            PartitionType::SizeBased { .. } => format!("{}", partition_num),
        };

        let partition_name = format!("{}_{}", config.partition_prefix, date_pattern);

        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {} LIKE {}",
            partition_name, table_name
        );

        self.connection.execute(&create_sql).await?;

        if let Some(partitions) = self.partitions.get_mut(table_name) {
            if let Some(last) = partitions.iter_mut().find(|p| p.is_active) {
                last.is_active = false;
                last.end_date = Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
            }

            partitions.push(LogPartition {
                partition_name: partition_name.clone(),
                table_name: table_name.to_string(),
                start_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                end_date: None,
                row_count: 0,
                size_mb: 0.0,
                is_active: true,
                compressed: false,
            });
        }

        Ok(())
    }

    async fn update_partition_stats(&self, table_name: &str, partition_name: &str) -> Result<()> {
        if let Some(partitions) = self.partitions.get(table_name) {
            if let Some(_stats) = partitions.iter().find(|p| &p.partition_name == partition_name) {
                let sql = format!("SELECT COUNT(*) as cnt FROM {}", partition_name);
                if let Ok(rows) = self.connection.query(&sql).await {
                    if let Some(_cnt) = rows.first().and_then(|r| r.get("cnt").and_then(|v| v.as_u64())) {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn query_logs(&self, query: LogQuery) -> Result<LogQueryResult> {
        let start = std::time::Instant::now();

        let partitions = self.get_partitions_for_query(&query)?;
        let mut all_results = Vec::new();
        let mut partitions_queried = Vec::new();

        let mut remaining = query.limit;
        let mut current_offset = query.offset;

        for partition_name in partitions {
            if remaining == 0 {
                break;
            }

            partitions_queried.push(partition_name.clone());

            let where_clauses = self.build_where_clause(&query, &partition_name)?;
            let sql = format!(
                "SELECT * FROM {} WHERE {} ORDER BY timestamp {} LIMIT {} OFFSET {}",
                partition_name,
                where_clauses,
                if query.order_desc { "DESC" } else { "ASC" },
                remaining,
                current_offset
            );

            match self.connection.query(&sql).await {
                Ok(rows) => {
                    let row_count = rows.len() as u64;

                    for row in rows {
                        if let Some(obj) = row.as_object() {
                            all_results.push(serde_json::Value::Object(obj.clone()));
                        }
                    }

                    if current_offset >= row_count {
                        current_offset -= row_count;
                    } else {
                        current_offset = 0;
                    }

                    remaining = remaining.saturating_sub(row_count);
                }
                Err(_) => continue,
            }
        }

        let total_matched = self.count_matching_logs(&query).await?;

        Ok(LogQueryResult {
            total_matched,
            results: all_results,
            partitions_queried,
            query_time_ms: start.elapsed().as_millis() as u64,
            has_more: total_matched > query.offset + query.limit,
        })
    }

    pub async fn query_logs_with_aggregation(&self, query: LogQuery) -> Result<LogAggregationResult> {
        let log_result = self.query_logs(query).await?;
        let aggregator = LogAggregator::new(3600, 300);

        let level_distribution = aggregator.aggregate_by_level(&log_result.results);
        let time_distribution = aggregator.aggregate_by_time_window(&log_result.results);

        Ok(LogAggregationResult {
            total_matched: log_result.total_matched,
            results: log_result.results,
            level_distribution,
            time_distribution,
            query_time_ms: log_result.query_time_ms,
        })
    }

    fn get_partitions_for_query(&self, query: &LogQuery) -> Result<Vec<String>> {
        let partitions = self.partitions.get(&query.table_name)
            .ok_or_else(|| anyhow!("No partitions found for table {}", query.table_name))?;

        let mut relevant_partitions: Vec<&LogPartition> = partitions.iter()
            .filter(|p| {
                if let (Some(start), Some(end)) = (query.start_time, query.end_time) {
                    let part_start = chrono::DateTime::parse_from_rfc3339(&p.start_date)
                        .map(|dt| dt.timestamp())
                        .unwrap_or(0);
                    part_start >= start && part_start <= end
                } else {
                    p.is_active
                }
            })
            .collect();

        relevant_partitions.sort_by(|a, b| a.start_date.cmp(&b.start_date));

        Ok(relevant_partitions.into_iter().map(|p| p.partition_name.clone()).collect())
    }

    fn build_where_clause(&self, query: &LogQuery, _partition: &str) -> Result<String> {
        let mut clauses = Vec::new();

        if !query.level_filter.is_empty() {
            let levels = query.level_filter.iter()
                .map(|l| format!("'{}'", l))
                .collect::<Vec<_>>()
                .join(", ");
            clauses.push(format!("level IN ({})", levels));
        }

        if let Some(ref keyword) = query.keyword_filter {
            clauses.push(format!("message LIKE '%{}%'", keyword.replace('\'', "''")));
        }

        if let (Some(start), Some(end)) = (query.start_time, query.end_time) {
            clauses.push(format!(
                "timestamp BETWEEN FROM_UNIXTIME({}) AND FROM_UNIXTIME({})",
                start, end
            ));
        }

        if clauses.is_empty() {
            Ok("1=1".to_string())
        } else {
            Ok(clauses.join(" AND "))
        }
    }

    async fn count_matching_logs(&self, query: &LogQuery) -> Result<u64> {
        let partitions = self.get_partitions_for_query(query)?;
        let mut total = 0u64;

        for partition_name in partitions {
            let where_clauses = self.build_where_clause(query, &partition_name)?;
            let sql = format!(
                "SELECT COUNT(*) as cnt FROM {} WHERE {}",
                partition_name, where_clauses
            );

            if let Ok(rows) = self.connection.query(&sql).await {
                total += rows.first()
                    .and_then(|r| r.get("cnt").and_then(|v| v.as_u64()))
                    .unwrap_or(0);
            }
        }

        Ok(total)
    }

    pub async fn cleanup_old_partitions(&mut self, table_name: &str) -> Result<u32> {
        let config = self.configs.get(table_name)
            .ok_or_else(|| anyhow!("Log table {} not registered", table_name))?;

        let cutoff_date = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(config.retention_days as i64))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();

        let mut deleted_count = 0u32;

        if let Some(partitions) = self.partitions.get_mut(table_name) {
            let to_delete: Vec<String> = partitions.iter()
                .filter(|p| !p.is_active && p.start_date < cutoff_date)
                .map(|p| p.partition_name.clone())
                .collect();

            for partition_name in to_delete {
                let drop_sql = format!("DROP TABLE IF EXISTS {}", partition_name);
                if self.connection.execute(&drop_sql).await.is_ok() {
                    partitions.retain(|p| p.partition_name != partition_name);
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }

    pub async fn archive_old_partitions(&mut self, table_name: &str, archive_table: &str) -> Result<u64> {
        let config = self.configs.get(table_name)
            .ok_or_else(|| anyhow!("Log table {} not registered", table_name))?;

        let cutoff_date = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(config.retention_days as i64))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();

        let mut archived_count = 0u64;

        if let Some(partitions) = self.partitions.get(table_name) {
            let to_archive: Vec<String> = partitions.iter()
                .filter(|p| !p.is_active && p.start_date < cutoff_date)
                .map(|p| p.partition_name.clone())
                .collect();

            for partition_name in to_archive {
                let archive_sql = format!(
                    "INSERT INTO {} SELECT * FROM {}",
                    archive_table, partition_name
                );
                if self.connection.execute(&archive_sql).await.is_ok() {
                    archived_count += 1;
                }
            }
        }

        Ok(archived_count)
    }

    pub async fn get_log_stats(&self, table_name: &str) -> Result<LogStats> {
        let partitions = self.partitions.get(table_name)
            .ok_or_else(|| anyhow!("No partitions found for table {}", table_name))?;

        let total_partitions = partitions.len();
        let active_partitions = partitions.iter().filter(|p| p.is_active).count();

        let total_rows: u64 = partitions.iter().map(|p| p.row_count).sum();
        let total_size: f64 = partitions.iter().map(|p| p.size_mb).sum();

        let oldest = partitions.iter().min_by(|a, b| a.start_date.cmp(&b.start_date));
        let newest = partitions.iter().max_by(|a, b| a.start_date.cmp(&b.start_date));

        let compressed_count = partitions.iter().filter(|p| p.compressed).count();
        let compression_ratio = if total_partitions > 0 {
            compressed_count as f64 / total_partitions as f64
        } else {
            0.0
        };

        Ok(LogStats {
            table_name: table_name.to_string(),
            total_partitions,
            active_partitions,
            total_size_gb: total_size / 1024.0,
            total_rows,
            compression_ratio,
            oldest_partition: oldest.map(|p| p.start_date.clone()),
            newest_partition: newest.map(|p| p.start_date.clone()),
        })
    }

    pub async fn compress_partition(&mut self, table_name: &str, partition_name: &str) -> Result<()> {
        if let Some(partitions) = self.partitions.get_mut(table_name) {
            if let Some(partition) = partitions.iter_mut().find(|p| &p.partition_name == partition_name) {
                let optimize_sql = format!("OPTIMIZE TABLE {}", partition_name);
                self.connection.execute(&optimize_sql).await?;
                partition.compressed = true;
            }
        }
        Ok(())
    }

    pub fn get_all_partitions(&self, table_name: &str) -> Option<Vec<LogPartition>> {
        self.partitions.get(table_name).cloned()
    }

    pub fn get_partition_count(&self, table_name: &str) -> usize {
        self.partitions.get(table_name).map(|p| p.len()).unwrap_or(0)
    }

    pub async fn truncate_old_logs(&mut self, table_name: &str, days_to_keep: u32) -> Result<u64> {
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(days_to_keep as i64))
            .unwrap()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let sql = format!(
            "DELETE FROM {} WHERE timestamp < '{}'",
            table_name, cutoff
        );

        self.connection.execute(&sql).await?;
        Ok(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub context: Option<serde_json::Value>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAggregationResult {
    pub total_matched: u64,
    pub results: Vec<serde_json::Value>,
    pub level_distribution: HashMap<String, u64>,
    pub time_distribution: Vec<TimeWindowAggregation>,
    pub query_time_ms: u64,
}

pub struct LogAggregator {
    time_window_secs: u64,
    aggregation_interval_secs: u64,
}

impl LogAggregator {
    pub fn new(time_window_secs: u64, aggregation_interval_secs: u64) -> Self {
        Self {
            time_window_secs,
            aggregation_interval_secs,
        }
    }

    pub fn aggregate_by_level(&self, logs: &[serde_json::Value]) -> HashMap<String, u64> {
        let mut counts = HashMap::new();

        for log in logs {
            if let Some(level) = log.get("level").and_then(|v| v.as_str()) {
                *counts.entry(level.to_string()).or_insert(0) += 1;
            }
        }

        counts
    }

    pub fn aggregate_by_time_window(&self, logs: &[serde_json::Value]) -> Vec<TimeWindowAggregation> {
        let mut windows: HashMap<u64, TimeWindowAggregation> = HashMap::new();

        for log in logs {
            if let Some(timestamp) = log.get("timestamp").and_then(|v| v.as_str()) {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp) {
                    let window_key = dt.timestamp() / self.aggregation_interval_secs as i64;

                    let entry = windows.entry(window_key as u64).or_insert_with(|| TimeWindowAggregation {
                        window_start: window_key as i64 * self.aggregation_interval_secs as i64,
                        window_end: (window_key + 1) as i64 * self.aggregation_interval_secs as i64 - 1,
                        count: 0,
                        levels: HashMap::new(),
                        error_count: 0,
                        warning_count: 0,
                    });

                    entry.count += 1;

                    if let Some(level) = log.get("level").and_then(|v| v.as_str()) {
                        *entry.levels.entry(level.to_string()).or_insert(0) += 1;

                        match level {
                            "ERROR" | "FATAL" | "CRITICAL" => entry.error_count += 1,
                            "WARN" | "WARNING" => entry.warning_count += 1,
                            _ => {}
                        }
                    }
                }
            }
        }

        let mut result: Vec<_> = windows.into_values().collect();
        result.sort_by_key(|w| w.window_start);
        result
    }

    pub fn find_error_spikes(&self, logs: &[serde_json::Value], threshold: u64) -> Vec<ErrorSpike> {
        let windows = self.aggregate_by_time_window(logs);
        let mut spikes = Vec::new();

        for window in windows {
            if window.error_count >= threshold {
                spikes.push(ErrorSpike {
                    window_start: window.window_start,
                    window_end: window.window_end,
                    error_count: window.error_count,
                    total_logs: window.count,
                });
            }
        }

        spikes
    }

    pub fn calculate_error_rate(&self, logs: &[serde_json::Value]) -> f64 {
        if logs.is_empty() {
            return 0.0;
        }

        let error_count = logs.iter()
            .filter(|log| {
                log.get("level")
                    .and_then(|v| v.as_str())
                    .map(|l| l == "ERROR" || l == "FATAL" || l == "CRITICAL")
                    .unwrap_or(false)
            })
            .count() as f64;

        error_count / logs.len() as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindowAggregation {
    pub window_start: i64,
    pub window_end: i64,
    pub count: u64,
    pub levels: HashMap<String, u64>,
    pub error_count: u64,
    pub warning_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSpike {
    pub window_start: i64,
    pub window_end: i64,
    pub error_count: u64,
    pub total_logs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_type_serialization() {
        let pt = PartitionType::Daily;
        let json = serde_json::to_string(&pt).unwrap();
        assert!(json.contains("Daily"));
    }

    #[test]
    fn test_log_aggregator() {
        let aggregator = LogAggregator::new(3600, 60);

        let logs = vec![
            serde_json::json!({"level": "INFO", "timestamp": "2024-01-01T00:00:00Z"}),
            serde_json::json!({"level": "ERROR", "timestamp": "2024-01-01T00:00:00Z"}),
            serde_json::json!({"level": "INFO", "timestamp": "2024-01-01T00:00:00Z"}),
        ];

        let counts = aggregator.aggregate_by_level(&logs);
        assert_eq!(counts.get("INFO"), Some(&2));
        assert_eq!(counts.get("ERROR"), Some(&1));
    }

    #[test]
    fn test_log_stats_calculation() {
        let partitions = vec![
            LogPartition {
                partition_name: "logs_p0".to_string(),
                table_name: "logs".to_string(),
                start_date: "2024-01-01".to_string(),
                end_date: Some("2024-01-02".to_string()),
                row_count: 1000,
                size_mb: 100.0,
                is_active: false,
                compressed: false,
            },
            LogPartition {
                partition_name: "logs_p1".to_string(),
                table_name: "logs".to_string(),
                start_date: "2024-01-02".to_string(),
                end_date: None,
                row_count: 500,
                size_mb: 50.0,
                is_active: true,
                compressed: true,
            },
        ];

        let total_rows: u64 = partitions.iter().map(|p| p.row_count).sum();
        assert_eq!(total_rows, 1500);

        let compressed_ratio = partitions.iter().filter(|p| p.compressed).count() as f64 / partitions.len() as f64;
        assert_eq!(compressed_ratio, 0.5);
    }

    #[test]
    fn test_error_spike_detection() {
        let aggregator = LogAggregator::new(3600, 60);

        let logs = vec![
            serde_json::json!({"level": "ERROR", "timestamp": "2024-01-01T00:00:00Z"}),
            serde_json::json!({"level": "ERROR", "timestamp": "2024-01-01T00:00:00Z"}),
            serde_json::json!({"level": "ERROR", "timestamp": "2024-01-01T00:00:00Z"}),
            serde_json::json!({"level": "INFO", "timestamp": "2024-01-01T00:00:00Z"}),
        ];

        let spikes = aggregator.find_error_spikes(&logs, 2);
        assert!(!spikes.is_empty());
        assert_eq!(spikes[0].error_count, 3);
    }

    #[test]
    fn test_error_rate_calculation() {
        let aggregator = LogAggregator::new(3600, 60);

        let logs = vec![
            serde_json::json!({"level": "ERROR", "timestamp": "2024-01-01T00:00:00Z"}),
            serde_json::json!({"level": "INFO", "timestamp": "2024-01-01T00:00:00Z"}),
            serde_json::json!({"level": "INFO", "timestamp": "2024-01-01T00:00:00Z"}),
            serde_json::json!({"level": "INFO", "timestamp": "2024-01-01T00:00:00Z"}),
        ];

        let rate = aggregator.calculate_error_rate(&logs);
        assert_eq!(rate, 0.25);
    }

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry {
            level: "ERROR".to_string(),
            message: "Test error".to_string(),
            context: Some(serde_json::json!({"user_id": 123})),
            source: Some("test_module".to_string()),
        };

        assert_eq!(entry.level, "ERROR");
        assert_eq!(entry.message, "Test error");
    }
}
