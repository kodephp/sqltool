/// 数据库连接配置模块

use serde::{Deserialize, Serialize};

/// 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub charset: Option<String>,
    pub max_connections: Option<u32>,
    pub timeout: Option<u64>,
}

impl ConnectionConfig {
    pub fn new(host: &str, port: u16, username: &str, password: &str, database: &str) -> Self {
        Self {
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            database: database.to_string(),
            charset: None,
            max_connections: None,
            timeout: None,
        }
    }

    pub fn with_charset(mut self, charset: &str) -> Self {
        self.charset = Some(charset.to_string());
        self
    }

    pub fn with_max_connections(mut self, max: u32) -> Self {
        self.max_connections = Some(max);
        self
    }

    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 生成本地 Socket 连接字符串
    pub fn to_sqlite_connection_string(&self) -> String {
        if self.host.starts_with('/') || self.host == ":memory:" {
            if self.host == ":memory:" {
                return "sqlite::memory:".to_string();
            }
            return format!("sqlite:{}", self.host);
        }
        format!("sqlite:{}", self.database)
    }

    /// 生成 MySQL 连接字符串
    pub fn to_mysql_connection_string(&self) -> String {
        let charset = self.charset.as_deref().unwrap_or("utf8mb4");
        format!(
            "mysql://{}:{}@{}:{}/{}?charset={}",
            self.username,
            self.password,
            self.host,
            self.port,
            self.database,
            charset
        )
    }

    /// 生成 PostgreSQL 连接字符串
    pub fn to_postgres_connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password,
            self.host,
            self.port,
            self.database
        )
    }

    /// 生成 Redis 连接字符串
    pub fn to_redis_connection_string(&self) -> String {
        if self.username.is_empty() {
            format!(
                "redis://:{}@{}:{}",
                self.password,
                self.host,
                self.port
            )
        } else {
            format!(
                "redis://{}:{}@{}:{}",
                self.username,
                self.password,
                self.host,
                self.port
            )
        }
    }
}

/// MySQL 特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySqlConfig {
    pub connection_config: ConnectionConfig,
    pub auto_reconnect: bool,
    pub compress: bool,
    pub found_rows: bool,
}

impl MySqlConfig {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            connection_config: config,
            auto_reconnect: true,
            compress: false,
            found_rows: true,
        }
    }

    pub fn to_connection_string(&self) -> String {
        let base = self.connection_config.to_mysql_connection_string();
        let mut params = vec![];
        
        if self.auto_reconnect {
            params.push("auto_reconnect=true");
        }
        if self.compress {
            params.push("compress=true");
        }
        if self.found_rows {
            params.push("found_rows=true");
        }
        
        if params.is_empty() {
            base
        } else {
            format!("{}&{}", base, params.join("&"))
        }
    }
}

/// PostgreSQL 特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub connection_config: ConnectionConfig,
    pub ssl_mode: SslMode,
    pub application_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl PostgresConfig {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            connection_config: config,
            ssl_mode: SslMode::Prefer,
            application_name: None,
        }
    }

    pub fn with_ssl_mode(mut self, mode: SslMode) -> Self {
        self.ssl_mode = mode;
        self
    }

    pub fn with_application_name(mut self, name: &str) -> Self {
        self.application_name = Some(name.to_string());
        self
    }

    pub fn to_connection_string(&self) -> String {
        let base = self.connection_config.to_postgres_connection_string();
        let mut params: Vec<String> = vec![];
        
        let ssl_param = match self.ssl_mode {
            SslMode::Disable => "sslmode=disable",
            SslMode::Allow => "sslmode=allow",
            SslMode::Prefer => "sslmode=prefer",
            SslMode::Require => "sslmode=require",
            SslMode::VerifyCa => "sslmode=verify-ca",
            SslMode::VerifyFull => "sslmode=verify-full",
        };
        params.push(ssl_param.to_string());
        
        if let Some(ref name) = self.application_name {
            params.push(format!("application_name={}", name));
        }
        
        format!("{}?{}", base, params.join("&"))
    }
}

/// SQLite 特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub connection_config: ConnectionConfig,
    pub read_only: bool,
    pub journal_mode: JournalMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    Wal,
    Off,
}

impl SqliteConfig {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            connection_config: config,
            read_only: false,
            journal_mode: JournalMode::Wal,
        }
    }

    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn with_journal_mode(mut self, mode: JournalMode) -> Self {
        self.journal_mode = mode;
        self
    }

    pub fn to_connection_string(&self) -> String {
        let base = self.connection_config.to_sqlite_connection_string();
        let mut params = vec![];
        
        if self.read_only {
            params.push("mode=ro");
        }
        
        let journal_param = match self.journal_mode {
            JournalMode::Delete => "journal_mode=delete",
            JournalMode::Truncate => "journal_mode=truncate",
            JournalMode::Persist => "journal_mode=persist",
            JournalMode::Memory => "journal_mode=memory",
            JournalMode::Wal => "journal_mode=wal",
            JournalMode::Off => "journal_mode=off",
        };
        params.push(journal_param);
        
        if params.is_empty() {
            base
        } else {
            format!("{}?{}", base, params.join("&"))
        }
    }
}

/// Redis 特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub connection_config: ConnectionConfig,
    pub db: Option<u8>,
}

impl RedisConfig {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            connection_config: config,
            db: None,
        }
    }

    pub fn with_db(mut self, db: u8) -> Self {
        self.db = Some(db);
        self
    }

    pub fn to_connection_string(&self) -> String {
        let base = self.connection_config.to_redis_connection_string();
        if let Some(db) = self.db {
            format!("{}/{}", base, db)
        } else {
            base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mysql_connection_string() {
        let config = ConnectionConfig::new("localhost", 3306, "root", "password", "testdb");
        let mysql_config = MySqlConfig::new(config);
        let conn_str = mysql_config.to_connection_string();
        
        assert!(conn_str.contains("mysql://"));
        assert!(conn_str.contains("root:password"));
        assert!(conn_str.contains("localhost:3306"));
        assert!(conn_str.contains("testdb"));
        
        println!("MySQL Connection: {}", conn_str);
    }

    #[test]
    fn test_postgres_connection_string() {
        let config = ConnectionConfig::new("localhost", 5432, "postgres", "password", "testdb");
        let pg_config = PostgresConfig::new(config);
        let conn_str = pg_config.to_connection_string();
        
        assert!(conn_str.contains("postgres://"));
        assert!(conn_str.contains("postgres:password"));
        assert!(conn_str.contains("localhost:5432"));
        assert!(conn_str.contains("testdb"));
        assert!(conn_str.contains("sslmode=prefer"));
        
        println!("PostgreSQL Connection: {}", conn_str);
    }

    #[test]
    fn test_sqlite_connection_string() {
        let config = ConnectionConfig::new("/var/data/app.db", 0, "", "", "");
        let sqlite_config = SqliteConfig::new(config);
        let conn_str = sqlite_config.to_connection_string();
        
        assert_eq!(conn_str, "sqlite:/var/data/app.db?journal_mode=wal");
        
        println!("SQLite Connection: {}", conn_str);
    }

    #[test]
    fn test_sqlite_memory_connection() {
        let config = ConnectionConfig::new(":memory:", 0, "", "", "");
        let sqlite_config = SqliteConfig::new(config);
        let conn_str = sqlite_config.to_connection_string();
        
        assert_eq!(conn_str, "sqlite::memory:?journal_mode=wal");
        
        println!("SQLite Memory Connection: {}", conn_str);
    }

    #[test]
    fn test_redis_connection_string() {
        let config = ConnectionConfig::new("localhost", 6379, "default", "password", "");
        let redis_config = RedisConfig::new(config);
        let conn_str = redis_config.to_connection_string();
        
        assert_eq!(conn_str, "redis://default:password@localhost:6379");
        
        println!("Redis Connection: {}", conn_str);
    }
}
