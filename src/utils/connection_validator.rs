/// 连接字符串验证模块

use crate::utils::error::SqlToolError;

#[derive(Debug, Clone)]
pub struct ConnectionString {
    pub db_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub database: String,
    pub options: Vec<(String, String)>,
}

impl ConnectionString {
    pub fn parse(conn_str: &str) -> Result<Self, SqlToolError> {
        let conn_str = conn_str.trim();

        if conn_str.is_empty() {
            return Err(SqlToolError::validation_error("Connection string cannot be empty"));
        }

        let parts: Vec<&str> = conn_str.splitn(2, "://").collect();
        if parts.len() != 2 {
            return Err(SqlToolError::validation_error(&format!(
                "Invalid connection string format: missing '://'. Example: mysql://user:pass@host:port/db"
            )));
        }

        let db_type = parts[0].to_lowercase();
        let remaining = parts[1];

        let (auth, host_db) = if let Some(pos) = remaining.find('@') {
            (&remaining[..pos], &remaining[pos + 1..])
        } else {
            ("", remaining)
        };

        let (username, password) = if !auth.is_empty() {
            let auth_parts: Vec<&str> = auth.splitn(2, ':').collect();
            let user = auth_parts.first().map(|s| s.to_string()).unwrap_or_default();
            let pass = auth_parts.get(1).map(|s| s.to_string());
            (Some(user), pass)
        } else {
            (None, None)
        };

        let host_db_parts: Vec<&str> = host_db.splitn(2, '/').collect();
        let host_port = host_db_parts.first().unwrap_or(&"");
        let database = host_db_parts.get(1).unwrap_or(&"").to_string();

        let host_port_parts: Vec<&str> = host_port.splitn(2, ':').collect();
        let host = host_port_parts.first().unwrap_or(&"localhost").to_string();
        let port: u16 = host_port_parts.get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| default_port(&db_type));

        let options = if database.contains('?') {
            let db_and_options: Vec<&str> = database.splitn(2, '?').collect();
            let opts_str = db_and_options[1];
            opts_str.split('&')
                .filter_map(|pair| {
                    let kv: Vec<&str> = pair.splitn(2, '=').collect();
                    if kv.len() == 2 {
                        Some((kv[0].to_string(), kv[1].to_string()))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let database = database.split('?').next().unwrap_or(&database).to_string();

        Ok(Self {
            db_type,
            host,
            port,
            username,
            password,
            database,
            options,
        })
    }

    pub fn validate(&self) -> Result<(), SqlToolError> {
        match self.db_type.as_str() {
            "mysql" | "postgresql" | "sqlite" | "redis" | "mongodb" | "oracle" => {}
            other => {
                return Err(SqlToolError::validation_error(&format!(
                    "Unsupported database type: {}. Supported: mysql, postgresql, sqlite, redis, mongodb, oracle",
                    other
                )));
            }
        }

        if self.host.is_empty() && self.db_type != "sqlite" {
            return Err(SqlToolError::validation_error("Host cannot be empty"));
        }

        if self.database.is_empty() && self.db_type != "redis" {
            return Err(SqlToolError::validation_error("Database name cannot be empty"));
        }

        if let Some(ref password) = self.password {
            if password.contains('@') || password.contains('/') || password.contains(':') {
                log::warn!("Password contains special characters that might cause parsing issues");
            }
        }

        Ok(())
    }

    pub fn mask_password(&self) -> String {
        let masked = self.password
            .as_ref()
            .map(|p| if p.len() > 2 {
                format!("{}***{}", &p[0..1], &p[p.len()-1..])
            } else {
                "***".to_string()
            })
            .unwrap_or_else(|| "none".to_string());

        format!(
            "{}://{}:{}@{}:{}/{}",
            self.db_type,
            self.username.as_deref().unwrap_or(""),
            masked,
            self.host,
            self.port,
            self.database
        )
    }

    pub fn is_localhost(&self) -> bool {
        self.host == "localhost" || self.host == "127.0.0.1" || self.host == "::1"
    }
}

fn default_port(db_type: &str) -> u16 {
    match db_type {
        "mysql" => 3306,
        "postgresql" | "postgres" => 5432,
        "sqlite" => 0,
        "redis" => 6379,
        "mongodb" => 27017,
        "oracle" => 1521,
        _ => 0,
    }
}

pub fn validate_connection_string(conn_str: &str) -> Result<ConnectionString, SqlToolError> {
    let parsed = ConnectionString::parse(conn_str)?;
    parsed.validate()?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mysql() {
        let conn = ConnectionString::parse("mysql://root:password@localhost:3306/mydb").unwrap();
        assert_eq!(conn.db_type, "mysql");
        assert_eq!(conn.host, "localhost");
        assert_eq!(conn.port, 3306);
        assert_eq!(conn.username, Some("root".to_string()));
        assert_eq!(conn.password, Some("password".to_string()));
        assert_eq!(conn.database, "mydb");
    }

    #[test]
    fn test_parse_postgresql() {
        let conn = ConnectionString::parse("postgresql://postgres:pass@192.168.1.100:5432/testdb").unwrap();
        assert_eq!(conn.db_type, "postgresql");
        assert_eq!(conn.host, "192.168.1.100");
        assert_eq!(conn.port, 5432);
    }

    #[test]
    fn test_parse_sqlite() {
        let conn = ConnectionString::parse("sqlite:///tmp/test.db").unwrap();
        assert_eq!(conn.db_type, "sqlite");
        assert_eq!(conn.host, "");
        assert_eq!(conn.database, "tmp/test.db");
    }

    #[test]
    fn test_parse_with_options() {
        let conn = ConnectionString::parse("mysql://root:pass@localhost:3306/mydb?charset=utf8mb4&timeout=10").unwrap();
        assert_eq!(conn.options.len(), 2);
        assert_eq!(conn.options[0], ("charset".to_string(), "utf8mb4".to_string()));
    }

    #[test]
    fn test_validate_valid() {
        let conn = ConnectionString::parse("mysql://root:pass@localhost:3306/mydb").unwrap();
        assert!(conn.validate().is_ok());
    }

    #[test]
    fn test_validate_unsupported_db() {
        let conn = ConnectionString::parse("unsupported://localhost/db").unwrap();
        assert!(conn.validate().is_err());
    }

    #[test]
    fn test_mask_password() {
        let conn = ConnectionString::parse("mysql://root:secretpass@localhost/mydb").unwrap();
        let masked = conn.mask_password();
        assert!(!masked.contains("secretpass"));
        assert!(masked.contains("***"));
    }

    #[test]
    fn test_is_localhost() {
        let conn1 = ConnectionString::parse("mysql://root@localhost/mydb").unwrap();
        assert!(conn1.is_localhost());

        let conn2 = ConnectionString::parse("mysql://root@127.0.0.1/mydb").unwrap();
        assert!(conn2.is_localhost());

        let conn3 = ConnectionString::parse("mysql://root@192.168.1.1/mydb").unwrap();
        assert!(!conn3.is_localhost());
    }
}
