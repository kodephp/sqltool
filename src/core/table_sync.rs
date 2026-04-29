/// 表级别同步模块

use crate::databases::DatabaseConnection;
use crate::models::{FieldMapping, TableSchema};
use anyhow::Result;

/// 同步模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// 完全同步 - 先清空目标表再插入
    FullSync,
    /// 增量同步 - 只插入新数据
    Incremental,
    /// 更新同步 - 更新已存在的数据
    Update,
    /// 智能同步 - 根据配置自动选择
    Smart,
}

/// 同步配置
#[derive(Debug, Clone)]
pub struct TableSyncConfig {
    /// 同步模式
    pub mode: SyncMode,
    /// 是否同步索引
    pub sync_indexes: bool,
    /// 是否同步外键
    pub sync_foreign_keys: bool,
    /// 冲突处理策略
    pub conflict_strategy: ConflictStrategy,
    /// 批处理大小
    pub batch_size: usize,
    /// 是否验证数据
    pub validate_data: bool,
}

impl Default for TableSyncConfig {
    fn default() -> Self {
        Self {
            mode: SyncMode::Smart,
            sync_indexes: true,
            sync_foreign_keys: true,
            conflict_strategy: ConflictStrategy::Update,
            batch_size: 1000,
            validate_data: true,
        }
    }
}

/// 冲突策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// 跳过已存在的记录
    Skip,
    /// 更新已存在的记录
    Update,
    /// 替换已存在的记录
    Replace,
    /// 报错
    Error,
}

/// 表同步器
pub struct TableSync {
    source_db: Box<dyn DatabaseConnection>,
    target_db: Box<dyn DatabaseConnection>,
    config: TableSyncConfig,
}

impl TableSync {
    pub fn new(
        source_db: Box<dyn DatabaseConnection>,
        target_db: Box<dyn DatabaseConnection>,
        config: TableSyncConfig,
    ) -> Self {
        Self {
            source_db,
            target_db,
            config,
        }
    }

    pub fn with_default_config(
        source_db: Box<dyn DatabaseConnection>,
        target_db: Box<dyn DatabaseConnection>,
    ) -> Self {
        Self::new(source_db, target_db, TableSyncConfig::default())
    }

    /// 同步单个表
    pub async fn sync_table(
        &self,
        source_table: &str,
        target_table: &str,
        mappings: &[FieldMapping],
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();
        
        // 1. 获取源表结构
        let source_schema = self.source_db.get_table_schema(source_table).await?;
        result.source_row_count = self.count_rows(&source_schema).await?;
        
        // 2. 获取目标表结构（如果存在）
        let target_schema = match self.target_db.get_table_schema(target_table).await {
            Ok(schema) => Some(schema),
            Err(_) => None,
        };
        
        // 3. 根据配置和目标表是否存在，决定同步策略
        let target_exists = target_schema.is_some();
        match (&self.config.mode, &target_schema) {
            (SyncMode::FullSync, _) => {
                // 完全同步：先清空目标表，再插入所有数据
                self.full_sync(source_table, target_table, mappings, &source_schema).await?;
                result.skipped_rows = 0;
                result.updated_rows = 0;
            }
            (SyncMode::Incremental, Some(_)) => {
                // 增量同步：只插入不存在的记录
                let (inserted, skipped) = self.incremental_sync(source_table, target_table, mappings).await?;
                result.inserted_rows = inserted;
                result.skipped_rows = skipped;
            }
            (SyncMode::Update, Some(ts)) => {
                // 更新同步：更新已存在的记录
                let updated = self.update_sync(source_table, target_table, mappings, ts).await?;
                result.updated_rows = updated;
            }
            (SyncMode::Smart, None) | (SyncMode::Incremental, None) | (SyncMode::Update, None) => {
                // 目标表不存在或智能模式且目标不存在：创建表并完全同步
                self.full_sync(source_table, target_table, mappings, &source_schema).await?;
            }
            (SyncMode::Smart, Some(ts)) => {
                // 智能模式且目标存在：根据主键进行智能同步
                let (inserted, updated, skipped) = self.smart_sync(source_table, target_table, mappings, ts).await?;
                result.inserted_rows = inserted;
                result.updated_rows = updated;
                result.skipped_rows = skipped;
            }
        }
        
        // 4. 同步索引（如果配置允许）
        if self.config.sync_indexes && !target_exists {
            self.sync_indexes(&source_schema, target_table).await?;
        }
        
        // 5. 同步外键（如果配置允许）
        if self.config.sync_foreign_keys && !target_exists {
            self.sync_foreign_keys(&source_schema, target_table).await?;
        }
        
        result.total_rows = result.inserted_rows + result.updated_rows + result.skipped_rows;
        Ok(result)
    }

    /// 完全同步
    async fn full_sync(
        &self,
        source_table: &str,
        target_table: &str,
        mappings: &[FieldMapping],
        source_schema: &TableSchema,
    ) -> Result<()> {
        // 构建 CREATE TABLE 语句
        let create_sql = self.build_create_table_sql(target_table, source_schema)?;
        self.target_db.execute(&create_sql).await?;
        
        // 清空目标表
        let truncate_sql = format!("DELETE FROM {}", target_table);
        self.target_db.execute(&truncate_sql).await?;
        
        // 查询源数据
        let select_sql = self.build_select_sql(source_table, mappings);
        let rows = self.source_db.query(&select_sql).await?;
        
        // 分批插入
        for chunk in rows.chunks(self.config.batch_size) {
            let insert_sql = self.build_insert_sql(target_table, mappings, chunk.len());
            for row in chunk {
                if let serde_json::Value::Object(obj) = row {
                    let _values = self.extract_values(obj.clone(), mappings);
                    self.target_db.execute(&insert_sql).await?;
                }
            }
        }
        
        Ok(())
    }

    /// 增量同步
    async fn incremental_sync(
        &self,
        source_table: &str,
        target_table: &str,
        mappings: &[FieldMapping],
    ) -> Result<(usize, usize)> {
        let mut inserted = 0;
        let mut skipped = 0;
        
        // 查询源数据
        let select_sql = self.build_select_sql(source_table, mappings);
        let rows = self.source_db.query(&select_sql).await?;
        
        for row in rows {
            if let serde_json::Value::Object(obj) = row {
                // 检查是否已存在
                if self.record_exists(target_table, &obj, mappings).await? {
                    skipped += 1;
                } else {
                    let insert_sql = self.build_insert_sql(target_table, mappings, 1);
                    let _values = self.extract_values(obj, mappings);
                    self.target_db.execute(&insert_sql).await?;
                    inserted += 1;
                }
            }
        }
        
        Ok((inserted, skipped))
    }

    /// 更新同步
    async fn update_sync(
        &self,
        source_table: &str,
        target_table: &str,
        mappings: &[FieldMapping],
        _target_schema: &TableSchema,
    ) -> Result<usize> {
        let mut updated = 0;
        
        // 查询源数据
        let select_sql = self.build_select_sql(source_table, mappings);
        let rows = self.source_db.query(&select_sql).await?;
        
        for row in rows {
            if let serde_json::Value::Object(obj) = row {
                if self.record_exists(target_table, &obj, mappings).await? {
                    let update_sql = self.build_update_sql(target_table, mappings, &obj);
                    self.target_db.execute(&update_sql).await?;
                    updated += 1;
                }
            }
        }
        
        Ok(updated)
    }

    /// 智能同步
    async fn smart_sync(
        &self,
        source_table: &str,
        target_table: &str,
        mappings: &[FieldMapping],
        target_schema: &TableSchema,
    ) -> Result<(usize, usize, usize)> {
        let mut inserted = 0;
        let mut updated = 0;
        let mut skipped = 0;
        
        // 找出主键字段
        let pk_fields: Vec<&str> = target_schema.fields.iter()
            .filter(|f| f.primary_key)
            .map(|f| f.name.as_str())
            .collect();
        
        // 查询源数据
        let select_sql = self.build_select_sql(source_table, mappings);
        let rows = self.source_db.query(&select_sql).await?;
        
        for row in rows {
            if let serde_json::Value::Object(obj) = row {
                let exists = self.record_exists_with_pk(target_table, &obj, mappings, &pk_fields).await?;
                
                match exists {
                    true => {
                        match self.config.conflict_strategy {
                            ConflictStrategy::Skip => skipped += 1,
                            ConflictStrategy::Update | ConflictStrategy::Replace => {
                                let update_sql = self.build_update_sql(target_table, mappings, &obj);
                                self.target_db.execute(&update_sql).await?;
                                updated += 1;
                            }
                            ConflictStrategy::Error => {
                                return Err(anyhow::anyhow!("Record already exists in target table"));
                            }
                        }
                    }
                    false => {
                        let insert_sql = self.build_insert_sql(target_table, mappings, 1);
                        let _values = self.extract_values(obj, mappings);
                        self.target_db.execute(&insert_sql).await?;
                        inserted += 1;
                    }
                }
            }
        }
        
        Ok((inserted, updated, skipped))
    }

    /// 检查记录是否存在
    async fn record_exists(
        &self,
        table: &str,
        _data: &serde_json::Map<String, serde_json::Value>,
        _mappings: &[FieldMapping],
    ) -> Result<bool> {
        // 简化实现：检查是否有任何记录
        let query = format!("SELECT COUNT(*) as count FROM {} LIMIT 1", table);
        let result = self.target_db.query(&query).await?;
        Ok(!result.is_empty())
    }

    /// 根据主键检查记录是否存在
    async fn record_exists_with_pk(
        &self,
        table: &str,
        data: &serde_json::Map<String, serde_json::Value>,
        mappings: &[FieldMapping],
        pk_fields: &[&str],
    ) -> Result<bool> {
        if pk_fields.is_empty() {
            return self.record_exists(table, data, mappings).await;
        }
        
        let conditions: Vec<String> = pk_fields.iter()
            .filter_map(|pk| {
                mappings.iter()
                    .find(|m| m.target_field == *pk)
                    .and_then(|m| data.get(&m.source_field))
                    .map(|v| format!("{} = {}", pk, v))
            })
            .collect();
        
        if conditions.is_empty() {
            return Ok(false);
        }
        
        let query = format!("SELECT COUNT(*) as count FROM {} WHERE {}", table, conditions.join(" AND "));
        let result = self.target_db.query(&query).await?;
        
        Ok(!result.is_empty())
    }

    /// 统计行数
    async fn count_rows(&self, schema: &TableSchema) -> Result<usize> {
        let query = format!("SELECT COUNT(*) as count FROM {}", schema.name);
        let result = self.source_db.query(&query).await?;
        
        if let Some(serde_json::Value::Object(obj)) = result.first() {
            if let Some(serde_json::Value::Number(num)) = obj.get("count") {
                return Ok(num.as_i64().unwrap_or(0) as usize);
            }
        }
        Ok(0)
    }

    /// 构建 CREATE TABLE SQL
    fn build_create_table_sql(&self, table_name: &str, schema: &TableSchema) -> Result<String> {
        let mut field_defs = Vec::new();
        
        for field in &schema.fields {
            let mut def = format!("{} {}", field.name, field.data_type);
            if !field.nullable {
                def.push_str(" NOT NULL");
            }
            if let Some(ref default) = field.default_value {
                def.push_str(&format!(" DEFAULT {}", default));
            }
            if field.auto_increment {
                def.push_str(" AUTOINCREMENT");
            }
            field_defs.push(def);
        }
        
        // 添加主键
        let pk_fields: Vec<&str> = schema.fields.iter()
            .filter(|f| f.primary_key)
            .map(|f| f.name.as_str())
            .collect();
        
        if !pk_fields.is_empty() {
            field_defs.push(format!("PRIMARY KEY ({})", pk_fields.join(", ")));
        }
        
        Ok(format!("CREATE TABLE {} ({})", table_name, field_defs.join(", ")))
    }

    /// 构建 SELECT SQL
    fn build_select_sql(&self, table: &str, mappings: &[FieldMapping]) -> String {
        let fields: String = mappings.iter()
            .map(|m| m.source_field.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        
        format!("SELECT {} FROM {}", fields, table)
    }

    /// 构建 INSERT SQL
    fn build_insert_sql(&self, table: &str, mappings: &[FieldMapping], _row_count: usize) -> String {
        let fields: String = mappings.iter()
            .map(|m| m.target_field.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        
        let placeholders: String = mappings.iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        
        format!("INSERT INTO {} ({}) VALUES ({})", table, fields, placeholders)
    }

    /// 构建 UPDATE SQL
    fn build_update_sql(&self, table: &str, mappings: &[FieldMapping], data: &serde_json::Map<String, serde_json::Value>) -> String {
        let set_clauses: Vec<String> = mappings.iter()
            .filter_map(|m| {
                data.get(&m.source_field)
                    .map(|v| format!("{} = {}", m.target_field, v))
            })
            .collect();
        
        // 假设第一个字段是主键
        if let Some(first_mapping) = mappings.first() {
            if let Some(value) = data.get(&first_mapping.source_field) {
                return format!("UPDATE {} SET {} WHERE {} = {}", 
                    table, 
                    set_clauses.join(", "), 
                    first_mapping.target_field,
                    value
                );
            }
        }
        
        format!("UPDATE {} SET {}", table, set_clauses.join(", "))
    }

    /// 提取值
    fn extract_values(&self, data: serde_json::Map<String, serde_json::Value>, mappings: &[FieldMapping]) -> Vec<serde_json::Value> {
        mappings.iter()
            .filter_map(|m| data.get(&m.source_field).cloned())
            .collect()
    }

    /// 同步索引
    async fn sync_indexes(&self, schema: &TableSchema, target_table: &str) -> Result<()> {
        for index in &schema.indexes {
            let unique = if index.unique { "UNIQUE" } else { "" };
            let fields = index.fields.join(", ");
            let sql = format!("CREATE {} INDEX {} ON {} ({})", unique, index.name, target_table, fields);
            self.target_db.execute(&sql).await?;
        }
        Ok(())
    }

    /// 同步外键
    async fn sync_foreign_keys(&self, schema: &TableSchema, target_table: &str) -> Result<()> {
        for fk in &schema.foreign_keys {
            let sql = format!(
                "ALTER TABLE {} ADD FOREIGN KEY ({}) REFERENCES {}({})",
                target_table,
                fk.fields.join(", "),
                fk.reference_table,
                fk.reference_fields.join(", ")
            );
            self.target_db.execute(&sql).await?;
        }
        Ok(())
    }
}

/// 同步结果
#[derive(Debug, Clone, Default)]
pub struct SyncResult {
    pub total_rows: usize,
    pub inserted_rows: usize,
    pub updated_rows: usize,
    pub skipped_rows: usize,
    pub source_row_count: usize,
}

impl SyncResult {
    pub fn format(&self) -> String {
        format!(
            "同步完成: 总行数={}, 新增={}, 更新={}, 跳过={}",
            self.total_rows, self.inserted_rows, self.updated_rows, self.skipped_rows
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_default() {
        let config = TableSyncConfig::default();
        assert_eq!(config.mode, SyncMode::Smart);
        assert!(config.sync_indexes);
        assert!(config.sync_foreign_keys);
        assert_eq!(config.conflict_strategy, ConflictStrategy::Update);
        assert_eq!(config.batch_size, 1000);
    }

    #[test]
    fn test_sync_result_format() {
        let result = SyncResult {
            total_rows: 100,
            inserted_rows: 50,
            updated_rows: 30,
            skipped_rows: 20,
            source_row_count: 100,
        };
        
        let formatted = result.format();
        assert!(formatted.contains("100"));
        assert!(formatted.contains("50"));
        assert!(formatted.contains("30"));
        assert!(formatted.contains("20"));
        
        println!("{}", formatted);
    }
}
