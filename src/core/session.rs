use crate::databases::DatabaseConnection;
use crate::core::sql_executor::{SqlExecutor, ExecutionResult};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub database_name: String,
    pub db_type: String,
    pub user: Option<String>,
    pub created_at: u64,
    pub last_activity: u64,
    pub is_active: bool,
    pub session_variables: HashMap<String, String>,
    pub connection_info: ConnectionInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub charset: String,
    pub autocommit: bool,
}

impl Session {
    pub fn new(id: &str, db_type: &str, database: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id: id.to_string(),
            database_name: database.to_string(),
            db_type: db_type.to_string(),
            user: None,
            created_at: now,
            last_activity: now,
            is_active: true,
            session_variables: HashMap::new(),
            connection_info: ConnectionInfo {
                host: String::new(),
                port: 0,
                database: database.to_string(),
                charset: "utf8mb4".to_string(),
                autocommit: true,
            },
        }
    }

    pub fn touch(&mut self) {
        self.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn set_variable(&mut self, key: &str, value: &str) {
        self.session_variables.insert(key.to_string(), value.to_string());
    }

    pub fn get_variable(&self, key: &str) -> Option<String> {
        self.session_variables.get(key).cloned()
    }

    pub fn is_expired(&self, timeout_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_activity > timeout_secs
    }
}

pub struct SessionManager {
    sessions: RwLock<HashMap<String, Arc<RwLock<Session>>>>,
    max_sessions: usize,
    session_timeout_secs: u64,
}

impl SessionManager {
    pub fn new(max_sessions: usize, session_timeout_secs: u64) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            max_sessions,
            session_timeout_secs,
        }
    }

    pub fn create_session(&self, id: &str, db_type: &str, database: &str) -> Result<Session> {
        {
            let sessions = self.sessions.write().unwrap();
            if sessions.len() >= self.max_sessions {
                drop(sessions);
                self.cleanup_expired_sessions()?;
            }
        }

        let mut sessions = self.sessions.write().unwrap();
        let session = Session::new(id, db_type, database);
        let session_arc = Arc::new(RwLock::new(session.clone()));
        sessions.insert(id.to_string(), session_arc);

        Ok(session)
    }

    pub fn get_session(&self, id: &str) -> Result<Session> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))
            .map(|s| s.read().unwrap().clone())
    }

    pub fn update_session(&self, session: &Session) -> Result<()> {
        let sessions = self.sessions.write().unwrap();
        if let Some(s) = sessions.get(&session.id) {
            let mut s = s.write().unwrap();
            *s = session.clone();
            Ok(())
        } else {
            Err(anyhow!("Session not found: {}", session.id))
        }
    }

    pub fn close_session(&self, id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();
        sessions
            .remove(id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;
        Ok(())
    }

    pub fn list_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .map(|s| s.read().unwrap().clone())
            .collect()
    }

    pub fn cleanup_expired_sessions(&self) -> Result<usize> {
        let mut sessions = self.sessions.write().unwrap();
        self.cleanup_expired_sessions_internal(&mut sessions)
    }

    fn cleanup_expired_sessions_internal(
        &self,
        sessions: &mut HashMap<String, Arc<RwLock<Session>>>,
    ) -> Result<usize> {
        let before = sessions.len();
        sessions.retain(|_, session| {
            let s = session.read().unwrap();
            !s.is_expired(self.session_timeout_secs)
        });
        let after = sessions.len();
        Ok(before - after)
    }

    pub fn session_count(&self) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions.len()
    }

    pub fn get_active_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .filter_map(|s| {
                let s = s.read().unwrap();
                if s.is_active && !s.is_expired(self.session_timeout_secs) {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

pub struct DatabaseSession {
    pub session: Session,
    pub executor: SqlExecutor,
}

impl DatabaseSession {
    pub async fn connect(
        id: &str,
        connection: Box<dyn DatabaseConnection>,
        _db_type: &str,
        database: &str,
    ) -> Result<Self> {
        let session = Session::new(id, "unknown", database);
        let executor = SqlExecutor::new(connection, "unknown");

        Ok(Self {
            session,
            executor,
        })
    }

    pub async fn execute(&mut self, sql: &str) -> ExecutionResult {
        self.session.touch();
        self.executor.execute(sql).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewOperation {
    pub name: String,
    pub definition: String,
    pub or_replace: bool,
}

pub struct ViewManager {
    executor: SqlExecutor,
}

impl ViewManager {
    pub fn new(executor: SqlExecutor) -> Self {
        Self { executor }
    }

    pub async fn create_view(&mut self, operation: ViewOperation) -> ExecutionResult {
        let or_replace = if operation.or_replace { "OR REPLACE" } else { "" };
        let sql = format!(
            "CREATE {} VIEW {} AS {}",
            or_replace,
            operation.name,
            operation.definition
        );
        self.executor.execute(&sql).await
    }

    pub async fn drop_view(&mut self, name: &str, if_exists: bool) -> ExecutionResult {
        let if_exists = if if_exists { "IF EXISTS" } else { "" };
        let sql = format!("DROP VIEW {} {}", if_exists, name);
        self.executor.execute(&sql).await
    }

    pub async fn rename_view(&mut self, old_name: &str, new_name: &str) -> ExecutionResult {
        let sql = format!("RENAME TABLE {} TO {}", old_name, new_name);
        self.executor.execute(&sql).await
    }
}

pub struct FunctionManager {
    executor: SqlExecutor,
}

impl FunctionManager {
    pub fn new(executor: SqlExecutor) -> Self {
        Self { executor }
    }

    pub async fn create_function(
        &mut self,
        name: &str,
        args: &[(&str, &str)],
        return_type: &str,
        body: &str,
        language: &str,
    ) -> ExecutionResult {
        let args_str = args
            .iter()
            .map(|(n, t)| format!("{} {}", n, t))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "CREATE FUNCTION {} ({}) RETURNS {} LANGUAGE {} AS $$\n{}\n$$",
            name, args_str, return_type, language, body
        );

        self.executor.execute(&sql).await
    }

    pub async fn drop_function(&mut self, name: &str, if_exists: bool) -> ExecutionResult {
        let if_exists = if if_exists { "IF EXISTS" } else { "" };
        let sql = format!("DROP FUNCTION {} {}", if_exists, name);
        self.executor.execute(&sql).await
    }
}

pub struct IndexManager {
    executor: SqlExecutor,
}

impl IndexManager {
    pub fn new(executor: SqlExecutor) -> Self {
        Self { executor }
    }

    pub async fn create_index(
        &mut self,
        table: &str,
        name: &str,
        fields: &[&str],
        unique: bool,
    ) -> ExecutionResult {
        let unique_keyword = if unique { "UNIQUE" } else { "" };
        let fields_str = fields.join(", ");
        let sql = format!(
            "CREATE {} INDEX {} ON {} ({})",
            unique_keyword, name, table, fields_str
        );
        self.executor.execute(&sql).await
    }

    pub async fn drop_index(&mut self, name: &str, table: Option<&str>) -> ExecutionResult {
        let sql = if let Some(t) = table {
            format!("DROP INDEX {} ON {}", name, t)
        } else {
            format!("DROP INDEX {}", name)
        };
        self.executor.execute(&sql).await
    }

    pub async fn analyze_table(&mut self, table: &str) -> ExecutionResult {
        let sql = format!("ANALYZE TABLE {}", table);
        self.executor.execute(&sql).await
    }

    pub async fn check_table(&mut self, table: &str) -> ExecutionResult {
        let sql = format!("CHECK TABLE {}", table);
        self.executor.execute(&sql).await
    }

    pub async fn repair_table(&mut self, table: &str) -> ExecutionResult {
        let sql = format!("REPAIR TABLE {}", table);
        self.executor.execute(&sql).await
    }

    pub async fn optimize_table(&mut self, table: &str) -> ExecutionResult {
        let sql = format!("OPTIMIZE TABLE {}", table);
        self.executor.execute(&sql).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new("sess_001", "mysql", "testdb");
        assert_eq!(session.id, "sess_001");
        assert_eq!(session.database_name, "testdb");
        assert!(session.is_active);
    }

    #[test]
    fn test_session_variables() {
        let mut session = Session::new("sess_001", "mysql", "testdb");
        session.set_variable("sql_mode", "STRICT_TRANS_TABLES");
        assert_eq!(session.get_variable("sql_mode"), Some("STRICT_TRANS_TABLES".to_string()));
    }

    #[test]
    fn test_session_manager() {
        let manager = SessionManager::new(10, 3600);
        let session = manager.create_session("sess_001", "mysql", "testdb").unwrap();
        assert_eq!(session.id, "sess_001");

        let retrieved = manager.get_session("sess_001").unwrap();
        assert_eq!(retrieved.id, "sess_001");

        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 1);

        manager.close_session("sess_001").unwrap();
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_query_builder_integration() {
        use crate::core::sql_executor::QueryBuilder;
        use std::collections::HashMap;

        let query = QueryBuilder::new("users")
            .select(&["id", "name"])
            .where_cond("active = TRUE")
            .build_select();

        assert!(query.contains("SELECT id, name FROM users"));
    }
}
