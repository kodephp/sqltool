use crate::databases::DatabaseConnection;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryConfig {
    pub threshold_ms: u64,
    pub enable_logging: bool,
    pub max_log_entries: usize,
    pub include_explain: bool,
    pub auto_optimize: bool,
    pub alert_threshold_ratio: f64,
}

impl Default for SlowQueryConfig {
    fn default() -> Self {
        Self {
            threshold_ms: 1000,
            enable_logging: true,
            max_log_entries: 1000,
            include_explain: true,
            auto_optimize: false,
            alert_threshold_ratio: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryLog {
    pub id: String,
    pub sql: String,
    pub execution_time_ms: u64,
    pub rows_affected: u64,
    pub timestamp: i64,
    pub database: String,
    pub user: Option<String>,
    pub explain_plan: Option<String>,
    pub recommendations: Vec<String>,
    pub table_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryProfile {
    pub sql: String,
    pub execution_time_ms: u64,
    pub planning_time_ms: u64,
    pub execution_time_breakdown: HashMap<String, u64>,
    pub rows_scanned: u64,
    pub rows_returned: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub sort_operations: u64,
    pub join_operations: u64,
    pub index_usage: Vec<IndexUsageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexUsageInfo {
    pub index_name: String,
    pub table_name: String,
    pub usage_count: u64,
    pub index_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryOptimizationSuggestion {
    pub query: String,
    pub issue_type: IssueType,
    pub severity: Severity,
    pub description: String,
    pub recommendation: String,
    pub estimated_improvement: Option<String>,
    pub auto_fix_sql: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueType {
    MissingIndex,
    FullTableScan,
    CartesianJoin,
    NPlusOneQuery,
    SelectStar,
    OrderByWithoutIndex,
    LikePrefixSearch,
    ORCondition,
    ImplicitConversion,
    SubqueryInWhere,
    CountStar,
    NoLimit,
    UnusedIndex,
    LargeTableScan,
    ShardOverload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "Critical"),
            Severity::High => write!(f, "High"),
            Severity::Medium => write!(f, "Medium"),
            Severity::Low => write!(f, "Low"),
            Severity::Info => write!(f, "Info"),
        }
    }
}

pub struct SlowQueryDetector {
    config: SlowQueryConfig,
    logs: Vec<SlowQueryLog>,
    connection: Box<dyn DatabaseConnection>,
    db_type: String,
    table_query_count: HashMap<String, u64>,
    table_slow_count: HashMap<String, u64>,
}

impl SlowQueryDetector {
    pub fn new(connection: Box<dyn DatabaseConnection>, config: SlowQueryConfig, db_type: &str) -> Self {
        Self {
            config,
            logs: Vec::new(),
            connection,
            db_type: db_type.to_string(),
            table_query_count: HashMap::new(),
            table_slow_count: HashMap::new(),
        }
    }

    pub fn with_default_config(connection: Box<dyn DatabaseConnection>, db_type: &str) -> Self {
        Self::new(connection, SlowQueryConfig::default(), db_type)
    }

    pub async fn execute_and_analyze(&mut self, sql: &str) -> Result<(bool, QueryProfile)> {
        let start = Instant::now();
        let threshold = Duration::from_millis(self.config.threshold_ms);

        let result = self.connection.query(sql).await;
        let execution_time = start.elapsed().as_millis() as u64;

        let is_slow = start.elapsed() > threshold;

        let profile = self.build_profile(sql, execution_time).await?;

        let table_name = Self::extract_table_name(sql);

        if let Some(ref table) = table_name {
            *self.table_query_count.entry(table.clone()).or_insert(0) += 1;
            if is_slow {
                *self.table_slow_count.entry(table.clone()).or_insert(0) += 1;
            }
        }

        if is_slow && self.config.enable_logging {
            let explain_plan = if self.config.include_explain {
                self.get_explain_plan(sql).await.ok()
            } else {
                None
            };

            let suggestions = self.analyze_query(sql).await.unwrap_or_default();

            let log_entry = SlowQueryLog {
                id: format!("sq_{}", chrono::Utc::now().timestamp_millis()),
                sql: sql.to_string(),
                execution_time_ms: execution_time,
                rows_affected: 0,
                timestamp: chrono::Utc::now().timestamp(),
                database: String::new(),
                user: None,
                explain_plan,
                recommendations: suggestions.iter().map(|s| s.recommendation.clone()).collect(),
                table_name: table_name.clone(),
            };

            self.add_log(log_entry);
        }

        match result {
            Ok(_) => Ok((is_slow, profile)),
            Err(e) => Err(e),
        }
    }

    fn extract_table_name(sql: &str) -> Option<String> {
        let sql_lower = sql.to_lowercase();
        let patterns = [
            "from ",
            "into ",
            "update ",
            "join ",
        ];

        for pattern in &patterns {
            if let Some(pos) = sql_lower.find(pattern) {
                let after = &sql_lower[pos + pattern.len()..];
                let table = after.split_whitespace().next()?;
                let clean = table.trim_matches(|c| c == '(' || c == ')' || c == ',' || c == ';');
                if !clean.is_empty() {
                    return Some(clean.to_string());
                }
            }
        }
        None
    }

    pub fn get_table_slow_ratio(&self, table_name: &str) -> Option<f64> {
        let total = self.table_query_count.get(table_name)?;
        let slow = self.table_slow_count.get(table_name)?;
        if *total == 0 {
            return None;
        }
        Some(*slow as f64 / *total as f64)
    }

    pub fn get_tables_needing_optimization(&self) -> Vec<(String, f64)> {
        let mut result: Vec<(String, f64)> = self.table_query_count.iter()
            .filter_map(|(table, total)| {
                let slow = self.table_slow_count.get(table).unwrap_or(&0);
                if *total > 10 {
                    let ratio = *slow as f64 / *total as f64;
                    if ratio >= self.config.alert_threshold_ratio {
                        return Some((table.clone(), ratio));
                    }
                }
                None
            })
            .collect();

        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        result
    }

    pub fn generate_optimization_report(&self) -> SlowQueryReport {
        let tables_to_optimize = self.get_tables_needing_optimization();
        let avg_time = self.get_average_slow_query_time().unwrap_or(0);

        SlowQueryReport {
            total_slow_queries: self.logs.len(),
            average_execution_time_ms: avg_time,
            tables_needing_attention: tables_to_optimize,
            recent_slow_queries: self.logs.iter().take(10).cloned().collect(),
            generated_at: chrono::Utc::now().timestamp(),
        }
    }

    async fn build_profile(&self, sql: &str, execution_time_ms: u64) -> Result<QueryProfile> {
        let mut breakdown = HashMap::new();
        breakdown.insert("execution".to_string(), execution_time_ms);
        breakdown.insert("planning".to_string(), execution_time_ms / 10);

        Ok(QueryProfile {
            sql: sql.to_string(),
            execution_time_ms,
            planning_time_ms: execution_time_ms / 10,
            execution_time_breakdown: breakdown,
            rows_scanned: 0,
            rows_returned: 0,
            cache_hits: 0,
            cache_misses: 1,
            sort_operations: 0,
            join_operations: 0,
            index_usage: Vec::new(),
        })
    }

    pub async fn get_explain_plan(&self, sql: &str) -> Result<String> {
        let explain_sql = match self.db_type.as_str() {
            "mysql" => format!("EXPLAIN {}", sql),
            "pgsql" | "postgres" => format!("EXPLAIN (ANALYZE, BUFFERS) {}", sql),
            "sqlite" => format!("EXPLAIN QUERY PLAN {}", sql),
            _ => format!("EXPLAIN {}", sql),
        };

        let rows = self.connection.query(&explain_sql).await?;
        let mut plan = String::new();

        for row in rows {
            if let Some(value) = row.get("QUERY PLAN").or_else(|| row.get("plan")) {
                plan.push_str(&value.to_string());
                plan.push('\n');
            }
        }

        Ok(plan.trim().to_string())
    }

    pub async fn analyze_query(&self, sql: &str) -> Result<Vec<QueryOptimizationSuggestion>> {
        let mut suggestions = Vec::new();
        let sql_lower = sql.to_lowercase();

        if sql_lower.contains("select *") || sql_lower.ends_with("select *") {
            suggestions.push(QueryOptimizationSuggestion {
                query: sql.to_string(),
                issue_type: IssueType::SelectStar,
                severity: Severity::Medium,
                description: "查询使用了 SELECT * 可能导致不必要的数据传输".to_string(),
                recommendation: "明确指定需要的列名而不是使用 SELECT *".to_string(),
                estimated_improvement: Some("减少20-50%的数据传输量".to_string()),
                auto_fix_sql: None,
            });
        }

        if sql_lower.contains(" like '%") {
            suggestions.push(QueryOptimizationSuggestion {
                query: sql.to_string(),
                issue_type: IssueType::LikePrefixSearch,
                severity: Severity::Medium,
                description: "LIKE 查询以前导通配符开头无法使用索引".to_string(),
                recommendation: "如果可能，重构查询使用后置通配符或全文索引".to_string(),
                estimated_improvement: Some("可将查询从全表扫描改为索引扫描".to_string()),
                auto_fix_sql: None,
            });
        }

        if sql_lower.contains(" or ") && sql_lower.contains(" where ") {
            suggestions.push(QueryOptimizationSuggestion {
                query: sql.to_string(),
                issue_type: IssueType::ORCondition,
                severity: Severity::Medium,
                description: "OR 条件可能导致全表扫描".to_string(),
                recommendation: "考虑使用 UNION 替代 OR 或确保每个条件都有索引".to_string(),
                estimated_improvement: Some("取决于索引覆盖情况".to_string()),
                auto_fix_sql: None,
            });
        }

        if sql_lower.contains(" order by ") && !sql_lower.contains(" limit ") {
            suggestions.push(QueryOptimizationSuggestion {
                query: sql.to_string(),
                issue_type: IssueType::OrderByWithoutIndex,
                severity: Severity::Low,
                description: "ORDER BY 没有 LIMIT 限制可能返回大量数据".to_string(),
                recommendation: "添加 LIMIT 限制返回行数".to_string(),
                estimated_improvement: Some("减少内存使用和排序开销".to_string()),
                auto_fix_sql: None,
            });
        }

        if sql_lower.contains(" from ") && !sql_lower.contains(" where ") {
            suggestions.push(QueryOptimizationSuggestion {
                query: sql.to_string(),
                issue_type: IssueType::NoLimit,
                severity: Severity::Low,
                description: "查询没有 WHERE 条件和 LIMIT".to_string(),
                recommendation: "添加 WHERE 条件和 LIMIT 限制".to_string(),
                estimated_improvement: Some("防止返回过多数据".to_string()),
                auto_fix_sql: None,
            });
        }

        if sql_lower.contains("not in") || sql_lower.contains("not exists") {
            suggestions.push(QueryOptimizationSuggestion {
                query: sql.to_string(),
                issue_type: IssueType::SubqueryInWhere,
                severity: Severity::Low,
                description: "NOT IN 或 NOT EXISTS 可能效率较低".to_string(),
                recommendation: "考虑使用 LEFT JOIN WHERE ... IS NULL 模式".to_string(),
                estimated_improvement: Some("通常可提升10-50%性能".to_string()),
                auto_fix_sql: None,
            });
        }

        if sql_lower.contains("count(*)") && sql_lower.contains(" from ") && !sql_lower.contains(" where ") {
            suggestions.push(QueryOptimizationSuggestion {
                query: sql.to_string(),
                issue_type: IssueType::CountStar,
                severity: Severity::Info,
                description: "COUNT(*) 没有 WHERE 条件".to_string(),
                recommendation: "考虑使用索引覆盖的 COUNT(*) 或缓存结果".to_string(),
                estimated_improvement: Some("取决于表大小".to_string()),
                auto_fix_sql: None,
            });
        }

        Ok(suggestions)
    }

    pub async fn check_missing_indexes(&self, sql: &str) -> Result<Vec<QueryOptimizationSuggestion>> {
        let explain_plan = self.get_explain_plan(sql).await?;
        let mut suggestions = Vec::new();

        if explain_plan.contains("SCAN") || explain_plan.contains("FULL") {
            suggestions.push(QueryOptimizationSuggestion {
                query: sql.to_string(),
                issue_type: IssueType::FullTableScan,
                severity: Severity::High,
                description: "查询执行了全表扫描".to_string(),
                recommendation: "分析 WHERE 子句和 JOIN 条件，考虑创建合适的索引".to_string(),
                estimated_improvement: Some("索引可将查询提升100-1000倍".to_string()),
                auto_fix_sql: None,
            });
        }

        Ok(suggestions)
    }

    pub fn suggest_index_creation(&self, sql: &str) -> Vec<String> {
        Self::suggest_index_creation_static(sql)
    }

    pub fn suggest_index_creation_static(sql: &str) -> Vec<String> {
        let sql_lower = sql.to_lowercase();
        let mut suggestions = Vec::new();

        if let Some(table) = Self::extract_table_name(sql) {
            if sql_lower.contains("where ") {
                let where_part = sql_lower.split("where ").nth(1).unwrap_or("");
                let conditions: Vec<&str> = where_part.split(" and ").collect();

                for cond in conditions {
                    let field = cond.split(|c| c == '=' || c == '>' || c == '<').next();
                    if let Some(f) = field {
                        let clean = f.trim().trim_start_matches(|c| c == '(' || c == ' ');
                        if !clean.is_empty() && !clean.contains(" ") {
                            suggestions.push(format!(
                                "CREATE INDEX idx_{}_{} ON {} ({});",
                                table, clean, table, clean
                            ));
                        }
                    }
                }
            }
        }

        suggestions
    }

    pub fn generate_auto_fix_sql(&self, suggestion: &QueryOptimizationSuggestion) -> Option<String> {
        Self::generate_auto_fix_sql_static(suggestion)
    }

    pub fn generate_auto_fix_sql_static(suggestion: &QueryOptimizationSuggestion) -> Option<String> {
        match suggestion.issue_type {
            IssueType::SelectStar => {
                let sql = &suggestion.query;
                if let Some(table) = Self::extract_table_name(sql) {
                    Some(sql.replace("SELECT *", &format!("SELECT id, {}_fields", table)))
                } else {
                    None
                }
            }
            IssueType::NoLimit => {
                Some(format!("{} LIMIT 100", suggestion.query))
            }
            IssueType::OrderByWithoutIndex => {
                Some(format!("{} LIMIT 1000", suggestion.query))
            }
            _ => None,
        }
    }

    fn add_log(&mut self, log: SlowQueryLog) {
        self.logs.insert(0, log);
        if self.logs.len() > self.config.max_log_entries {
            self.logs.pop();
        }
    }

    pub fn get_logs(&self) -> &[SlowQueryLog] {
        &self.logs
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    pub fn get_slow_query_count(&self) -> usize {
        self.logs.len()
    }

    pub fn get_average_slow_query_time(&self) -> Option<u64> {
        if self.logs.is_empty() {
            return None;
        }
        let total: u64 = self.logs.iter().map(|l| l.execution_time_ms).sum();
        Some(total / self.logs.len() as u64)
    }

    pub fn get_threshold(&self) -> u64 {
        self.config.threshold_ms
    }

    pub fn set_threshold(&mut self, threshold_ms: u64) {
        self.config.threshold_ms = threshold_ms;
    }

    pub fn get_logs_by_table(&self, table_name: &str) -> Vec<&SlowQueryLog> {
        self.logs.iter()
            .filter(|log| log.table_name.as_ref().map(|t| t == table_name).unwrap_or(false))
            .collect()
    }

    pub fn get_top_slow_queries(&self, limit: usize) -> Vec<&SlowQueryLog> {
        let mut sorted: Vec<&SlowQueryLog> = self.logs.iter().collect();
        sorted.sort_by(|a, b| b.execution_time_ms.cmp(&a.execution_time_ms));
        sorted.into_iter().take(limit).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryReport {
    pub total_slow_queries: usize,
    pub average_execution_time_ms: u64,
    pub tables_needing_attention: Vec<(String, f64)>,
    pub recent_slow_queries: Vec<SlowQueryLog>,
    pub generated_at: i64,
}

pub struct QueryMetrics {
    total_queries: u64,
    slow_queries: u64,
    failed_queries: u64,
    average_execution_time: u64,
    queries_by_type: HashMap<String, u64>,
}

impl QueryMetrics {
    pub fn new() -> Self {
        Self {
            total_queries: 0,
            slow_queries: 0,
            failed_queries: 0,
            average_execution_time: 0,
            queries_by_type: HashMap::new(),
        }
    }

    pub fn record_query(&mut self, execution_time_ms: u64, sql: &str, is_slow: bool, is_failed: bool) {
        self.total_queries += 1;

        if is_slow {
            self.slow_queries += 1;
        }

        if is_failed {
            self.failed_queries += 1;
        }

        let sql_type = Self::classify_sql(sql);
        *self.queries_by_type.entry(sql_type).or_insert(0) += 1;

        let total_time = self.average_execution_time * (self.total_queries - 1) + execution_time_ms;
        self.average_execution_time = total_time / self.total_queries;
    }

    fn classify_sql(sql: &str) -> String {
        let upper = sql.trim().to_uppercase();
        if upper.starts_with("SELECT") {
            "SELECT".to_string()
        } else if upper.starts_with("INSERT") {
            "INSERT".to_string()
        } else if upper.starts_with("UPDATE") {
            "UPDATE".to_string()
        } else if upper.starts_with("DELETE") {
            "DELETE".to_string()
        } else {
            "OTHER".to_string()
        }
    }

    pub fn slow_query_ratio(&self) -> Option<f64> {
        if self.total_queries == 0 {
            return None;
        }
        Some(self.slow_queries as f64 / self.total_queries as f64)
    }

    pub fn format(&self) -> String {
        format!(
            "Query Metrics:\n  Total: {} | Slow: {} | Failed: {}\n  Avg Time: {}ms\n  By Type: {:?}",
            self.total_queries,
            self.slow_queries,
            self.failed_queries,
            self.average_execution_time,
            self.queries_by_type
        )
    }
}

impl Default for QueryMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slow_query_config_default() {
        let config = SlowQueryConfig::default();
        assert_eq!(config.threshold_ms, 1000);
        assert!(config.enable_logging);
    }

    #[test]
    fn test_query_metrics() {
        let mut metrics = QueryMetrics::new();
        metrics.record_query(100, "SELECT * FROM users", false, false);
        metrics.record_query(2000, "SELECT * FROM orders", true, false);
        metrics.record_query(50, "INSERT INTO users VALUES (1)", false, false);

        assert_eq!(metrics.total_queries, 3);
        assert_eq!(metrics.slow_queries, 1);
        assert_eq!(metrics.slow_query_ratio(), Some(1.0 / 3.0));
    }

    #[test]
    fn test_query_classification() {
        assert_eq!(QueryMetrics::classify_sql("SELECT * FROM users"), "SELECT");
        assert_eq!(QueryMetrics::classify_sql("INSERT INTO users VALUES (1)"), "INSERT");
        assert_eq!(QueryMetrics::classify_sql("UPDATE users SET name = 'test'"), "UPDATE");
        assert_eq!(QueryMetrics::classify_sql("DELETE FROM users WHERE id = 1"), "DELETE");
    }

    #[test]
    fn test_extract_table_name() {
        assert_eq!(
            SlowQueryDetector::extract_table_name("SELECT * FROM users WHERE id = 1"),
            Some("users".to_string())
        );
        assert_eq!(
            SlowQueryDetector::extract_table_name("INSERT INTO orders VALUES (1)"),
            Some("orders".to_string())
        );
        assert_eq!(
            SlowQueryDetector::extract_table_name("UPDATE products SET name = 'test'"),
            Some("products".to_string())
        );
    }

    #[test]
    fn test_suggest_index_creation() {
        let sql = "SELECT * FROM users WHERE age = 25 AND status = 'active'";
        let indexes = SlowQueryDetector::suggest_index_creation_static(sql);
        assert!(!indexes.is_empty());
        assert!(indexes.iter().any(|i| i.contains("users")));
    }

    #[test]
    fn test_generate_auto_fix_sql() {
        let suggestion = QueryOptimizationSuggestion {
            query: "SELECT * FROM users".to_string(),
            issue_type: IssueType::NoLimit,
            severity: Severity::Low,
            description: "No limit".to_string(),
            recommendation: "Add limit".to_string(),
            estimated_improvement: None,
            auto_fix_sql: None,
        };

        let fixed = SlowQueryDetector::generate_auto_fix_sql_static(&suggestion);
        assert!(fixed.is_some());
        assert!(fixed.unwrap().contains("LIMIT"));
    }

    #[test]
    fn test_slow_query_report() {
        let report = SlowQueryReport {
            total_slow_queries: 0,
            average_execution_time_ms: 0,
            tables_needing_attention: vec![],
            recent_slow_queries: vec![],
            generated_at: chrono::Utc::now().timestamp(),
        };
        assert_eq!(report.total_slow_queries, 0);
        assert!(report.tables_needing_attention.is_empty());
    }
}
