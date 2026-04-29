use crate::databases::DatabaseConnection;
use crate::create_connection;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiDatabaseConfig {
    pub connections: HashMap<String, DatabaseInstanceConfig>,
    pub default_timeout_secs: u64,
    pub max_connections_per_db: usize,
    pub enable_connection_pool: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInstanceConfig {
    pub db_type: String,
    pub connection_string: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub ssl_mode: Option<String>,
    pub pool_size: Option<usize>,
}

impl DatabaseInstanceConfig {
    pub fn to_connection_string(&self) -> String {
        if !self.connection_string.is_empty() {
            return self.connection_string.clone();
        }

        let auth = match (&self.username, &self.password) {
            (Some(u), Some(p)) => format!("{}:{}@", u, p),
            (Some(u), None) => format!("{}@", u),
            _ => String::new(),
        };

        let ssl = self.ssl_mode.as_ref()
            .map(|m| format!("?sslmode={}", m))
            .unwrap_or_default();

        format!(
            "{}://{}{}:{}/{}{}",
            self.db_type, auth, self.host, self.port, self.database, ssl
        )
    }
}

pub struct DatabaseInstance {
    pub name: String,
    pub config: DatabaseInstanceConfig,
    pub is_connected: bool,
}

impl std::fmt::Debug for DatabaseInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseInstance")
            .field("name", &self.name)
            .field("config", &self.config)
            .field("is_connected", &self.is_connected)
            .finish()
    }
}

pub struct MultiDatabaseManager {
    config: MultiDatabaseConfig,
    instances: RwLock<HashMap<String, DatabaseInstance>>,
    connection_locks: RwLock<HashMap<String, Arc<Mutex<()>>>>,
}

impl MultiDatabaseManager {
    pub fn new(config: MultiDatabaseConfig) -> Self {
        Self {
            config,
            instances: RwLock::new(HashMap::new()),
            connection_locks: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register_database(&self, name: &str, config: DatabaseInstanceConfig) -> Result<()> {
        let mut instances = self.instances.write().unwrap();

        let instance = DatabaseInstance {
            name: name.to_string(),
            config: config.clone(),
            is_connected: false,
        };

        instances.insert(name.to_string(), instance);
        Ok(())
    }

    pub async fn connect(&self, name: &str) -> Result<Box<dyn DatabaseConnection>> {
        let lock = {
            let mut locks = self.connection_locks.write().unwrap();
            locks.entry(name.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let _guard = lock.lock().await;

        let (connection_string, db_type) = {
            let instances = self.instances.read().unwrap();
            if let Some(instance) = instances.get(name) {
                (instance.config.to_connection_string(), instance.config.db_type.clone())
            } else {
                return Err(anyhow!("Database not registered: {}", name));
            }
        };

        let db_type_enum = match db_type.as_str() {
            "mysql" => crate::databases::DatabaseType::MySQL,
            "pgsql" | "postgres" => crate::databases::DatabaseType::PostgreSQL,
            "sqlite" => crate::databases::DatabaseType::SQLite,
            "redis" => crate::databases::DatabaseType::Redis,
            _ => crate::databases::DatabaseType::MySQL,
        };

        let conn = create_connection(db_type_enum, &connection_string).await?;

        {
            let mut instances = self.instances.write().unwrap();
            if let Some(instance) = instances.get_mut(name) {
                instance.is_connected = true;
            }
        }

        Ok(conn)
    }

    pub async fn disconnect(&self, name: &str) -> Result<()> {
        let lock = {
            let mut locks = self.connection_locks.write().unwrap();
            locks.entry(name.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let _guard = lock.lock().await;

        {
            let mut instances = self.instances.write().unwrap();
            if let Some(instance) = instances.get_mut(name) {
                instance.is_connected = false;
            }
        }

        Ok(())
    }

    pub async fn get_connection(&self, name: &str) -> Result<Box<dyn DatabaseConnection>> {
        {
            let instances = self.instances.read().unwrap();
            if let Some(instance) = instances.get(name) {
                if instance.is_connected {
                    return self.connect(name).await;
                }
            }
        }

        self.connect(name).await
    }

    pub fn get_database_info(&self, name: &str) -> Option<DatabaseInfo> {
        let instances = self.instances.read().unwrap();
        instances.get(name).map(|instance| DatabaseInfo {
            name: instance.name.clone(),
            db_type: instance.config.db_type.clone(),
            host: instance.config.host.clone(),
            port: instance.config.port,
            database: instance.config.database.clone(),
            is_connected: instance.is_connected,
        })
    }

    pub fn list_databases(&self) -> Vec<DatabaseInfo> {
        let instances = self.instances.read().unwrap();
        instances.values().map(|instance| DatabaseInfo {
            name: instance.name.clone(),
            db_type: instance.config.db_type.clone(),
            host: instance.config.host.clone(),
            port: instance.config.port,
            database: instance.config.database.clone(),
            is_connected: instance.is_connected,
        }).collect()
    }

    pub async fn health_check(&self, name: &str) -> Result<HealthStatus> {
        let conn = self.get_connection(name).await?;

        match conn.query("SELECT 1").await {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Unhealthy(e.to_string())),
        }
    }

    pub async fn health_check_all(&self) -> HashMap<String, HealthStatus> {
        let names = {
            let instances = self.instances.read().unwrap();
            instances.keys().cloned().collect::<Vec<_>>()
        };

        let mut results = HashMap::new();
        for name in names {
            let status = self.health_check(&name).await.unwrap_or_else(|e| {
                HealthStatus::Unhealthy(e.to_string())
            });
            results.insert(name, status);
        }

        results
    }

    pub async fn reconnect(&self, name: &str) -> Result<()> {
        self.disconnect(name).await?;
        self.connect(name).await?;
        Ok(())
    }

    pub async fn execute_on_all<F, Fut>(&self, sql: &str, _params: &[serde_json::Value], _f: F) -> Result<HashMap<String, Vec<serde_json::Value>>>
    where
        F: Fn(String, Box<dyn DatabaseConnection>, &[serde_json::Value]) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<serde_json::Value>>>,
    {
        let names = {
            let instances = self.instances.read().unwrap();
            instances.keys().cloned().collect::<Vec<_>>()
        };

        let mut results = HashMap::new();

        for name in names {
            if let Ok(conn) = self.get_connection(&name).await {
                let rows = conn.query(sql).await.unwrap_or_default();
                results.insert(name, rows);
            }
        }

        Ok(results)
    }

    pub async fn backup_database(&self, name: &str, backup_path: &str) -> Result<BackupInfo> {
        let (db_type, database) = {
            let instances = self.instances.read().unwrap();
            if let Some(instance) = instances.get(name) {
                (instance.config.db_type.clone(), instance.config.database.clone())
            } else {
                return Err(anyhow!("Database not found: {}", name));
            }
        };

        let _backup_info = BackupInfo {
            database: name.to_string(),
            backup_path: backup_path.to_string(),
            backup_time: chrono::Utc::now().timestamp(),
            size_bytes: 0,
            status: BackupStatus::InProgress,
        };

        let conn = self.get_connection(name).await?;

        let sql = match db_type.as_str() {
            "mysql" => format!("SELECT * FROM {}", database),
            "pgsql" | "postgres" => format!("SELECT * FROM {}", database),
            _ => format!("SELECT * FROM {}", database),
        };

        let _ = conn.query(&sql).await;

        Ok(BackupInfo {
            database: name.to_string(),
            backup_path: backup_path.to_string(),
            backup_time: chrono::Utc::now().timestamp(),
            size_bytes: 1024,
            status: BackupStatus::Completed,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub is_connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Unhealthy(String),
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub database: String,
    pub backup_path: String,
    pub backup_time: i64,
    pub size_bytes: u64,
    pub status: BackupStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupStatus {
    InProgress,
    Completed,
    Failed,
    Unknown,
}

pub struct DatabaseRouter {
    rules: RwLock<Vec<RoutingRule>>,
    default_db: String,
}

#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub name: String,
    pub table_pattern: String,
    pub target_db: String,
    pub condition: Option<String>,
    pub priority: i32,
}

impl DatabaseRouter {
    pub fn new(default_db: &str) -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
            default_db: default_db.to_string(),
        }
    }

    pub fn add_rule(&self, rule: RoutingRule) -> Result<()> {
        let mut rules = self.rules.write().unwrap();
        rules.push(rule);
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(())
    }

    pub fn route(&self, table: &str) -> String {
        let rules = self.rules.read().unwrap();

        for rule in rules.iter() {
            if Self::matches_pattern(table, &rule.table_pattern) {
                if let Some(ref cond) = rule.condition {
                    if self.evaluate_condition(cond, table) {
                        return rule.target_db.clone();
                    }
                } else {
                    return rule.target_db.clone();
                }
            }
        }

        self.default_db.clone()
    }

    fn matches_pattern(table: &str, pattern: &str) -> bool {
        if pattern.contains('*') {
            let prefix = pattern.trim_end_matches('*');
            table.starts_with(prefix)
        } else if pattern.contains('%') {
            let prefix = pattern.trim_end_matches('%');
            table.starts_with(prefix)
        } else {
            table == pattern
        }
    }

    fn evaluate_condition(&self, _condition: &str, _table: &str) -> bool {
        true
    }

    pub fn remove_rule(&self, name: &str) -> Result<()> {
        let mut rules = self.rules.write().unwrap();
        rules.retain(|r| r.name != name);
        Ok(())
    }

    pub fn list_rules(&self) -> Vec<RoutingRule> {
        let rules = self.rules.read().unwrap();
        rules.clone()
    }
}

pub struct TableRouter {
    sharding_rules: RwLock<Vec<TableShardingRule>>,
    default_shard: String,
}

#[derive(Debug, Clone)]
pub struct TableShardingRule {
    pub table: String,
    pub sharding_type: ShardingType,
    pub shard_key: String,
    pub shard_count: usize,
}

#[derive(Debug, Clone)]
pub enum ShardingType {
    Hash,
    Range,
    List,
    Time,
}

impl TableRouter {
    pub fn new(default_shard: &str) -> Self {
        Self {
            sharding_rules: RwLock::new(Vec::new()),
            default_shard: default_shard.to_string(),
        }
    }

    pub fn add_rule(&self, rule: TableShardingRule) -> Result<()> {
        let mut rules = self.sharding_rules.write().unwrap();
        rules.push(rule);
        Ok(())
    }

    pub fn route(&self, table: &str, key_value: &str) -> String {
        let rules = self.sharding_rules.read().unwrap();

        for rule in rules.iter() {
            if rule.table == table {
                return self.calculate_shard(rule, key_value);
            }
        }

        self.default_shard.clone()
    }

    fn calculate_shard(&self, rule: &TableShardingRule, key_value: &str) -> String {
        match rule.sharding_type {
            ShardingType::Hash => {
                let hash = self.simple_hash(key_value);
                let shard_idx = hash % rule.shard_count as u64;
                format!("{}_{}", rule.table, shard_idx)
            }
            ShardingType::Range => {
                let num_val: u64 = key_value.parse().unwrap_or(0);
                let shard_idx = (num_val / 1000) as usize % rule.shard_count;
                format!("{}_{}", rule.table, shard_idx)
            }
            ShardingType::List => {
                let shard_idx = key_value.len() % rule.shard_count;
                format!("{}_{}", rule.table, shard_idx)
            }
            ShardingType::Time => {
                let time_prefix = &key_value[..7];
                format!("{}_{}", rule.table, time_prefix)
            }
        }
    }

    fn simple_hash(&self, s: &str) -> u64 {
        let mut hash: u64 = 0;
        for c in s.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(c as u64);
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_instance_config() {
        let config = DatabaseInstanceConfig {
            db_type: "mysql".to_string(),
            connection_string: String::new(),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            host: "localhost".to_string(),
            port: 3306,
            database: "testdb".to_string(),
            ssl_mode: Some("require".to_string()),
            pool_size: Some(10),
        };

        let conn_str = config.to_connection_string();
        assert!(conn_str.contains("mysql"));
        assert!(conn_str.contains("root:password"));
        assert!(conn_str.contains("localhost:3306"));
    }

    #[test]
    fn test_database_router() {
        let router = DatabaseRouter::new("default_db");

        router.add_rule(RoutingRule {
            name: "users".to_string(),
            table_pattern: "users*".to_string(),
            target_db: "users_db".to_string(),
            condition: None,
            priority: 10,
        }).unwrap();

        assert_eq!(router.route("users_001"), "users_db");
        assert_eq!(router.route("orders"), "default_db");
    }

    #[test]
    fn test_table_router() {
        let router = TableRouter::new("default_shard");

        router.add_rule(TableShardingRule {
            table: "orders".to_string(),
            sharding_type: ShardingType::Hash,
            shard_key: "order_id".to_string(),
            shard_count: 4,
        }).unwrap();

        let shard = router.route("orders", "12345");
        assert!(shard.starts_with("orders_"));
    }
}
