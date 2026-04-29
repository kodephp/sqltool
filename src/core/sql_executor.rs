use crate::databases::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct SqlExecutor {
    pub connection: Box<dyn DatabaseConnection>,
    db_type: String,
    last_execution_time: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub rows_affected: u64,
    pub returned_rows: Vec<serde_json::Map<String, serde_json::Value>>,
    pub execution_time_ms: u64,
    pub sql: String,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub metadata: ExecutionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub auto_commit: bool,
    pub isolation_level: Option<String>,
    pub catalog: Option<String>,
    pub schema: Option<String>,
}

impl ExecutionResult {
    pub fn success(sql: &str, rows_affected: u64, execution_time_ms: u64) -> Self {
        Self {
            success: true,
            rows_affected,
            returned_rows: Vec::new(),
            execution_time_ms,
            sql: sql.to_string(),
            error: None,
            warnings: Vec::new(),
            metadata: ExecutionMetadata {
                auto_commit: true,
                isolation_level: None,
                catalog: None,
                schema: None,
            },
        }
    }

    pub fn success_with_rows(
        sql: &str,
        rows: Vec<serde_json::Map<String, serde_json::Value>>,
        execution_time_ms: u64,
    ) -> Self {
        Self {
            success: true,
            rows_affected: 0,
            returned_rows: rows,
            execution_time_ms,
            sql: sql.to_string(),
            error: None,
            warnings: Vec::new(),
            metadata: ExecutionMetadata {
                auto_commit: true,
                isolation_level: None,
                catalog: None,
                schema: None,
            },
        }
    }

    pub fn error(sql: &str, err: &str, execution_time_ms: u64) -> Self {
        Self {
            success: false,
            rows_affected: 0,
            returned_rows: Vec::new(),
            execution_time_ms,
            sql: sql.to_string(),
            error: Some(err.to_string()),
            warnings: Vec::new(),
            metadata: ExecutionMetadata {
                auto_commit: true,
                isolation_level: None,
                catalog: None,
                schema: None,
            },
        }
    }

    pub fn format(&self) -> String {
        if self.success {
            if self.returned_rows.is_empty() {
                format!(
                    "OK. {} rows affected ({} ms)\nSQL: {}",
                    self.rows_affected, self.execution_time_ms, self.sql
                )
            } else {
                format!(
                    "OK. {} rows returned ({} ms)\nSQL: {}",
                    self.returned_rows.len(),
                    self.execution_time_ms,
                    self.sql
                )
            }
        } else {
            format!(
                "ERROR: {}\nSQL: {}\nExecution time: {} ms",
                self.error.as_ref().unwrap_or(&"Unknown error".to_string()),
                self.sql,
                self.execution_time_ms
            )
        }
    }

    pub fn summary(&self) -> String {
        if self.success {
            format!(
                "✓ {} rows affected | {} ms",
                self.rows_affected, self.execution_time_ms
            )
        } else {
            format!(
                "✗ Error: {} | {} ms",
                self.error.as_ref().unwrap_or(&"Unknown".to_string()),
                self.execution_time_ms
            )
        }
    }
}

impl SqlExecutor {
    pub fn new(connection: Box<dyn DatabaseConnection>, db_type: &str) -> Self {
        Self {
            connection,
            db_type: db_type.to_string(),
            last_execution_time: None,
        }
    }

    pub fn with_connection(connection: Box<dyn DatabaseConnection>) -> Self {
        Self {
            connection,
            db_type: "unknown".to_string(),
            last_execution_time: None,
        }
    }

    pub async fn execute(&mut self, sql: &str) -> ExecutionResult {
        let start = Instant::now();
        let sql = sql.trim();

        if sql.is_empty() {
            return ExecutionResult::error(sql, "Empty SQL statement", 0);
        }

        let sql_type = self.classify_sql(sql);

        match sql_type {
            SqlType::Select | SqlType::Show | SqlType::Describe | SqlType::Explain => {
                self.execute_query(sql, start).await
            }
            SqlType::Insert | SqlType::Update | SqlType::Delete | SqlType::Replace => {
                self.execute_modification(sql, start).await
            }
            SqlType::Create | SqlType::Alter | SqlType::Drop | SqlType::Truncate => {
                self.execute_ddl(sql, start).await
            }
            SqlType::StartTransaction | SqlType::Begin | SqlType::Commit | SqlType::Rollback => {
                self.execute_transaction_control(sql, start).await
            }
            SqlType::Set | SqlType::Use | SqlType::Other => {
                self.execute_admin(sql, start).await
            }
        }
    }

    async fn execute_query(&mut self, sql: &str, start: Instant) -> ExecutionResult {
        match self.connection.query(sql).await {
            Ok(rows) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                let returned_rows: Vec<serde_json::Map<String, serde_json::Value>> = rows
                    .into_iter()
                    .map(|row| {
                        row.as_object().cloned().unwrap_or_else(|| serde_json::Map::new())
                    })
                    .collect();
                ExecutionResult::success_with_rows(sql, returned_rows, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    async fn execute_modification(&mut self, sql: &str, start: Instant) -> ExecutionResult {
        match self.connection.execute(sql).await {
            Ok(_) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::success(sql, 1, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    async fn execute_ddl(&mut self, sql: &str, start: Instant) -> ExecutionResult {
        match self.connection.execute(sql).await {
            Ok(_) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                let rows_affected = self.get_rows_affected_from_ddl(sql);
                ExecutionResult::success(sql, rows_affected, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    async fn execute_transaction_control(&mut self, sql: &str, start: Instant) -> ExecutionResult {
        match self.connection.query(sql).await {
            Ok(_) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::success(sql, 0, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    async fn execute_admin(&mut self, sql: &str, start: Instant) -> ExecutionResult {
        match self.connection.query(sql).await {
            Ok(rows) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                let returned_rows: Vec<serde_json::Map<String, serde_json::Value>> = rows
                    .into_iter()
                    .map(|row| {
                        row.as_object().cloned().unwrap_or_else(|| serde_json::Map::new())
                    })
                    .collect();
                ExecutionResult::success_with_rows(sql, returned_rows, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    fn classify_sql(&self, sql: &str) -> SqlType {
        let upper = sql.to_uppercase();
        let trimmed = upper.trim();

        if trimmed.starts_with("SELECT") || trimmed.starts_with("WITH") {
            SqlType::Select
        } else if trimmed.starts_with("INSERT") {
            SqlType::Insert
        } else if trimmed.starts_with("UPDATE") {
            SqlType::Update
        } else if trimmed.starts_with("DELETE") {
            SqlType::Delete
        } else if trimmed.starts_with("REPLACE") {
            SqlType::Replace
        } else if trimmed.starts_with("CREATE") {
            SqlType::Create
        } else if trimmed.starts_with("ALTER") {
            SqlType::Alter
        } else if trimmed.starts_with("DROP") {
            SqlType::Drop
        } else if trimmed.starts_with("TRUNCATE") {
            SqlType::Truncate
        } else if trimmed.starts_with("SHOW") || trimmed.starts_with("DESCRIBE") || trimmed.starts_with("DESC") {
            SqlType::Show
        } else if trimmed.starts_with("EXPLAIN") {
            SqlType::Explain
        } else if trimmed.starts_with("START TRANSACTION") || trimmed.starts_with("BEGIN") {
            SqlType::StartTransaction
        } else if trimmed.starts_with("COMMIT") {
            SqlType::Commit
        } else if trimmed.starts_with("ROLLBACK") {
            SqlType::Rollback
        } else if trimmed.starts_with("SET") {
            SqlType::Set
        } else if trimmed.starts_with("USE") {
            SqlType::Use
        } else {
            SqlType::Other
        }
    }

    fn get_rows_affected_from_ddl(&self, sql: &str) -> u64 {
        let upper = sql.to_uppercase();
        if upper.contains("CREATE") || upper.contains("DROP") || upper.contains("ALTER") {
            0
        } else {
            0
        }
    }

    pub async fn insert(&mut self, table: &str, data: &HashMap<String, serde_json::Value>) -> ExecutionResult {
        let fields: Vec<String> = data.keys().map(|k| k.clone()).collect();
        let placeholders: Vec<String> = fields.iter().enumerate().map(|(i, _)| format!("${}", i + 1)).collect();
        let _values: Vec<serde_json::Value> = data.values().map(|v| v.clone()).collect();

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            fields.join(", "),
            placeholders.join(", ")
        );

        let start = Instant::now();
        match self.connection.execute(&sql).await {
            Ok(_) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::success(&sql, 1, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(&sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    pub async fn update(
        &mut self,
        table: &str,
        data: &HashMap<String, serde_json::Value>,
        where_clause: &str,
    ) -> ExecutionResult {
        let set_clauses: Vec<String> = data
            .keys()
            .enumerate()
            .map(|(i, k)| format!("{} = ${}", k, i + 1))
            .collect();

        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            table,
            set_clauses.join(", "),
            where_clause
        );

        let start = Instant::now();
        match self.connection.execute(&sql).await {
            Ok(_) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::success(&sql, 1, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(&sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    pub async fn delete(&mut self, table: &str, where_clause: &str) -> ExecutionResult {
        let sql = format!("DELETE FROM {} WHERE {}", table, where_clause);
        let start = Instant::now();

        match self.connection.execute(&sql).await {
            Ok(_) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::success(&sql, 1, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(&sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    pub async fn select(
        &mut self,
        table: &str,
        fields: &[&str],
        where_clause: Option<&str>,
        order_by: Option<&str>,
        limit: Option<u64>,
    ) -> ExecutionResult {
        let fields_str = if fields.is_empty() {
            "*".to_string()
        } else {
            fields.join(", ")
        };

        let mut sql = format!("SELECT {} FROM {}", fields_str, table);

        if let Some(where_str) = where_clause {
            sql.push_str(&format!(" WHERE {}", where_str));
        }

        if let Some(order_str) = order_by {
            sql.push_str(&format!(" ORDER BY {}", order_str));
        }

        if let Some(limit_val) = limit {
            sql.push_str(&format!(" LIMIT {}", limit_val));
        }

        let start = Instant::now();
        match self.connection.query(&sql).await {
            Ok(rows) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                let returned_rows: Vec<serde_json::Map<String, serde_json::Value>> = rows
                    .into_iter()
                    .map(|row| {
                        row.as_object().cloned().unwrap_or_else(|| serde_json::Map::new())
                    })
                    .collect();
                ExecutionResult::success_with_rows(&sql, returned_rows, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(&sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    pub async fn batch_execute(&mut self, sql_statements: &[&str]) -> Vec<ExecutionResult> {
        let mut results = Vec::new();
        for sql in sql_statements {
            let result = self.execute(sql).await;
            results.push(result);
        }
        results
    }

    pub async fn create_table_if_not_exists(
        &mut self,
        table: &str,
        fields: &[(&str, &str)],
        primary_key: Option<&str>,
    ) -> ExecutionResult {
        let mut field_defs: Vec<String> = fields
            .iter()
            .map(|(name, dtype)| format!("{} {}", name, dtype))
            .collect();

        if let Some(pk) = primary_key {
            field_defs.push(format!("PRIMARY KEY ({})", pk));
        }

        let sql = format!("CREATE TABLE IF NOT EXISTS {} ({})", table, field_defs.join(", "));
        let start = Instant::now();

        match self.connection.execute(&sql).await {
            Ok(_) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::success(&sql, 0, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(&sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    pub async fn drop_table(&mut self, table: &str, if_exists: bool) -> ExecutionResult {
        let sql = if if_exists {
            format!("DROP TABLE IF EXISTS {}", table)
        } else {
            format!("DROP TABLE {}", table)
        };

        let start = Instant::now();
        match self.connection.execute(&sql).await {
            Ok(_) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::success(&sql, 0, execution_time_ms)
            }
            Err(e) => {
                let execution_time_ms = start.elapsed().as_millis() as u64;
                ExecutionResult::error(&sql, &e.to_string(), execution_time_ms)
            }
        }
    }

    pub fn last_execution_time(&self) -> Option<Duration> {
        self.last_execution_time
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlType {
    Select,
    Insert,
    Update,
    Delete,
    Replace,
    Create,
    Alter,
    Drop,
    Truncate,
    Show,
    Describe,
    Explain,
    StartTransaction,
    Begin,
    Commit,
    Rollback,
    Set,
    Use,
    Other,
}

pub struct QueryBuilder {
    table: String,
    fields: Vec<String>,
    conditions: Vec<String>,
    order_by: Vec<String>,
    group_by: Vec<String>,
    having: Vec<String>,
    limit_val: Option<u64>,
    offset_val: Option<u64>,
    joins: Vec<String>,
}

impl QueryBuilder {
    pub fn new(table: &str) -> Self {
        Self {
            table: table.to_string(),
            fields: Vec::new(),
            conditions: Vec::new(),
            order_by: Vec::new(),
            group_by: Vec::new(),
            having: Vec::new(),
            limit_val: None,
            offset_val: None,
            joins: Vec::new(),
        }
    }

    pub fn select(mut self, fields: &[&str]) -> Self {
        self.fields = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn select_all(mut self) -> Self {
        self.fields.push("*".to_string());
        self
    }

    pub fn where_cond(mut self, condition: &str) -> Self {
        self.conditions.push(condition.to_string());
        self
    }

    pub fn order_by(mut self, field: &str, direction: &str) -> Self {
        self.order_by.push(format!("{} {}", field, direction));
        self
    }

    pub fn group_by(mut self, field: &str) -> Self {
        self.group_by.push(field.to_string());
        self
    }

    pub fn having(mut self, condition: &str) -> Self {
        self.having.push(condition.to_string());
        self
    }

    pub fn limit(mut self, limit: u64) -> Self {
        self.limit_val = Some(limit);
        self
    }

    pub fn offset(mut self, offset: u64) -> Self {
        self.offset_val = Some(offset);
        self
    }

    pub fn join(mut self, join_type: &str, table: &str, on: &str) -> Self {
        self.joins.push(format!("{} JOIN {} ON {}", join_type, table, on));
        self
    }

    pub fn build_select(&self) -> String {
        let fields = if self.fields.is_empty() {
            "*".to_string()
        } else {
            self.fields.join(", ")
        };

        let mut sql = format!("SELECT {} FROM {}", fields, self.table);

        if !self.joins.is_empty() {
            sql.push_str(&format!(" {}", self.joins.join(" ")));
        }

        if !self.conditions.is_empty() {
            sql.push_str(&format!(" WHERE {}", self.conditions.join(" AND ")));
        }

        if !self.group_by.is_empty() {
            sql.push_str(&format!(" GROUP BY {}", self.group_by.join(", ")));
        }

        if !self.having.is_empty() {
            sql.push_str(&format!(" HAVING {}", self.having.join(" AND ")));
        }

        if !self.order_by.is_empty() {
            sql.push_str(&format!(" ORDER BY {}", self.order_by.join(", ")));
        }

        if let Some(limit) = self.limit_val {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset_val {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        sql
    }

    pub fn build_insert(&self, data: &HashMap<String, serde_json::Value>) -> String {
        let fields: Vec<String> = data.keys().cloned().collect();
        let values: Vec<String> = data.values().map(|v| self.value_to_sql(v)).collect();
        format!("INSERT INTO {} ({}) VALUES ({})", self.table, fields.join(", "), values.join(", "))
    }

    pub fn build_update(&self, data: &HashMap<String, serde_json::Value>) -> String {
        let set_clauses: Vec<String> = data
            .iter()
            .map(|(k, v)| format!("{} = {}", k, self.value_to_sql(v)))
            .collect();

        let mut sql = format!("UPDATE {} SET {}", self.table, set_clauses.join(", "));

        if !self.conditions.is_empty() {
            sql.push_str(&format!(" WHERE {}", self.conditions.join(" AND ")));
        }

        sql
    }

    pub fn build_delete(&self) -> String {
        let mut sql = format!("DELETE FROM {}", self.table);

        if !self.conditions.is_empty() {
            sql.push_str(&format!(" WHERE {}", self.conditions.join(" AND ")));
        }

        sql
    }

    fn value_to_sql(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "NULL".to_string(),
            serde_json::Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            serde_json::Value::Array(arr) => {
                let values: Vec<String> = arr.iter().map(|v| self.value_to_sql(v)).collect();
                format!("({})", values.join(", "))
            }
            serde_json::Value::Object(obj) => {
                let values: Vec<String> = obj.values().map(|v| self.value_to_sql(v)).collect();
                format!("({})", values.join(", "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder_select() {
        let query = QueryBuilder::new("users")
            .select(&["id", "name", "email"])
            .where_cond("active = TRUE")
            .order_by("created_at", "DESC")
            .limit(10)
            .build_select();

        assert!(query.contains("SELECT id, name, email FROM users"));
        assert!(query.contains("WHERE active = TRUE"));
        assert!(query.contains("ORDER BY created_at DESC"));
        assert!(query.contains("LIMIT 10"));
    }

    #[test]
    fn test_query_builder_insert() {
        let query = QueryBuilder::new("users");
        let mut data = HashMap::new();
        data.insert("name".to_string(), serde_json::Value::String("Alice".to_string()));
        data.insert("email".to_string(), serde_json::Value::String("alice@example.com".to_string()));

        let sql = query.build_insert(&data);
        assert!(sql.contains("INSERT INTO users"));
        assert!(sql.contains("'Alice'"));
        assert!(sql.contains("alice@example.com"));
    }

    #[test]
    fn test_execution_result_format() {
        let result = ExecutionResult::success("INSERT INTO users VALUES (1)", 1, 5);
        let formatted = result.format();
        assert!(formatted.contains("1 rows affected"));
        assert!(formatted.contains("5 ms"));
    }

    #[test]
    fn test_execution_result_error_format() {
        let result = ExecutionResult::error("SELECT * FROM nonexistent", "Table not found", 3);
        let formatted = result.format();
        assert!(formatted.contains("ERROR"));
        assert!(formatted.contains("Table not found"));
    }
}
