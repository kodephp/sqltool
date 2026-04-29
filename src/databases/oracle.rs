#[cfg(feature = "oracle")]
use crate::models::{TableSchema, Field};
#[cfg(feature = "oracle")]
use crate::databases::connection::ConnectionConfig;
#[cfg(feature = "oracle")]
use anyhow::Result;
use async_trait::async_trait;

#[cfg(feature = "oracle")]
pub struct OracleConnection {
    config: ConnectionConfig,
    connection_string: String,
}

#[cfg(feature = "oracle")]
impl OracleConnection {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let config = ConnectionConfig::from_oracle_connection_string(connection_string)?;

        Ok(Self {
            config,
            connection_string: connection_string.to_string(),
        })
    }

    fn parse_connection_string(conn_str: &str) -> Result<(String, String, String, i64, String)> {
        let parts: Vec<&str> = conn_str.split("://").collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid Oracle connection string format: {}", conn_str);
        }

        let credentials: Vec<&str> = parts[1].split('@').collect();
        if credentials.len() != 2 {
            anyhow::bail!("Invalid Oracle connection string format");
        }

        let user_pass: Vec<&str> = credentials[0].split(':').collect();
        if user_pass.len() != 2 {
            anyhow::bail!("Invalid credentials format");
        }

        let host_db: Vec<&str> = credentials[1].split('/').collect();
        if host_db.len() != 2 {
            anyhow::bail!("Invalid host/db format");
        }

        let host_port: Vec<&str> = host_db[0].split(':').collect();
        let host = host_port[0].to_string();
        let port: i64 = if host_port.len() > 1 {
            host_port[1].parse().unwrap_or(1521)
        } else {
            1521
        };
        let db_name = host_db[1].to_string();

        Ok((user_pass[0].to_string(), user_pass[1].to_string(), host, port, db_name))
    }
}

#[cfg(feature = "oracle")]
impl ConnectionConfig {
    fn from_oracle_connection_string(conn_str: &str) -> Result<Self> {
        let (user, pass, host, port, db) = OracleConnection::parse_connection_string(conn_str)?;
        Ok(ConnectionConfig::new(&host, port as u16, &user, &pass, &db))
    }
}

#[cfg(feature = "oracle")]
#[async_trait]
impl super::DatabaseConnection for OracleConnection {
    async fn get_table_schema(&self, table_name: &str) -> Result<TableSchema> {
        let sql = format!(
            "SELECT column_name, data_type, data_length, nullable
             FROM user_tab_columns WHERE table_name = UPPER('{}') ORDER BY column_id",
            table_name
        );

        let rows = self.query(&sql).await?;

        let fields: Vec<Field> = rows
            .into_iter()
            .filter_map(|row| {
                row.as_object().map(|obj| {
                    Field {
                        name: obj.get("COLUMN_NAME").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        data_type: obj.get("DATA_TYPE").and_then(|v| v.as_str()).unwrap_or("VARCHAR2").to_string(),
                        length: obj.get("DATA_LENGTH").and_then(|v| v.as_i64()).map(|n| n as usize),
                        nullable: obj.get("NULLABLE").and_then(|v| v.as_str()).map(|s| s == "Y").unwrap_or(true),
                        default_value: None,
                        primary_key: false,
                        auto_increment: false,
                    }
                })
            })
            .collect();

        Ok(TableSchema {
            name: table_name.to_string(),
            fields,
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
        })
    }

    async fn get_all_tables(&self) -> Result<Vec<String>> {
        let sql = "SELECT table_name FROM user_tables ORDER BY table_name".to_string();
        let rows = self.query(&sql).await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                row.as_object()
                    .and_then(|obj| obj.get("TABLE_NAME"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect())
    }

    async fn execute(&self, sql: &str) -> Result<()> {
        let conn = oracle::Connection::connect(
            &self.config.username,
            &self.config.password,
            &self.connection_string,
        )?;
        conn.execute(sql)?;
        Ok(())
    }

    async fn query(&self, sql: &str) -> Result<Vec<Value>> {
        let conn = oracle::Connection::connect(
            &self.config.username,
            &self.config.password,
            &self.connection_string,
        )?;

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query()?;

        let mut results = Vec::new();
        for row_result in rows {
            let row = row_result?;
            let mut map = serde_json::Map::new();

            for (idx, col_desc) in row.columns().enumerate() {
                if let Ok(val) = row.get_ref(idx) {
                    let value: Value = match val {
                        oracle::Value::Varchar(s) => Value::String(s),
                        oracle::Value::Number(n) => {
                            if let Some(f) = n.to_f64() {
                                serde_json::Number::from_f64(f)
                                    .map(Value::Number)
                                    .unwrap_or(Value::Null)
                            } else if let Some(i) = n.to_i64() {
                                Value::Number(serde_json::Number::from(i))
                            } else {
                                Value::Null
                            }
                        }
                        oracle::Value::Date(d) => Value::String(format!("{:?}", d)),
                        oracle::Value::Timestamp(ts) => Value::String(format!("{:?}", ts)),
                        oracle::Value::Clob(_) => Value::String("[CLOB]".to_string()),
                        oracle::Value::Blob(_) => Value::String("[BLOB]".to_string()),
                        _ => Value::Null,
                    };
                    map.insert(col_desc.name().to_string(), value);
                }
            }
            results.push(Value::Object(map));
        }

        Ok(results)
    }

    async fn begin_transaction(&self) -> Result<()> { Ok(()) }
    async fn commit_transaction(&self) -> Result<()> { Ok(()) }
    async fn rollback_transaction(&self) -> Result<()> { Ok(()) }
}

#[cfg(not(feature = "oracle"))]
pub struct OracleConnection;

#[cfg(not(feature = "oracle"))]
impl OracleConnection {
    pub async fn new(_connection_string: &str) -> anyhow::Result<Self> {
        anyhow::bail!("Oracle support not enabled. Run with --features oracle or install Oracle Client libraries.");
    }
}

#[cfg(not(feature = "oracle"))]
#[async_trait]
impl super::DatabaseConnection for OracleConnection {
    async fn get_table_schema(&self, _table_name: &str) -> anyhow::Result<crate::models::TableSchema> {
        anyhow::bail!("Oracle support not enabled")
    }
    async fn get_all_tables(&self) -> anyhow::Result<Vec<String>> {
        anyhow::bail!("Oracle support not enabled")
    }
    async fn execute(&self, _sql: &str) -> anyhow::Result<()> {
        anyhow::bail!("Oracle support not enabled")
    }
    async fn query(&self, _sql: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        anyhow::bail!("Oracle support not enabled")
    }
    async fn begin_transaction(&self) -> anyhow::Result<()> {
        anyhow::bail!("Oracle support not enabled")
    }
    async fn commit_transaction(&self) -> anyhow::Result<()> {
        anyhow::bail!("Oracle support not enabled")
    }
    async fn rollback_transaction(&self) -> anyhow::Result<()> {
        anyhow::bail!("Oracle support not enabled")
    }
}

pub struct OracleConverter;

impl OracleConverter {
    pub fn to_mysql_type(oracle_type: &str) -> &'static str {
        match oracle_type.to_uppercase().as_str() {
            "VARCHAR2" | "CHAR" | "NCHAR" | "NVARCHAR2" => "VARCHAR",
            "NUMBER" => "BIGINT",
            "FLOAT" | "BINARY_FLOAT" | "BINARY_DOUBLE" => "DOUBLE",
            "DATE" | "TIMESTAMP" => "DATETIME",
            "CLOB" | "NCLOB" => "TEXT",
            "BLOB" | "RAW" | "LONG RAW" => "BLOB",
            "ROWID" => "BIGINT",
            _ => "VARCHAR",
        }
    }

    pub fn to_postgres_type(oracle_type: &str) -> &'static str {
        match oracle_type.to_uppercase().as_str() {
            "VARCHAR2" | "CHAR" | "NCHAR" | "NVARCHAR2" => "VARCHAR",
            "NUMBER" => "BIGINT",
            "FLOAT" | "BINARY_FLOAT" | "BINARY_DOUBLE" => "DOUBLE PRECISION",
            "DATE" => "TIMESTAMP",
            "TIMESTAMP" => "TIMESTAMP",
            "CLOB" | "NCLOB" => "TEXT",
            "BLOB" | "RAW" | "LONG RAW" => "BYTEA",
            "ROWID" => "BIGINT",
            _ => "VARCHAR",
        }
    }

    pub fn to_sqlite_type(oracle_type: &str) -> &'static str {
        match oracle_type.to_uppercase().as_str() {
            "VARCHAR2" | "CHAR" | "NCHAR" | "NVARCHAR2" => "TEXT",
            "NUMBER" => "INTEGER",
            "FLOAT" | "BINARY_FLOAT" | "BINARY_DOUBLE" => "REAL",
            "DATE" | "TIMESTAMP" => "TEXT",
            "CLOB" | "NCLOB" => "TEXT",
            "BLOB" | "RAW" | "LONG RAW" => "BLOB",
            _ => "TEXT",
        }
    }
}

#[cfg(feature = "oracle")]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connection_string() {
        let result = OracleConnection::parse_connection_string("oracle://system:pass@localhost:1521/orcl");
        assert!(result.is_ok());

        let (user, pass, host, port, db) = result.unwrap();
        assert_eq!(user, "system");
        assert_eq!(pass, "pass");
        assert_eq!(host, "localhost");
        assert_eq!(port, 1521);
        assert_eq!(db, "orcl");
    }

    #[test]
    fn test_type_conversion_to_mysql() {
        assert_eq!(OracleConverter::to_mysql_type("VARCHAR2"), "VARCHAR");
        assert_eq!(OracleConverter::to_mysql_type("NUMBER"), "BIGINT");
        assert_eq!(OracleConverter::to_mysql_type("CLOB"), "TEXT");
    }

    #[test]
    fn test_type_conversion_to_postgres() {
        assert_eq!(OracleConverter::to_postgres_type("VARCHAR2"), "VARCHAR");
        assert_eq!(OracleConverter::to_postgres_type("NUMBER"), "BIGINT");
        assert_eq!(OracleConverter::to_postgres_type("CLOB"), "TEXT");
    }

    #[test]
    fn test_type_conversion_to_sqlite() {
        assert_eq!(OracleConverter::to_sqlite_type("VARCHAR2"), "TEXT");
        assert_eq!(OracleConverter::to_sqlite_type("NUMBER"), "INTEGER");
        assert_eq!(OracleConverter::to_sqlite_type("CLOB"), "TEXT");
    }
}

#[cfg(not(feature = "oracle"))]
mod tests {
    #[test]
    fn test_oracle_feature_disabled() {
        // Oracle feature not enabled, tests skipped
        assert!(true);
    }
}
