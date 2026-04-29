use super::{DatabaseConnection, TableSchema};
use anyhow::Result;
use serde_json;
use sqlx::{PgPool, Column};
use sqlx::Row;

/// PostgreSQL 数据库连接实现
pub struct PostgresConnection {
    pool: PgPool,
}

impl PostgresConnection {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let pool = PgPool::connect(connection_string).await?;
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl DatabaseConnection for PostgresConnection {
    async fn get_table_schema(&self, table_name: &str) -> Result<TableSchema> {
        // 获取表结构
        let query = format!(r#"
            SELECT column_name, data_type, character_maximum_length, is_nullable, column_default, column_name IN (
                SELECT column_name FROM information_schema.key_column_usage 
                WHERE table_name = '{}' AND constraint_name IN (
                    SELECT constraint_name FROM information_schema.table_constraints 
                    WHERE table_name = '{}' AND constraint_type = 'PRIMARY KEY'
                )
            ) as primary_key, 
            column_default LIKE '%nextval%' as auto_increment
            FROM information_schema.columns
            WHERE table_name = '{}'
            ORDER BY ordinal_position
        "#, table_name, table_name, table_name);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut fields = vec![];
        for row in rows {
            let name: String = row.try_get("column_name")?;
            let data_type: String = row.try_get("data_type")?;
            let length: Option<usize> = row.try_get::<Option<i32>, _>("character_maximum_length").ok().flatten().map(|v| v as usize);
            let nullable: bool = row.try_get::<String, _>("is_nullable")? == "YES";
            let default_value: Option<String> = row.try_get("column_default").ok();
            let primary_key: bool = row.try_get("primary_key")?;
            let auto_increment: bool = row.try_get("auto_increment")?;

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
            SELECT indexname, tablename, indisunique, indisprimary
            FROM pg_indexes
            WHERE tablename = '{}'
        "#, table_name);

        let index_rows = sqlx::query(&index_query).fetch_all(&self.pool).await?;
        let mut indexes = vec![];

        for row in index_rows {
            let index_name: String = row.try_get("indexname")?;
            let is_unique: bool = row.try_get("indisunique")?;
            let is_primary: bool = row.try_get("indisprimary")?;

            // 跳过主键索引，因为已经在字段中处理
            if is_primary {
                continue;
            }

            // 获取索引字段
            let index_fields_query = format!(r#"
                SELECT column_name
                FROM information_schema.indexes
                WHERE table_name = '{}' AND index_name = '{}'
                ORDER BY ordinal_position
            "#, table_name, index_name);

            let index_fields_rows = sqlx::query(&index_fields_query).fetch_all(&self.pool).await?;
            let mut index_fields = vec![];

            for field_row in index_fields_rows {
                let column_name: String = field_row.try_get("column_name")?;
                index_fields.push(column_name);
            }

            indexes.push(crate::models::Index {
                name: index_name,
                fields: index_fields,
                unique: is_unique,
            });
        }

        // 获取外键
        let foreign_key_query = format!(r#"
            SELECT constraint_name, column_name, referenced_table_name, referenced_column_name
            FROM information_schema.key_column_usage
            WHERE table_name = '{}' AND referenced_table_name IS NOT NULL
            ORDER BY constraint_name, ordinal_position
        "#, table_name);

        let foreign_key_rows = sqlx::query(&foreign_key_query).fetch_all(&self.pool).await?;
        let mut foreign_keys = vec![];
        let mut current_fk = None;
        let mut current_fields = vec![];
        let mut current_ref_fields = vec![];
        let mut current_ref_table = String::new();

        for row in foreign_key_rows {
            let constraint_name: String = row.try_get("constraint_name")?;
            let column_name: String = row.try_get("column_name")?;
            let ref_table: String = row.try_get("referenced_table_name")?;
            let ref_column: String = row.try_get("referenced_column_name")?;

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
        let query = r#"
            SELECT table_name FROM information_schema.tables 
            WHERE table_schema = 'public'
        "#;
        let rows = sqlx::query(query).fetch_all(&self.pool).await?;

        let mut tables = vec![];
        for row in rows {
            let table_name: String = row.try_get("table_name")?;
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
        sqlx::query("BEGIN").execute(&self.pool).await?;
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
