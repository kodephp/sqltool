/// 增量数据同步模块

use crate::databases::DatabaseConnection;
use crate::models::FieldMapping;
use anyhow::Result;

/// 增量同步配置
#[derive(Debug, Clone)]
pub struct IncrementalConfig {
    /// 用于跟踪变化的字段（如 updated_at, timestamp 等）
    pub track_field: String,
    /// 同步间隔（秒）
    pub sync_interval: u64,
    /// 是否删除目标表中已删除的源数据
    pub sync_deletions: bool,
    /// 冲突解决策略
    pub conflict_resolution: ConflictResolution,
}

impl Default for IncrementalConfig {
    fn default() -> Self {
        Self {
            track_field: "updated_at".to_string(),
            sync_interval: 300, // 5分钟
            sync_deletions: false,
            conflict_resolution: ConflictResolution::SourceWins,
        }
    }
}

/// 冲突解决策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    /// 源数据优先
    SourceWins,
    /// 目标数据优先
    TargetWins,
    /// 合并数据
    Merge,
    /// 报错
    Error,
}

/// 增量同步器
pub struct IncrementalSync {
    source_db: Box<dyn DatabaseConnection>,
    target_db: Box<dyn DatabaseConnection>,
    config: IncrementalConfig,
    last_sync_time: Option<String>,
}

impl IncrementalSync {
    pub fn new(
        source_db: Box<dyn DatabaseConnection>,
        target_db: Box<dyn DatabaseConnection>,
        config: IncrementalConfig,
    ) -> Self {
        Self {
            source_db,
            target_db,
            config,
            last_sync_time: None,
        }
    }

    /// 设置上次同步时间
    pub fn set_last_sync_time(&mut self, time: &str) {
        self.last_sync_time = Some(time.to_string());
    }

    /// 执行增量同步
    pub async fn sync(
        &self,
        source_table: &str,
        target_table: &str,
        mappings: &[FieldMapping],
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();
        
        // 获取上次同步时间
        let last_sync = self.last_sync_time.as_deref().unwrap_or("1970-01-01 00:00:00");
        
        // 1. 同步新增和更新的数据
        let updated_rows = self.sync_updates(source_table, target_table, mappings, last_sync).await?;
        result.updated_rows = updated_rows;
        
        // 2. 如果需要，同步删除的数据
        if self.config.sync_deletions {
            let deleted_rows = self.sync_deletions(source_table, target_table, mappings).await?;
            result.deleted_rows = deleted_rows;
        }
        
        // 3. 获取当前同步时间
        result.sync_time = self.get_current_timestamp().await?;
        
        Ok(result)
    }

    /// 同步新增和更新的数据
    async fn sync_updates(
        &self,
        source_table: &str,
        target_table: &str,
        mappings: &[FieldMapping],
        last_sync: &str,
    ) -> Result<usize> {
        // 构建查询条件
        let track_field = &self.config.track_field;
        let query = format!(
            "SELECT * FROM {} WHERE {} > '{}'",
            source_table, track_field, last_sync
        );
        
        // 查询源数据
        let rows = self.source_db.query(&query).await?;
        let mut updated_count = 0;
        
        for row in rows {
            if let serde_json::Value::Object(obj) = row {
                // 检查目标表是否已存在该记录
                let exists = self.check_record_exists(target_table, &obj, mappings).await?;
                
                if exists {
                    // 更新记录
                    self.update_record(target_table, &obj, mappings).await?;
                } else {
                    // 插入新记录
                    self.insert_record(target_table, &obj, mappings).await?;
                }
                updated_count += 1;
            }
        }
        
        Ok(updated_count)
    }

    /// 同步删除的数据
    async fn sync_deletions(
        &self,
        source_table: &str,
        target_table: &str,
        mappings: &[FieldMapping],
    ) -> Result<usize> {
        // 获取目标表的所有记录ID
        let target_ids = self.get_all_ids(target_table, mappings).await?;
        
        // 获取源表的所有记录ID
        let source_ids = self.get_all_ids(source_table, mappings).await?;
        
        // 找出需要删除的记录
        let mut deleted_count = 0;
        for id in &target_ids {
            if !source_ids.contains(id) {
                self.delete_record(target_table, id, mappings).await?;
                deleted_count += 1;
            }
        }
        
        Ok(deleted_count)
    }

    /// 检查记录是否存在于目标表
    async fn check_record_exists(
        &self,
        target_table: &str,
        data: &serde_json::Map<String, serde_json::Value>,
        mappings: &[FieldMapping],
    ) -> Result<bool> {
        // 获取主键映射
        if let Some(pk_mapping) = mappings.iter().find(|m| m.source_field == "id") {
            if let Some(id_value) = data.get("id") {
                let query = format!(
                    "SELECT COUNT(*) as count FROM {} WHERE {} = {}",
                    target_table,
                    pk_mapping.target_field,
                    id_value
                );
                let result = self.target_db.query(&query).await?;
                if let Some(serde_json::Value::Object(obj)) = result.first() {
                    if let Some(serde_json::Value::Number(count)) = obj.get("count") {
                        return Ok(count.as_i64().unwrap_or(0) > 0);
                    }
                }
            }
        }
        Ok(false)
    }

    /// 更新目标表记录
    async fn update_record(
        &self,
        target_table: &str,
        data: &serde_json::Map<String, serde_json::Value>,
        mappings: &[FieldMapping],
    ) -> Result<()> {
        let mut set_clauses = vec![];
        let mut where_clause = String::new();
        
        for mapping in mappings {
            if let Some(value) = data.get(&mapping.source_field) {
                if mapping.source_field == "id" {
                    where_clause = format!("{} = {}", mapping.target_field, value);
                } else {
                    set_clauses.push(format!("{} = {}", mapping.target_field, value));
                }
            }
        }
        
        if !set_clauses.is_empty() && !where_clause.is_empty() {
            let query = format!(
                "UPDATE {} SET {} WHERE {}",
                target_table,
                set_clauses.join(", "),
                where_clause
            );
            self.target_db.execute(&query).await?;
        }
        
        Ok(())
    }

    /// 插入新记录到目标表
    async fn insert_record(
        &self,
        target_table: &str,
        data: &serde_json::Map<String, serde_json::Value>,
        mappings: &[FieldMapping],
    ) -> Result<()> {
        let mut fields = vec![];
        let mut values = vec![];
        
        for mapping in mappings {
            if let Some(value) = data.get(&mapping.source_field) {
                fields.push(mapping.target_field.clone());
                values.push(value.to_string());
            }
        }
        
        if !fields.is_empty() {
            let query = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                target_table,
                fields.join(", "),
                values.join(", ")
            );
            self.target_db.execute(&query).await?;
        }
        
        Ok(())
    }

    /// 删除目标表记录
    async fn delete_record(
        &self,
        target_table: &str,
        id: &str,
        mappings: &[FieldMapping],
    ) -> Result<()> {
        if let Some(pk_mapping) = mappings.iter().find(|m| m.source_field == "id") {
            let query = format!(
                "DELETE FROM {} WHERE {} = {}",
                target_table,
                pk_mapping.target_field,
                id
            );
            self.target_db.execute(&query).await?;
        }
        Ok(())
    }

    /// 获取表的所有记录ID
    async fn get_all_ids(
        &self,
        table: &str,
        mappings: &[FieldMapping],
    ) -> Result<Vec<String>> {
        let mut ids = vec![];
        
        if let Some(pk_mapping) = mappings.iter().find(|m| m.source_field == "id") {
            let query = format!("SELECT {} FROM {}", pk_mapping.target_field, table);
            let rows = self.source_db.query(&query).await?;
            
            for row in rows {
                if let serde_json::Value::Object(obj) = row {
                    if let Some(value) = obj.get(&pk_mapping.target_field) {
                        ids.push(value.to_string());
                    }
                }
            }
        }
        
        Ok(ids)
    }

    /// 获取当前时间戳
    async fn get_current_timestamp(&self) -> Result<String> {
        let result = self.source_db.query("SELECT datetime('now') as now").await?;
        if let Some(serde_json::Value::Object(obj)) = result.first() {
            if let Some(serde_json::Value::String(timestamp)) = obj.get("now") {
                return Ok(timestamp.clone());
            }
        }
        Ok(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
    }
}

/// 同步结果
#[derive(Debug, Clone, Default)]
pub struct SyncResult {
    pub updated_rows: usize,
    pub deleted_rows: usize,
    pub sync_time: String,
}

impl SyncResult {
    pub fn total_changes(&self) -> usize {
        self.updated_rows + self.deleted_rows
    }

    pub fn format(&self) -> String {
        format!(
            "Sync completed: {} rows updated, {} rows deleted, total: {} changes at {}",
            self.updated_rows,
            self.deleted_rows,
            self.total_changes(),
            self.sync_time
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_connection, DatabaseType, FieldMapping};

    #[tokio::test]
    async fn test_incremental_sync_config() {
        let config = IncrementalConfig::default();
        assert_eq!(config.track_field, "updated_at");
        assert_eq!(config.sync_interval, 300);
        assert!(!config.sync_deletions);
        assert_eq!(config.conflict_resolution, ConflictResolution::SourceWins);
    }

    #[tokio::test]
    async fn test_sync_result() {
        let result = SyncResult {
            updated_rows: 10,
            deleted_rows: 2,
            sync_time: "2024-01-01 12:00:00".to_string(),
        };
        
        assert_eq!(result.total_changes(), 12);
        assert!(result.format().contains("10 rows updated"));
        assert!(result.format().contains("2 rows deleted"));
    }
}