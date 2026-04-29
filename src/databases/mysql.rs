use super::{DatabaseConnection, TableSchema};
use anyhow::Result;
use serde_json;
use sqlx::{MySqlPool, Column};
use sqlx::Row;

/// MySQL 数据库连接实现
pub struct MySqlConnection {
    pool: MySqlPool,
}

impl MySqlConnection {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let pool = MySqlPool::connect(connection_string).await?;
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl DatabaseConnection for MySqlConnection {
    async fn get_table_schema(&self, table_name: &str) -> Result<TableSchema> {
        // 获取表结构
        let query = format!(r#"
            SELECT COLUMN_NAME, DATA_TYPE, CHARACTER_MAXIMUM_LENGTH, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_KEY, EXTRA
            FROM INFORMATION_SCHEMA.COLUMNS
            WHERE TABLE_NAME = '{}'
            ORDER BY ORDINAL_POSITION
        "#, table_name);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut fields = vec![];
        for row in rows {
            let name: String = row.try_get("COLUMN_NAME")?;
            let data_type: String = row.try_get("DATA_TYPE")?;
            let length: Option<usize> = row.try_get::<Option<i32>, _>("CHARACTER_MAXIMUM_LENGTH").ok().flatten().map(|v| v as usize);
            let nullable: bool = row.try_get::<String, _>("IS_NULLABLE")? == "YES";
            let default_value: Option<String> = row.try_get("COLUMN_DEFAULT").ok();
            let primary_key: bool = row.try_get::<String, _>("COLUMN_KEY")? == "PRI";
            let auto_increment: bool = row.try_get::<Option<String>, _>("EXTRA").unwrap_or(None).unwrap_or("".to_string()) == "auto_increment";

            fields.push(crate::models::Field {
                name,
                data_type,
                length,
                nullable,
                default_value,
                primary_key,
                auto_increment,
            });
        }

        // 获取索引
        let index_query = format!(r#"
            SELECT INDEX_NAME, COLUMN_NAME, NON_UNIQUE
            FROM INFORMATION_SCHEMA.STATISTICS
            WHERE TABLE_NAME = '{}' AND INDEX_NAME != 'PRIMARY'
            ORDER BY INDEX_NAME, SEQ_IN_INDEX
        "#, table_name);

        let index_rows = sqlx::query(&index_query).fetch_all(&self.pool).await?;
        let mut indexes = vec![];
        let mut current_index = None;
        let mut current_fields = vec![];

        for row in index_rows {
            let index_name: String = row.try_get("INDEX_NAME")?;
            let column_name: String = row.try_get("COLUMN_NAME")?;
            let non_unique: i32 = row.try_get("NON_UNIQUE")?;

            if current_index.as_ref() != Some(&index_name) {
                if let Some(name) = current_index {
                    indexes.push(crate::models::Index {
                        name,
                        fields: current_fields,
                        unique: non_unique == 0,
                    });
                }
                current_index = Some(index_name);
                current_fields = vec![column_name];
            } else {
                current_fields.push(column_name);
            }
        }

        if let Some(name) = current_index {
            indexes.push(crate::models::Index {
                name,
                fields: current_fields,
                unique: true, // 默认为唯一索引
            });
        }

        // 获取外键
        let foreign_key_query = format!(r#"
            SELECT CONSTRAINT_NAME, COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME
            FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE
            WHERE TABLE_NAME = '{}' AND REFERENCED_TABLE_NAME IS NOT NULL
            ORDER BY CONSTRAINT_NAME, ORDINAL_POSITION
        "#, table_name);

        let foreign_key_rows = sqlx::query(&foreign_key_query).fetch_all(&self.pool).await?;
        let mut foreign_keys = vec![];
        let mut current_fk = None;
        let mut current_fields = vec![];
        let mut current_ref_fields = vec![];
        let mut current_ref_table = String::new();

        for row in foreign_key_rows {
            let constraint_name: String = row.try_get("CONSTRAINT_NAME")?;
            let column_name: String = row.try_get("COLUMN_NAME")?;
            let ref_table: String = row.try_get("REFERENCED_TABLE_NAME")?;
            let ref_column: String = row.try_get("REFERENCED_COLUMN_NAME")?;

            if current_fk.as_ref() != Some(&constraint_name) {
                if let Some(name) = current_fk {
                    foreign_keys.push(crate::models::ForeignKey {
                        name,
                        fields: current_fields,
                        reference_table: current_ref_table,
                        reference_fields: current_ref_fields,
                    });
                }
                current_fk = Some(constraint_name);
                current_fields = vec![column_name];
                current_ref_fields = vec![ref_column];
                current_ref_table = ref_table;
            } else {
                current_fields.push(column_name);
                current_ref_fields.push(ref_column);
            }
        }

        if let Some(name) = current_fk {
            foreign_keys.push(crate::models::ForeignKey {
                name,
                fields: current_fields,
                reference_table: current_ref_table,
                reference_fields: current_ref_fields,
            });
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
        let query = "SHOW TABLES";
        let rows = sqlx::query(query).fetch_all(&self.pool).await?;

        let mut tables = vec![];
        for row in rows {
            let table_name: String = row.try_get(0)?;
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
        // 开始事务
        sqlx::query("START TRANSACTION").execute(&self.pool).await?;
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
