use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableConfig {
    pub table_name: String,
    pub max_rows: u64,
    pub shard_type: ShardStrategy,
    pub shard_prefix: String,
    pub compression_enabled: bool,
    pub auto_optimize: bool,
    pub optimize_threshold_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShardStrategy {
    RowCount { max_rows: u64 },
    DateSuffix { format: String },
    SizeBased { max_size_mb: u64 },
    TimeInterval { interval_hours: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    pub shard_name: String,
    pub shard_type: String,
    pub row_count: u64,
    pub size_mb: f64,
    pub created_at: i64,
    pub is_active: bool,
    pub data_range_start: Option<String>,
    pub data_range_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanningQueryResult {
    pub total_count: u64,
    pub results: Vec<serde_json::Value>,
    pub shard_results: HashMap<String, Vec<serde_json::Value>>,
    pub has_more: bool,
    pub next_shard: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub fields_to_compress: Vec<String>,
    pub compress_threshold_bytes: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fields_to_compress: vec![],
            compress_threshold_bytes: 1024,
        }
    }
}

pub struct AutoShardingManager {
    table_configs: HashMap<String, TableConfig>,
    shard_registry: HashMap<String, Vec<ShardInfo>>,
}

impl AutoShardingManager {
    pub fn new() -> Self {
        Self {
            table_configs: HashMap::new(),
            shard_registry: HashMap::new(),
        }
    }

    pub fn register_table(&mut self, config: TableConfig) {
        self.table_configs.insert(config.table_name.clone(), config);
    }

    pub fn get_shards(&self, table_name: &str) -> Vec<&ShardInfo> {
        self.shard_registry
            .get(table_name)
            .map(|shards| shards.iter().filter(|s| s.is_active).collect())
            .unwrap_or_default()
    }

    pub fn get_all_shards(&self, table_name: &str) -> Vec<&ShardInfo> {
        self.shard_registry
            .get(table_name)
            .map(|shards| shards.iter().collect())
            .unwrap_or_default()
    }

    pub fn create_new_shard(&mut self, table_name: &str) -> Result<Option<String>> {
        let config = self.table_configs.get(table_name)
            .ok_or_else(|| anyhow!("Table {} not registered", table_name))?;

        let shards = self.shard_registry.entry(table_name.to_string()).or_insert_with(Vec::new);
        let shard_number = shards.len() + 1;
        let shard_name = format!("{}_{}", config.shard_prefix, shard_number);

        let now = chrono::Utc::now();
        let shard_info = ShardInfo {
            shard_name: shard_name.clone(),
            shard_type: format!("{:?}", config.shard_type),
            row_count: 0,
            size_mb: 0.0,
            created_at: now.timestamp(),
            is_active: true,
            data_range_start: Some(now.format("%Y-%m-%d %H:%M:%S").to_string()),
            data_range_end: None,
        };

        if let Some(last) = shards.iter_mut().find(|s| s.is_active) {
            last.is_active = false;
            last.data_range_end = Some(now.format("%Y-%m-%d %H:%M:%S").to_string());
        }

        shards.push(shard_info);

        Ok(Some(shard_name))
    }

    pub async fn execute_spanning_insert(
        &mut self,
        table_name: &str,
        _data: &serde_json::Value,
    ) -> Result<()> {
        let active_shard_name = {
            let shards = self.shard_registry.get(table_name).ok_or_else(|| anyhow!("No shards found"))?;
            let active_shard = shards.iter().find(|s| s.is_active).ok_or_else(|| anyhow!("No active shard"))?;
            active_shard.shard_name.clone()
        };

        if let Some(shards) = self.shard_registry.get_mut(table_name) {
            if let Some(shard) = shards.iter_mut().find(|s| s.shard_name == active_shard_name) {
                shard.row_count += 1;
            }
        }

        self.check_and_shard(table_name).await?;

        Ok(())
    }

    pub async fn check_and_shard(&mut self, table_name: &str) -> Result<Option<String>> {
        let config = self.table_configs.get(table_name)
            .ok_or_else(|| anyhow!("Table {} not registered", table_name))?;

        let row_count = {
            let shards = self.shard_registry.get(table_name);
            shards.map(|s| s.iter().map(|sh| sh.row_count).sum::<u64>()).unwrap_or(0)
        };

        match &config.shard_type {
            ShardStrategy::RowCount { max_rows } => {
                if row_count >= *max_rows {
                    return self.create_new_shard(table_name);
                }
            }
            ShardStrategy::DateSuffix { format: _ } => {
                if row_count >= config.max_rows {
                    return self.create_new_shard(table_name);
                }
            }
            ShardStrategy::SizeBased { max_size_mb: _ } => {
                let size_mb: f64 = self.shard_registry.get(table_name)
                    .map(|s| s.iter().map(|sh| sh.size_mb).sum())
                    .unwrap_or(0.0);
                if size_mb >= config.max_rows as f64 {
                    return self.create_new_shard(table_name);
                }
            }
            ShardStrategy::TimeInterval { interval_hours: _ } => {
                if row_count >= config.max_rows {
                    return self.create_new_shard(table_name);
                }
            }
        }

        Ok(None)
    }

    pub fn spanning_query(
        &self,
        table_name: &str,
        condition: &str,
        order_by: &str,
        limit: u64,
        offset: u64,
    ) -> Result<SpanningQueryResult> {
        let shards = self.get_all_shards(table_name);
        let mut all_results = Vec::new();
        let mut shard_results = HashMap::new();
        let mut remaining = limit;
        let mut current_offset = offset;
        let mut total_count = 0u64;

        for shard in shards {
            if remaining == 0 {
                break;
            }

            let _sql = format!(
                "SELECT * FROM {} WHERE {} ORDER BY {} LIMIT {} OFFSET {}",
                shard.shard_name, condition, order_by, remaining, current_offset
            );

            let mock_rows: Vec<serde_json::Value> = vec![];
            total_count += mock_rows.len() as u64;

            for row in mock_rows {
                if let serde_json::Value::Object(obj) = row {
                    all_results.push(serde_json::Value::Object(obj));
                }
            }

            shard_results.insert(shard.shard_name.clone(), vec![]);

            if current_offset >= shard.row_count {
                current_offset = current_offset.saturating_sub(shard.row_count);
            } else {
                current_offset = 0;
            }

            remaining = remaining.saturating_sub(shard.row_count.min(remaining));
        }

        Ok(SpanningQueryResult {
            total_count,
            results: all_results,
            shard_results,
            has_more: remaining > 0,
            next_shard: None,
        })
    }

    pub fn spanning_query_sorted(
        &self,
        table_name: &str,
        condition: &str,
        sort_field: &str,
        ascending: bool,
        limit: u64,
        offset: u64,
    ) -> Result<SpanningQueryResult> {
        let mut result = self.spanning_query(table_name, condition, sort_field, limit * 10, 0)?;

        result.results.sort_by(|a, b| {
            let a_val = a.get(sort_field).map(|v| v.to_string()).unwrap_or_default();
            let b_val = b.get(sort_field).map(|v| v.to_string()).unwrap_or_default();
            if ascending {
                a_val.cmp(&b_val)
            } else {
                b_val.cmp(&a_val)
            }
        });

        let start = offset as usize;
        let end = (offset + limit) as usize;
        result.results = result.results.into_iter().skip(start).take(end - start).collect();
        result.total_count = result.results.len() as u64;

        Ok(result)
    }

    pub fn get_shard_summary(&self, table_name: &str) -> Option<ShardSummary> {
        let shards = self.shard_registry.get(table_name)?;

        let total_rows: u64 = shards.iter().map(|s| s.row_count).sum();
        let total_size: f64 = shards.iter().map(|s| s.size_mb).sum();

        Some(ShardSummary {
            table_name: table_name.to_string(),
            shard_count: shards.len(),
            total_rows,
            total_size_mb: total_size,
            active_shards: shards.iter().filter(|s| s.is_active).count(),
        })
    }

    pub fn list_all_shards(&self) -> HashMap<String, Vec<ShardInfo>> {
        self.shard_registry.clone()
    }

    pub fn get_table_config(&self, table_name: &str) -> Option<&TableConfig> {
        self.table_configs.get(table_name)
    }

    pub fn update_shard_stats(&mut self, table_name: &str, shard_name: &str, row_count: u64, size_mb: f64) -> Result<()> {
        if let Some(shards) = self.shard_registry.get_mut(table_name) {
            if let Some(shard) = shards.iter_mut().find(|s| s.shard_name == shard_name) {
                shard.row_count = row_count;
                shard.size_mb = size_mb;
                return Ok(());
            }
        }
        Err(anyhow!("Shard {} not found for table {}", shard_name, table_name))
    }

    pub fn get_active_shard(&self, table_name: &str) -> Option<&ShardInfo> {
        self.shard_registry.get(table_name)?
            .iter()
            .find(|s| s.is_active)
    }

    pub fn should_shard(&self, table_name: &str) -> bool {
        let config = match self.table_configs.get(table_name) {
            Some(c) => c,
            None => return false,
        };

        let shards = match self.shard_registry.get(table_name) {
            Some(s) => s,
            None => return false,
        };

        let total_rows: u64 = shards.iter().map(|s| s.row_count).sum();
        let total_size: f64 = shards.iter().map(|s| s.size_mb).sum();

        match &config.shard_type {
            ShardStrategy::RowCount { max_rows } => total_rows >= *max_rows,
            ShardStrategy::SizeBased { max_size_mb } => total_size >= *max_size_mb as f64,
            _ => total_rows >= config.max_rows,
        }
    }
}

impl Default for AutoShardingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardSummary {
    pub table_name: String,
    pub shard_count: usize,
    pub total_rows: u64,
    pub total_size_mb: f64,
    pub active_shards: usize,
}

pub trait StorageBackend: Send + Sync {
    fn store(&self, key: &str, value: &[u8]) -> Result<()>;
    fn retrieve(&self, key: &str) -> Result<Vec<u8>>;
    fn delete(&self, key: &str) -> Result<()>;
}

pub struct CompressionHandler {
    config: CompressionConfig,
}

impl CompressionHandler {
    pub fn new() -> Self {
        Self {
            config: CompressionConfig::default(),
        }
    }

    pub fn with_config(config: CompressionConfig) -> Self {
        Self { config }
    }

    pub fn compress_data(&self, data: &serde_json::Value) -> Result<serde_json::Value> {
        if !self.config.enabled {
            return Ok(data.clone());
        }

        let mut result = data.clone();

        if let serde_json::Value::Object(ref mut obj) = result {
            for field in &self.config.fields_to_compress {
                if let Some(value) = obj.get(field) {
                    let json_str = serde_json::to_string(value)?;
                    if json_str.len() > self.config.compress_threshold_bytes {
                        let compressed = Self::simple_compress(json_str.as_bytes());
                        let encoded = base64_encode(&compressed);
                        obj.insert(format!("_c_{}", field), serde_json::Value::String(encoded));
                        obj.remove(field);
                    }
                }
            }
        }

        Ok(result)
    }

    pub fn decompress_data(&self, data: &serde_json::Value) -> serde_json::Value {
        if !self.config.enabled {
            return data.clone();
        }

        let mut result = data.clone();

        if let serde_json::Value::Object(ref mut obj) = result {
            let fields_to_decompress: Vec<String> = obj.keys()
                .filter(|k| k.starts_with("_c_"))
                .cloned()
                .collect();

            for compressed_field in fields_to_decompress {
                let original_field = compressed_field.strip_prefix("_c_").unwrap().to_string();
                if let Some(serde_json::Value::String(encoded)) = obj.get(&compressed_field) {
                    if let Ok(decoded) = base64_decode(encoded) {
                        if let Ok(decompressed) = Self::simple_decompress(&decoded) {
                            if let Ok(value) = serde_json::from_str(&String::from_utf8_lossy(&decompressed)) {
                                obj.insert(original_field, value);
                            }
                        }
                    }
                }
                obj.remove(&compressed_field);
            }
        }

        result
    }

    pub fn compress_text_field(&self, field_name: &str, text: &str) -> Result<String> {
        if !self.config.enabled || !self.config.fields_to_compress.contains(&field_name.to_string()) {
            return Ok(text.to_string());
        }
        if text.len() < self.config.compress_threshold_bytes {
            return Ok(text.to_string());
        }
        let compressed = Self::simple_compress(text.as_bytes());
        Ok(base64_encode(&compressed))
    }

    pub fn decompress_text_field(&self, compressed_text: &str) -> Result<String> {
        let decoded = base64_decode(compressed_text)?;
        let decompressed = Self::simple_decompress(&decoded)?;
        Ok(String::from_utf8_lossy(&decompressed).to_string())
    }

    fn simple_compress(input: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < input.len() {
            let mut count = 1;
            while i + count < input.len() && input[i + count] == input[i] && count < 255 {
                count += 1;
            }
            if count > 3 {
                result.push(0xFF);
                result.push(input[i]);
                result.push(count as u8);
                i += count;
            } else {
                result.push(input[i]);
                i += 1;
            }
        }
        result
    }

    fn simple_decompress(input: &[u8]) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < input.len() {
            if input[i] == 0xFF && i + 2 < input.len() {
                let byte = input[i + 1];
                let count = input[i + 2] as usize;
                for _ in 0..count {
                    result.push(byte);
                }
                i += 3;
            } else {
                result.push(input[i]);
                i += 1;
            }
        }
        Ok(result)
    }
}

impl Default for CompressionHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in input.chunks(3) {
        let b = match chunk.len() {
            1 => [chunk[0], 0, 0],
            2 => [chunk[0], chunk[1], 0],
            _ => [chunk[0], chunk[1], chunk[2]],
        };
        result.push(CHARS[(b[0] >> 2) as usize] as char);
        result.push(CHARS[((b[0] & 0x03) << 4 | b[1] >> 4) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((b[1] & 0x0F) << 2 | b[2] >> 6) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(b[2] & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    const DECODE: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62, -1, -1, -1, 63,
        52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1,
        -1,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14,
        15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1,
        -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
        41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
    ];
    let input = input.trim_end_matches('=');
    let mut result = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;

    for c in input.chars() {
        if c as usize >= 128 {
            return Err(anyhow!("Invalid base64 character"));
        }
        let value = DECODE[c as usize];
        if value < 0 {
            return Err(anyhow!("Invalid base64 character"));
        }
        buffer = (buffer << 6) | (value as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }

    Ok(result)
}

pub struct SmartAutoOptimizer;

impl SmartAutoOptimizer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze_and_optimize(&self, table_name: &str) -> OptimizationReport {
        OptimizationReport {
            table_name: table_name.to_string(),
            issues: Vec::new(),
            suggestions: vec![
                "Consider adding indexes on frequently queried columns".to_string(),
                "Monitor table size and consider partitioning if large".to_string(),
            ],
            actions_taken: Vec::new(),
        }
    }

    pub fn analyze_table_health(&self, table_name: &str, row_count: u64, size_mb: f64) -> TableHealthReport {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();
        let mut severity = HealthSeverity::Healthy;

        if row_count > 10_000_000 {
            issues.push("Table has over 10 million rows".to_string());
            suggestions.push("Consider partitioning or archiving old data".to_string());
            severity = HealthSeverity::Warning;
        }

        if size_mb > 1024.0 {
            issues.push(format!("Table size exceeds 1GB ({} MB)", size_mb));
            suggestions.push("Consider compression or splitting large tables".to_string());
            severity = HealthSeverity::Critical;
        }

        if row_count > 0 && size_mb / row_count as f64 > 10.0 {
            issues.push("Average row size is unusually large".to_string());
            suggestions.push("Check for large TEXT/BLOB columns and consider compression".to_string());
            if severity == HealthSeverity::Healthy {
                severity = HealthSeverity::Warning;
            }
        }

        TableHealthReport {
            table_name: table_name.to_string(),
            row_count,
            size_mb,
            severity,
            issues,
            suggestions,
        }
    }

    pub fn generate_optimization_sql(&self, table_name: &str, db_type: &str) -> Vec<String> {
        let mut sqls = Vec::new();

        match db_type {
            "mysql" | "mariadb" => {
                sqls.push(format!("ANALYZE TABLE {};", table_name));
                sqls.push(format!("OPTIMIZE TABLE {};", table_name));
            }
            "postgresql" | "postgres" => {
                sqls.push(format!("VACUUM ANALYZE {};", table_name));
                sqls.push(format!("REINDEX TABLE {};", table_name));
            }
            "sqlite" => {
                sqls.push(format!("ANALYZE {};", table_name));
            }
            _ => {}
        }

        sqls
    }
}

impl Default for SmartAutoOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationReport {
    pub table_name: String,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
    pub actions_taken: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableHealthReport {
    pub table_name: String,
    pub row_count: u64,
    pub size_mb: f64,
    pub severity: HealthSeverity,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthSeverity {
    Healthy,
    Warning,
    Critical,
}

impl std::fmt::Display for HealthSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthSeverity::Healthy => write!(f, "Healthy"),
            HealthSeverity::Warning => write!(f, "Warning"),
            HealthSeverity::Critical => write!(f, "Critical"),
        }
    }
}

pub struct OptimizationRule {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedQuery {
    pub table_name: String,
    pub operation: CrudOperation,
    pub conditions: HashMap<String, serde_json::Value>,
    pub data: Option<serde_json::Value>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub order_by: Option<String>,
    pub ascending: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrudOperation {
    Create,
    Read,
    Update,
    Delete,
    Count,
}

impl std::fmt::Display for CrudOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrudOperation::Create => write!(f, "CREATE"),
            CrudOperation::Read => write!(f, "READ"),
            CrudOperation::Update => write!(f, "UPDATE"),
            CrudOperation::Delete => write!(f, "DELETE"),
            CrudOperation::Count => write!(f, "COUNT"),
        }
    }
}

pub struct UnifiedCrudExecutor;

impl UnifiedCrudExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn build_sql(&self, query: &UnifiedQuery) -> Result<String> {
        match query.operation {
            CrudOperation::Read => self.build_select(query),
            CrudOperation::Create => self.build_insert(query),
            CrudOperation::Update => self.build_update(query),
            CrudOperation::Delete => self.build_delete(query),
            CrudOperation::Count => self.build_count(query),
        }
    }

    fn build_select(&self, query: &UnifiedQuery) -> Result<String> {
        let mut sql = format!("SELECT * FROM {}", query.table_name);

        let where_clause = self.build_where(&query.conditions);
        if !where_clause.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }

        if let Some(ref order) = query.order_by {
            let direction = if query.ascending.unwrap_or(true) { "ASC" } else { "DESC" };
            sql.push_str(&format!(" ORDER BY {} {}", order, direction));
        }

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        Ok(sql)
    }

    fn build_insert(&self, query: &UnifiedQuery) -> Result<String> {
        let data = query.data.as_ref().ok_or_else(|| anyhow!("Data required for INSERT"))?;

        if let serde_json::Value::Object(obj) = data {
            let fields: Vec<String> = obj.keys().cloned().collect();
            let values: Vec<String> = obj.values().map(|v| self.value_to_sql(v)).collect();

            Ok(format!(
                "INSERT INTO {} ({}) VALUES ({})",
                query.table_name,
                fields.join(", "),
                values.join(", ")
            ))
        } else {
            Err(anyhow!("Data must be an object"))
        }
    }

    fn build_update(&self, query: &UnifiedQuery) -> Result<String> {
        let data = query.data.as_ref().ok_or_else(|| anyhow!("Data required for UPDATE"))?;

        if let serde_json::Value::Object(obj) = data {
            let sets: Vec<String> = obj.iter()
                .map(|(k, v)| format!("{} = {}", k, self.value_to_sql(v)))
                .collect();

            let mut sql = format!("UPDATE {} SET {}", query.table_name, sets.join(", "));

            let where_clause = self.build_where(&query.conditions);
            if !where_clause.is_empty() {
                sql.push_str(&format!(" WHERE {}", where_clause));
            }

            Ok(sql)
        } else {
            Err(anyhow!("Data must be an object"))
        }
    }

    fn build_delete(&self, query: &UnifiedQuery) -> Result<String> {
        let mut sql = format!("DELETE FROM {}", query.table_name);

        let where_clause = self.build_where(&query.conditions);
        if !where_clause.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }

        Ok(sql)
    }

    fn build_count(&self, query: &UnifiedQuery) -> Result<String> {
        let mut sql = format!("SELECT COUNT(*) as cnt FROM {}", query.table_name);

        let where_clause = self.build_where(&query.conditions);
        if !where_clause.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }

        Ok(sql)
    }

    fn build_where(&self, conditions: &HashMap<String, serde_json::Value>) -> String {
        if conditions.is_empty() {
            return String::new();
        }

        let clauses: Vec<String> = conditions.iter()
            .map(|(k, v)| format!("{} = {}", k, self.value_to_sql(v)))
            .collect();

        clauses.join(" AND ")
    }

    fn value_to_sql(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "NULL".to_string(),
            serde_json::Value::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            serde_json::Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| self.value_to_sql(v)).collect();
                format!("({})", items.join(", "))
            }
            serde_json::Value::Object(_) => "'{}'".to_string(),
        }
    }
}

impl Default for UnifiedCrudExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardMigrationPlanner {
    pub source_shards: Vec<String>,
    pub target_shards: Vec<String>,
    pub migration_steps: Vec<MigrationStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStep {
    pub step_number: usize,
    pub source_shard: String,
    pub target_shard: String,
    pub estimated_rows: u64,
    pub sql: String,
}

pub struct ShardRebalancer;

impl ShardRebalancer {
    pub fn new() -> Self {
        Self
    }

    pub fn plan_migration(&self, shards: &[ShardInfo], target_count: usize) -> ShardMigrationPlanner {
        let mut steps = Vec::new();
        let source_shards: Vec<String> = shards.iter().map(|s| s.shard_name.clone()).collect();
        let target_shards: Vec<String> = (1..=target_count)
            .map(|i| format!("shard_{}", i))
            .collect();

        let total_rows: u64 = shards.iter().map(|s| s.row_count).sum();
        let _target_rows_per_shard = total_rows / target_count as u64;

        for (i, shard) in shards.iter().enumerate() {
            if i < target_count {
                continue;
            }
            let target = &target_shards[i % target_count];
            steps.push(MigrationStep {
                step_number: steps.len() + 1,
                source_shard: shard.shard_name.clone(),
                target_shard: target.clone(),
                estimated_rows: shard.row_count,
                sql: format!(
                    "INSERT INTO {} SELECT * FROM {} WHERE shard_condition",
                    target, shard.shard_name
                ),
            });
        }

        ShardMigrationPlanner {
            source_shards,
            target_shards,
            migration_steps: steps,
        }
    }
}

impl Default for ShardRebalancer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_strategy_serialization() {
        let strategy = ShardStrategy::RowCount { max_rows: 2000000 };
        let json = serde_json::to_string(&strategy).unwrap();
        assert!(json.contains("2000000"));
    }

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert!(config.enabled);
    }

    #[test]
    fn test_base64_encode_decode() {
        let original = b"Hello, World!";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_compression_handler() {
        let handler = CompressionHandler::new();
        let data = serde_json::json!({
            "id": 1,
            "name": "test",
            "description": "a".repeat(2000)
        });

        let compressed = handler.compress_data(&data).unwrap();
        let decompressed = handler.decompress_data(&compressed);
        assert!(decompressed.get("description").is_some());
    }

    #[test]
    fn test_auto_sharding_manager() {
        let mut manager = AutoShardingManager::new();

        manager.register_table(TableConfig {
            table_name: "logs".to_string(),
            max_rows: 100,
            shard_type: ShardStrategy::RowCount { max_rows: 100 },
            shard_prefix: "logs_p".to_string(),
            compression_enabled: true,
            auto_optimize: true,
            optimize_threshold_ms: 1000,
        });

        manager.create_new_shard("logs").unwrap();
        manager.create_new_shard("logs").unwrap();

        let summary = manager.get_shard_summary("logs").unwrap();
        assert_eq!(summary.shard_count, 2);
    }

    #[test]
    fn test_unified_crud_select() {
        let executor = UnifiedCrudExecutor::new();
        let mut conditions = HashMap::new();
        conditions.insert("id".to_string(), serde_json::json!(1));

        let query = UnifiedQuery {
            table_name: "users".to_string(),
            operation: CrudOperation::Read,
            conditions,
            data: None,
            limit: Some(10),
            offset: Some(0),
            order_by: Some("name".to_string()),
            ascending: Some(true),
        };

        let sql = executor.build_sql(&query).unwrap();
        assert!(sql.contains("SELECT * FROM users"));
        assert!(sql.contains("WHERE id = 1"));
        assert!(sql.contains("ORDER BY name ASC"));
        assert!(sql.contains("LIMIT 10"));
    }

    #[test]
    fn test_unified_crud_insert() {
        let executor = UnifiedCrudExecutor::new();
        let query = UnifiedQuery {
            table_name: "users".to_string(),
            operation: CrudOperation::Create,
            conditions: HashMap::new(),
            data: Some(serde_json::json!({
                "name": "Alice",
                "age": 30
            })),
            limit: None,
            offset: None,
            order_by: None,
            ascending: None,
        };

        let sql = executor.build_sql(&query).unwrap();
        assert!(sql.contains("INSERT INTO users"));
        assert!(sql.contains("Alice"));
    }

    #[test]
    fn test_table_health_report() {
        let optimizer = SmartAutoOptimizer::new();
        let report = optimizer.analyze_table_health("large_table", 15_000_000, 2048.0);

        assert_eq!(report.severity, HealthSeverity::Critical);
        assert!(!report.issues.is_empty());
        assert!(!report.suggestions.is_empty());
    }

    #[test]
    fn test_shard_rebalancer() {
        let shards = vec![
            ShardInfo {
                shard_name: "shard_1".to_string(),
                shard_type: "hash".to_string(),
                row_count: 1000,
                size_mb: 100.0,
                created_at: 0,
                is_active: true,
                data_range_start: None,
                data_range_end: None,
            },
            ShardInfo {
                shard_name: "shard_2".to_string(),
                shard_type: "hash".to_string(),
                row_count: 2000,
                size_mb: 200.0,
                created_at: 0,
                is_active: true,
                data_range_start: None,
                data_range_end: None,
            },
        ];

        let rebalancer = ShardRebalancer::new();
        let plan = rebalancer.plan_migration(&shards, 1);

        assert!(!plan.source_shards.is_empty());
        assert!(!plan.target_shards.is_empty());
    }

    #[test]
    fn test_spanning_query_sorted() {
        let mut manager = AutoShardingManager::new();
        manager.register_table(TableConfig {
            table_name: "test".to_string(),
            max_rows: 1000,
            shard_type: ShardStrategy::RowCount { max_rows: 1000 },
            shard_prefix: "test_p".to_string(),
            compression_enabled: false,
            auto_optimize: false,
            optimize_threshold_ms: 1000,
        });

        manager.create_new_shard("test").unwrap();

        let result = manager.spanning_query_sorted("test", "1=1", "id", true, 10, 0).unwrap();
        assert_eq!(result.results.len(), 0);
    }

    #[test]
    fn test_compression_text_field() {
        let mut config = CompressionConfig::default();
        config.fields_to_compress = vec!["description".to_string()];
        config.compress_threshold_bytes = 10;

        let handler = CompressionHandler::with_config(config);
        let text = "This is a very long text that should be compressed by the handler.";

        let compressed = handler.compress_text_field("description", text).unwrap();
        assert_ne!(compressed, text);

        let decompressed = handler.decompress_text_field(&compressed).unwrap();
        assert_eq!(decompressed, text);
    }
}
