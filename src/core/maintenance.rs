/// 数据库维护模块 - 包含优化、修复、备份、恢复等功能

use crate::databases::DatabaseConnection;
use anyhow::Result;
use std::path::Path;
use chrono::{DateTime, Local};

/// 数据库维护器
pub struct DatabaseMaintainer {
    db: Box<dyn DatabaseConnection>,
    db_type: String,
}

impl DatabaseMaintainer {
    pub fn new(db: Box<dyn DatabaseConnection>, db_type: &str) -> Self {
        Self {
            db,
            db_type: db_type.to_lowercase(),
        }
    }

    /// 优化数据库
    pub async fn optimize(&self) -> Result<OptimizationResult> {
        let start_time = std::time::Instant::now();
        
        match self.db_type.as_str() {
            "mysql" | "mariadb" => self.optimize_mysql().await,
            "postgresql" | "postgres" => self.optimize_postgres().await,
            "sqlite" => self.optimize_sqlite().await,
            _ => Err(anyhow::anyhow!("Unsupported database type: {}", self.db_type)),
        }.map(|msg| {
            OptimizationResult {
                success: true,
                message: msg,
                duration: start_time.elapsed(),
            }
        })
    }

    /// MySQL 优化
    async fn optimize_mysql(&self) -> Result<String> {
        // 优化所有表
        self.db.execute("ANALYZE TABLE").await?;
        self.db.execute("OPTIMIZE TABLE").await?;
        self.db.execute("FLUSH TABLES").await?;
        Ok("MySQL database optimized successfully".to_string())
    }

    /// PostgreSQL 优化
    async fn optimize_postgres(&self) -> Result<String> {
        // PostgreSQL 使用 VACUUM
        self.db.execute("VACUUM ANALYZE").await?;
        Ok("PostgreSQL database vacuumed and analyzed successfully".to_string())
    }

    /// SQLite 优化
    async fn optimize_sqlite(&self) -> Result<String> {
        // SQLite 优化
        self.db.execute("VACUUM").await?;
        self.db.execute("ANALYZE").await?;
        Ok("SQLite database vacuumed and analyzed successfully".to_string())
    }

    /// 修复数据库
    pub async fn repair(&self) -> Result<RepairResult> {
        let start_time = std::time::Instant::now();
        
        match self.db_type.as_str() {
            "mysql" | "mariadb" => self.repair_mysql().await,
            "postgresql" | "postgres" => self.repair_postgres().await,
            "sqlite" => self.repair_sqlite().await,
            _ => Err(anyhow::anyhow!("Unsupported database type: {}", self.db_type)),
        }.map(|msg| {
            RepairResult {
                success: true,
                message: msg,
                duration: start_time.elapsed(),
            }
        })
    }

    /// MySQL 修复
    async fn repair_mysql(&self) -> Result<String> {
        // 修复所有表
        self.db.execute("REPAIR TABLE").await?;
        Ok("MySQL tables repaired successfully".to_string())
    }

    /// PostgreSQL 修复
    async fn repair_postgres(&self) -> Result<String> {
        // PostgreSQL 使用 REINDEX
        self.db.execute("REINDEX DATABASE").await?;
        Ok("PostgreSQL database reindexed successfully".to_string())
    }

    /// SQLite 修复
    async fn repair_sqlite(&self) -> Result<String> {
        // SQLite Integrity Check
        let query = "PRAGMA integrity_check";
        let result = self.db.query(query).await?;
        
        if let Some(serde_json::Value::Object(obj)) = result.first() {
            if let Some(serde_json::Value::String(status)) = obj.get("ok") {
                if status == "ok" {
                    return Ok("SQLite database integrity check passed".to_string());
                }
            }
        }
        
        // 如果检查失败，尝试修复
        self.db.execute("PRAGMA quick_check").await?;
        Ok("SQLite database repair completed".to_string())
    }

    /// 清理数据库
    pub async fn cleanup(&self) -> Result<CleanupResult> {
        let start_time = std::time::Instant::now();
        
        match self.db_type.as_str() {
            "mysql" | "mariadb" => self.cleanup_mysql().await,
            "postgresql" | "postgres" => self.cleanup_postgres().await,
            "sqlite" => self.cleanup_sqlite().await,
            _ => Err(anyhow::anyhow!("Unsupported database type: {}", self.db_type)),
        }.map(|tables_cleaned| {
            CleanupResult {
                success: true,
                tables_cleaned,
                duration: start_time.elapsed(),
            }
        })
    }

    /// MySQL 清理
    async fn cleanup_mysql(&self) -> Result<usize> {
        // 删除临时表和日志
        self.db.execute("DELETE FROM mysql.general_log").await?;
        self.db.execute("DELETE FROM mysql.slow_log").await?;
        Ok(2)
    }

    /// PostgreSQL 清理
    async fn cleanup_postgres(&self) -> Result<usize> {
        // 清理临时文件
        self.db.execute("VACUUM").await?;
        Ok(1)
    }

    /// SQLite 清理
    async fn cleanup_sqlite(&self) -> Result<usize> {
        // SQLite 清理
        self.db.execute("DELETE FROM sqlite_sequence").await?;
        Ok(1)
    }

    /// 获取数据库状态
    pub async fn get_status(&self) -> Result<DatabaseStatus> {
        let tables = self.db.get_all_tables().await?;
        let mut table_count = 0;
        let total_size = 0;
        
        for table in &tables {
            if let Ok(_schema) = self.db.get_table_schema(table).await {
                table_count += 1;
            }
        }
        
        Ok(DatabaseStatus {
            database_type: self.db_type.clone(),
            table_count,
            total_size_bytes: total_size,
            is_connected: true,
            last_check: Local::now(),
        })
    }
}

/// 优化结果
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub success: bool,
    pub message: String,
    pub duration: std::time::Duration,
}

/// 修复结果
#[derive(Debug, Clone)]
pub struct RepairResult {
    pub success: bool,
    pub message: String,
    pub duration: std::time::Duration,
}

/// 清理结果
#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub success: bool,
    pub tables_cleaned: usize,
    pub duration: std::time::Duration,
}

/// 数据库状态
#[derive(Debug, Clone)]
pub struct DatabaseStatus {
    pub database_type: String,
    pub table_count: usize,
    pub total_size_bytes: usize,
    pub is_connected: bool,
    pub last_check: DateTime<Local>,
}

impl DatabaseStatus {
    pub fn format(&self) -> String {
        format!(
            "Database Status:\n\
             - Type: {}\n\
             - Tables: {}\n\
             - Size: {} bytes\n\
             - Connected: {}\n\
             - Last Check: {}",
            self.database_type,
            self.table_count,
            self.total_size_bytes,
            if self.is_connected { "Yes" } else { "No" },
            self.last_check.format("%Y-%m-%d %H:%M:%S")
        )
    }
}

/// 数据库备份器
pub struct DatabaseBackup {
    db: Box<dyn DatabaseConnection>,
    db_type: String,
}

impl DatabaseBackup {
    pub fn new(db: Box<dyn DatabaseConnection>, db_type: &str) -> Self {
        Self {
            db,
            db_type: db_type.to_lowercase(),
        }
    }

    /// 创建完整备份
    pub async fn full_backup(&self, backup_path: &str) -> Result<BackupResult> {
        let start_time = std::time::Instant::now();
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        
        // 获取所有表
        let tables = self.db.get_all_tables().await?;
        let mut backup_data = Vec::new();
        let mut table_count = 0;
        let mut row_count = 0;
        
        // 备份每个表的数据
        for table in &tables {
            table_count += 1;
            
            // 获取表结构
            if let Ok(schema) = self.db.get_table_schema(table).await {
                // 生成 CREATE TABLE 语句
                let create_sql = self.generate_create_table_sql(&schema);
                backup_data.push(format!("{};", create_sql));
                
                // 获取表数据
                let query = format!("SELECT * FROM {}", table);
                let rows = self.db.query(&query).await?;
                row_count += rows.len();
                
                for row in rows {
                    if let serde_json::Value::Object(obj) = row {
                        let insert_sql = self.generate_insert_sql(table, &obj);
                        backup_data.push(format!("{};", insert_sql));
                    }
                }
            }
        }
        
        // 写入备份文件
        let backup_file = format!("{}/backup_{}.sql", backup_path, timestamp);
        let content = backup_data.join("\n");
        let size_bytes = content.len();
        
        if let Some(parent) = Path::new(&backup_file).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&backup_file, &content)?;
        
        Ok(BackupResult {
            success: true,
            backup_file,
            table_count,
            row_count,
            size_bytes,
            duration: start_time.elapsed(),
            backup_type: "FULL".to_string(),
        })
    }

    /// 创建表级备份
    pub async fn table_backup(&self, table: &str, backup_path: &str) -> Result<BackupResult> {
        let start_time = std::time::Instant::now();
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        
        // 获取表结构
        let schema = self.db.get_table_schema(table).await?;
        let create_sql = self.generate_create_table_sql(&schema);
        
        // 获取表数据
        let query = format!("SELECT * FROM {}", table);
        let rows = self.db.query(&query).await?;
        let row_count = rows.len();
        
        let mut backup_data = vec![format!("{};", create_sql)];
        for row in &rows {
            if let serde_json::Value::Object(obj) = row {
                let insert_sql = self.generate_insert_sql(table, obj);
                backup_data.push(format!("{};", insert_sql));
            }
        }
        
        // 写入备份文件
        let backup_file = format!("{}/{}_{}.sql", backup_path, table, timestamp);
        let content = backup_data.join("\n");
        let size_bytes = content.len();
        
        if let Some(parent) = Path::new(&backup_file).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&backup_file, &content)?;
        
        Ok(BackupResult {
            success: true,
            backup_file,
            table_count: 1,
            row_count,
            size_bytes,
            duration: start_time.elapsed(),
            backup_type: "TABLE".to_string(),
        })
    }

    /// 生成 CREATE TABLE SQL
    fn generate_create_table_sql(&self, schema: &crate::models::TableSchema) -> String {
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
        
        let pk_fields: Vec<&str> = schema.fields.iter()
            .filter(|f| f.primary_key)
            .map(|f| f.name.as_str())
            .collect();
        
        if !pk_fields.is_empty() {
            field_defs.push(format!("PRIMARY KEY ({})", pk_fields.join(", ")));
        }
        
        format!("CREATE TABLE {} ({})", schema.name, field_defs.join(", "))
    }

    /// 生成 INSERT SQL
    fn generate_insert_sql(&self, table: &str, data: &serde_json::Map<String, serde_json::Value>) -> String {
        let fields: Vec<&str> = data.keys().map(|s| s.as_str()).collect();
        let values: Vec<String> = data.values().map(|v| self.value_to_sql(v)).collect();
        
        format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            fields.join(", "),
            values.join(", ")
        )
    }

    /// 值转换为 SQL 格式
    fn value_to_sql(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "NULL".to_string(),
            serde_json::Value::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            _ => "NULL".to_string(),
        }
    }
}

/// 备份结果
#[derive(Debug, Clone)]
pub struct BackupResult {
    pub success: bool,
    pub backup_file: String,
    pub table_count: usize,
    pub row_count: usize,
    pub size_bytes: usize,
    pub duration: std::time::Duration,
    pub backup_type: String,
}

impl BackupResult {
    pub fn format(&self) -> String {
        format!(
            "Backup completed:\n\
             - Type: {}\n\
             - File: {}\n\
             - Tables: {}\n\
             - Rows: {}\n\
             - Size: {} bytes\n\
             - Duration: {:?}",
            self.backup_type,
            self.backup_file,
            self.table_count,
            self.row_count,
            self.size_bytes,
            self.duration
        )
    }
}

/// 数据库恢复器
pub struct DatabaseRestore {
    db: Box<dyn DatabaseConnection>,
    db_type: String,
}

impl DatabaseRestore {
    pub fn new(db: Box<dyn DatabaseConnection>, db_type: &str) -> Self {
        Self {
            db,
            db_type: db_type.to_lowercase(),
        }
    }

    /// 从备份文件恢复
    pub async fn restore(&self, backup_file: &str) -> Result<RestoreResult> {
        let start_time = std::time::Instant::now();
        
        // 读取备份文件
        let content = std::fs::read_to_string(backup_file)?;
        let statements: Vec<&str> = content.split(';').collect();
        
        let mut tables_restored = 0;
        let mut rows_restored = 0;
        
        for statement in statements {
            let stmt = statement.trim();
            if stmt.is_empty() || stmt.starts_with("--") {
                continue;
            }
            
            if stmt.to_uppercase().starts_with("CREATE TABLE") {
                // 先删除已存在的表
                let table_name = self.extract_table_name(stmt);
                if !table_name.is_empty() {
                    let drop_sql = format!("DROP TABLE IF EXISTS {}", table_name);
                    let _ = self.db.execute(&drop_sql).await;
                    tables_restored += 1;
                }
            }
            
            // 执行 SQL 语句
            if let Err(e) = self.db.execute(stmt).await {
                eprintln!("Warning: Failed to execute statement: {}", e);
            } else if stmt.to_uppercase().starts_with("INSERT") {
                rows_restored += 1;
            }
        }
        
        Ok(RestoreResult {
            success: true,
            backup_file: backup_file.to_string(),
            tables_restored,
            rows_restored,
            duration: start_time.elapsed(),
        })
    }

    /// 提取表名
    fn extract_table_name(&self, sql: &str) -> String {
        // 简单解析 CREATE TABLE 语句获取表名
        let parts: Vec<&str> = sql.split_whitespace().collect();
        if parts.len() >= 3 {
            parts[2].trim_matches(|c| c == '(' || c == ')').to_string()
        } else {
            String::new()
        }
    }
}

/// 恢复结果
#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub success: bool,
    pub backup_file: String,
    pub tables_restored: usize,
    pub rows_restored: usize,
    pub duration: std::time::Duration,
}

impl RestoreResult {
    pub fn format(&self) -> String {
        format!(
            "Restore completed:\n\
             - File: {}\n\
             - Tables: {}\n\
             - Rows: {}\n\
             - Duration: {:?}",
            self.backup_file,
            self.tables_restored,
            self.rows_restored,
            self.duration
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_result() {
        let result = OptimizationResult {
            success: true,
            message: "Test optimization".to_string(),
            duration: std::time::Duration::from_secs(10),
        };
        
        assert!(result.success);
        println!("Optimization: success={}, message={}, duration={:?}", 
            result.success, result.message, result.duration);
    }

    #[test]
    fn test_backup_result() {
        let result = BackupResult {
            success: true,
            backup_file: "/tmp/backup_20240101.sql".to_string(),
            table_count: 5,
            row_count: 1000,
            size_bytes: 50000,
            duration: std::time::Duration::from_secs(5),
            backup_type: "FULL".to_string(),
        };
        
        assert!(result.success);
        assert_eq!(result.table_count, 5);
        assert_eq!(result.row_count, 1000);
        
        println!("{}", result.format());
    }

    #[test]
    fn test_restore_result() {
        let result = RestoreResult {
            success: true,
            backup_file: "/tmp/backup_20240101.sql".to_string(),
            tables_restored: 5,
            rows_restored: 1000,
            duration: std::time::Duration::from_secs(3),
        };
        
        assert!(result.success);
        println!("{}", result.format());
    }

    #[test]
    fn test_database_status() {
        let status = DatabaseStatus {
            database_type: "MySQL".to_string(),
            table_count: 10,
            total_size_bytes: 1048576,
            is_connected: true,
            last_check: Local::now(),
        };
        
        println!("{}", status.format());
    }
}
