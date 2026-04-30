use crate::models::TableSchema;
use anyhow::Result;
use serde_json;
use async_trait::async_trait;

/// 数据库连接 trait
#[async_trait]
pub trait DatabaseConnection {
    async fn get_table_schema(&self, table_name: &str) -> Result<TableSchema>;
    async fn get_all_tables(&self) -> Result<Vec<String>>;
    async fn execute(&self, sql: &str) -> Result<()>;
    async fn query(&self, sql: &str) -> Result<Vec<serde_json::Value>>;
    async fn begin_transaction(&self) -> Result<()>;
    async fn commit_transaction(&self) -> Result<()>;
    async fn rollback_transaction(&self) -> Result<()>;
}

pub enum DatabaseType {
    MySQL,
    PostgreSQL,
    SQLite,
    Redis,
    Oracle,
    MongoDB,
}

mod mysql;
mod sqlite;
mod postgres;
mod redis;
mod oracle;
mod compatibility;
pub mod connection;

pub use compatibility::{DatabaseVersion, DatabaseCompatibility, MySqlCompatibility, PostgresCompatibility, SqliteCompatibility};
pub use oracle::{OracleConverter, OracleConnection};

pub async fn create_connection(
    db_type: DatabaseType,
    connection_string: &str
) -> Result<Box<dyn DatabaseConnection>> {
    match db_type {
        DatabaseType::MySQL => {
            let conn = mysql::MySqlConnection::new(connection_string).await?;
            Ok(Box::new(conn))
        }
        DatabaseType::PostgreSQL => {
            let conn = postgres::PostgresConnection::new(connection_string).await?;
            Ok(Box::new(conn))
        },
        DatabaseType::SQLite => {
            let conn = sqlite::SqliteConnection::new(connection_string).await?;
            Ok(Box::new(conn))
        }
        DatabaseType::Redis => {
            let conn = redis::RedisConnection::new(connection_string).await?;
            Ok(Box::new(conn))
        }
        #[cfg(feature = "oracle")]
        DatabaseType::Oracle => {
            let conn = oracle::OracleConnection::new(connection_string).await?;
            Ok(Box::new(conn))
        }
        #[cfg(not(feature = "oracle"))]
        DatabaseType::Oracle => {
            anyhow::bail!("Oracle support not enabled. Build with --features oracle")
        }
        DatabaseType::MongoDB => {
            todo!("MongoDB connection not yet implemented")
        }
    }
}
