use super::{DatabaseConnection, TableSchema};
use anyhow::Result;
use serde_json;
use sqlx::{SqlitePool, Column};
use sqlx::Row;

/// SQLite 数据库连接实现
pub struct SqliteConnection {
    pool: SqlitePool,
}

impl SqliteConnection {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let pool = SqlitePool::connect(connection_string).await?;
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl DatabaseConnection for SqliteConnection {
    async fn get_table_schema(&self, table_name: &str) -> Result<TableSchema> {
        // 获取表结构
        let query = format!(r#"
            PRAGMA table_info('{}')
        "#, table_name);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut fields = vec![];
        for row in rows {
            let name: String = row.try_get("name")?;
            let type_str: String = row.try_get("type")?;
            let notnull: i32 = row.try_get("notnull")?;
            let dflt_value: Option<String> = row.try_get("dflt_value").ok();
            let pk: i32 = row.try_get("pk")?;

            // 解析数据类型和长度
            let (data_type, length) = parse_sqlite_type(&type_str);
            let auto_increment = pk > 0 && data_type == "INTEGER";

            fields.push(crate::models::Field {
                name,
                data_type,
                length,
                nullable: notnull == 0,
                default_value: dflt_value,
                primary_key: pk > 0,
                auto_increment,
            });
        }

        // 获取索引
        let index_query = format!(r#"
            PRAGMA index_list('{}')
        "#, table_name);

        let index_rows = sqlx::query(&index_query).fetch_all(&self.pool).await?;
        let mut indexes = vec![];

        for row in index_rows {
            let name: String = row.try_get("name")?;
            let unique: i32 = row.try_get("unique")?;

            // 获取索引字段
            let index_info_query = format!(r#"
                PRAGMA index_info('{}')
            "#, name);

            let index_info_rows = sqlx::query(&index_info_query).fetch_all(&self.pool).await?;
            let mut index_fields = vec![];

            for info_row in index_info_rows {
                let column_name: String = info_row.try_get("name")?;
                index_fields.push(column_name);
            }

            indexes.push(crate::models::Index {
                name,
                fields: index_fields,
                unique: unique == 1,
            });
        }

        // 获取外键
        let foreign_key_query = format!(r#"
            PRAGMA foreign_key_list('{}')
        "#, table_name);

        let foreign_key_rows = sqlx::query(&foreign_key_query).fetch_all(&self.pool).await?;
        let mut foreign_keys: Vec<crate::models::ForeignKey> = vec![];

        for row in foreign_key_rows {
            let id: i32 = row.try_get("id")?;
            let _seq: i32 = row.try_get("seq")?;
            let table: String = row.try_get("table")?;
            let from: String = row.try_get("from")?;
            let to: String = row.try_get("to")?;

            // 外键约束名称
            let constraint_name = format!("fk_{}_{}_{}", table_name, table, id);

            // 检查是否已经添加过该外键
            if let Some(fk) = foreign_keys.iter_mut().find(|fk| fk.name == constraint_name) {
                fk.fields.push(from);
                fk.reference_fields.push(to);
            } else {
                foreign_keys.push(crate::models::ForeignKey {
                    name: constraint_name,
                    fields: vec![from],
                    reference_table: table,
                    reference_fields: vec![to],
                });
            }
        }

        Ok(TableSchema {
            name: table_name.to_string(),
            fields,
            indexes,
            foreign_keys,
        })
    }
    
    async fn get_all_tables(&self) -> Result<Vec<String>> {
        // 获取所有表名
        let query = r#"
            SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'
        "#;

        let rows = sqlx::query(query).fetch_all(&self.pool).await?;
        let mut tables = vec![];

        for row in rows {
            let table_name: String = row.try_get("name")?;
            tables.push(table_name);
        }

        Ok(tables)
    }
    
    async fn execute(&self, sql: &str) -> Result<()> {
        sqlx::query(sql).execute(&self.pool).await?;
        Ok(())
    }
    
    async fn query(&self, sql: &str) -> Result<Vec<serde_json::Value>> {
        // 执行查询并返回结果
        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        let mut results = vec![];

        for row in rows {
            let mut obj = serde_json::Map::new();
            for (idx, column) in row.columns().iter().enumerate() {
                let column_name = column.name().to_string();
                let value = match row.try_get::<Option<String>, _>(idx) {
                    Ok(Some(v)) => serde_json::Value::String(v),
                    Ok(None) => serde_json::Value::Null,
                    Err(_) => match row.try_get::<Option<i32>, _>(idx) {
                        Ok(Some(v)) => serde_json::Value::Number(serde_json::Number::from(v)),
                        Ok(None) => serde_json::Value::Null,
                        Err(_) => match row.try_get::<Option<i64>, _>(idx) {
                            Ok(Some(v)) => serde_json::Value::Number(serde_json::Number::from(v)),
                            Ok(None) => serde_json::Value::Null,
                            Err(_) => match row.try_get::<Option<f32>, _>(idx) {
                                Ok(Some(v)) => serde_json::Value::Number(serde_json::Number::from_f64(v as f64).unwrap()),
                                Ok(None) => serde_json::Value::Null,
                                Err(_) => match row.try_get::<Option<f64>, _>(idx) {
                                    Ok(Some(v)) => serde_json::Value::Number(serde_json::Number::from_f64(v).unwrap()),
                                    Ok(None) => serde_json::Value::Null,
                                    Err(_) => match row.try_get::<Option<bool>, _>(idx) {
                                        Ok(Some(v)) => serde_json::Value::Bool(v),
                                        Ok(None) => serde_json::Value::Null,
                                        Err(_) => serde_json::Value::Null,
                                    },
                                },
                            },
                        },
                    },
                };
                obj.insert(column_name, value);
            }
            results.push(serde_json::Value::Object(obj));
        }

        Ok(results)
    }
    
    async fn begin_transaction(&self) -> Result<()> {
        // 开始事务 - 使用BEGIN IMMEDIATE确保事务立即开始
        sqlx::query("BEGIN IMMEDIATE").execute(&self.pool).await?;
        Ok(())
    }
    
    async fn commit_transaction(&self) -> Result<()> {
        // 提交事务
        sqlx::query("COMMIT").execute(&self.pool).await?;
        Ok(())
    }
    
    async fn rollback_transaction(&self) -> Result<()> {
        // 回滚事务
        sqlx::query("ROLLBACK").execute(&self.pool).await?;
        Ok(())
    }
}

/// 解析SQLite数据类型
fn parse_sqlite_type(type_str: &str) -> (String, Option<usize>) {
    let type_str = type_str.to_uppercase();
    
    if type_str.starts_with("VARCHAR(") {
        let len_str = type_str.trim_start_matches("VARCHAR(").trim_end_matches(")");
        if let Ok(len) = len_str.parse::<usize>() {
            return ("VARCHAR".to_string(), Some(len));
        }
    } else if type_str.starts_with("TEXT(") {
        let len_str = type_str.trim_start_matches("TEXT(").trim_end_matches(")");
        if let Ok(len) = len_str.parse::<usize>() {
            return ("TEXT".to_string(), Some(len));
        }
    } else if type_str.starts_with("INTEGER(") {
        let len_str = type_str.trim_start_matches("INTEGER(").trim_end_matches(")");
        if let Ok(len) = len_str.parse::<usize>() {
            return ("INTEGER".to_string(), Some(len));
        }
    } else if type_str.starts_with("REAL(") {
        let len_str = type_str.trim_start_matches("REAL(").trim_end_matches(")");
        if let Ok(len) = len_str.parse::<usize>() {
            return ("REAL".to_string(), Some(len));
        }
    }
    
    // 处理其他类型
    match type_str.as_str() {
        "INT" | "INTEGER" | "TINYINT" | "SMALLINT" | "MEDIUMINT" | "BIGINT" | "UNSIGNED BIG INT" | "INT2" | "INT8" => {
            ("INTEGER".to_string(), None)
        }
        "CHARACTER(1)" | "NCHAR(1)" | "NATIVE CHARACTER(1)" | "VARCHARACTER(1)" | "NVARCHARACTER(1)" => {
            ("CHAR".to_string(), Some(1))
        }
        "CHARACTER" | "CHAR" | "NCHAR" | "NATIVE CHARACTER" | "VARCHARACTER" | "VARCHAR" | "NVARCHARACTER" | "NVARCHAR" => {
            ("VARCHAR".to_string(), None)
        }
        "TEXT" => {
            ("TEXT".to_string(), None)
        }
        "REAL" | "DOUBLE" | "DOUBLE PRECISION" | "FLOAT" => {
            ("REAL".to_string(), None)
        }
        "NUMERIC" | "DECIMAL" | "BOOLEAN" | "DATE" | "DATETIME" => {
            ("NUMERIC".to_string(), None)
        }
        "BLOB" => {
            ("BLOB".to_string(), None)
        }
        _ => {
            (type_str.to_string(), None)
        }
    }
}
