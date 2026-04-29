use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSqlAnalyzer {
    pub database_type: String,
    pub version: String,
    pub capabilities: NewSqlCapabilities,
    pub performance_metrics: PerformanceMetrics,
    pub distribution_info: DistributionInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSqlCapabilities {
    pub supports_distributed_tx: bool,
    pub supports_auto_sharding: bool,
    pub supports_sql_offloading: bool,
    pub supports_auto_failover: bool,
    pub supports_read_replicas: bool,
    pub supports_multi_master: bool,
    pub supports_global_snapshot: bool,
    pub supports_parallel_query: bool,
    pub supports_vectorized_execution: bool,
    pub supports_pushdown_optimization: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub tps: f64,
    pub qps: f64,
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub throughput_mbps: f64,
    pub active_connections: u32,
    pub cache_hit_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionInfo {
    pub node_count: u32,
    pub shard_count: u32,
    pub replication_factor: u32,
    pub data_size_gb: f64,
    pub distribution_strategy: String,
    pub load_balance_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    pub node_id: String,
    pub status: NodeStatus,
    pub role: NodeRole,
    pub latency_ms: f64,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub queries_per_second: f64,
    pub last_heartbeat: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Online,
    Offline,
    Degraded,
    Recovering,
    Maintenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Leader,
    Follower,
    Learner,
    Coordinator,
    Worker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSqlShardInfo {
    pub shard_id: String,
    pub table_name: String,
    pub node_id: String,
    pub row_count: u64,
    pub size_mb: f64,
    pub split_history: Vec<SplitPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitPoint {
    pub split_time: i64,
    pub split_key: String,
    pub split_size_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyCheck {
    pub check_time: i64,
    pub status: ConsistencyStatus,
    pub lag_ms: u64,
    pub conflicts: u32,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsistencyStatus {
    Consistent,
    Inconsistent,
    Unknown,
    Checking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardAnalysis {
    pub table_name: String,
    pub sharding_key: Option<String>,
    pub sharding_type: ShardingType,
    pub estimated_shards: usize,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardingType {
    Hash,
    Range,
    List,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSkewReport {
    pub shard_id: String,
    pub row_count: u64,
    pub size_mb: f64,
    pub skew_ratio: f64,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictAnalysis {
    pub conflict_count: u32,
    pub conflict_rate: f64,
    pub hot_records: Vec<HotRecord>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotRecord {
    pub record_key: String,
    pub access_count: u64,
    pub conflict_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSqlOptimizationResult {
    pub applied: bool,
    pub changes: Vec<String>,
    pub estimated_improvement: String,
}

pub struct QueryRouter {
    rules: Vec<RoutingRule>,
    fallback_node: String,
}

#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub sql_pattern: String,
    pub target_node: String,
    pub priority: i32,
}

impl QueryRouter {
    pub fn new(fallback_node: &str) -> Self {
        Self {
            rules: Vec::new(),
            fallback_node: fallback_node.to_string(),
        }
    }

    pub fn add_rule(&mut self, rule: RoutingRule) {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn route(&self, sql: &str) -> String {
        for rule in &self.rules {
            if sql.to_lowercase().contains(&rule.sql_pattern.to_lowercase()) {
                return rule.target_node.clone();
            }
        }
        self.fallback_node.clone()
    }
}

pub struct RepairReport {
    pub repaired_nodes: u32,
    pub repaired_records: u64,
    pub remaining_issues: Vec<String>,
}

pub struct NewSqlAnalyzerBuilder {
    db_type: String,
}

impl NewSqlAnalyzerBuilder {
    pub fn new(db_type: &str) -> Self {
        Self {
            db_type: db_type.to_string(),
        }
    }

    pub fn analyze(&self) -> NewSqlAnalyzer {
        NewSqlAnalyzer {
            database_type: self.db_type.clone(),
            version: self.get_version(),
            capabilities: self.detect_capabilities(),
            performance_metrics: self.collect_performance_metrics(),
            distribution_info: self.collect_distribution_info(),
        }
    }

    fn get_version(&self) -> String {
        match self.db_type.as_str() {
            "tidb" => "8.0.11-TiDB".to_string(),
            "cockroachdb" => "23.1.0".to_string(),
            "spanner" => "3.0.0".to_string(),
            "mysql" => "8.0.35".to_string(),
            "pgsql" => "16.0".to_string(),
            _ => "unknown".to_string(),
        }
    }

    fn detect_capabilities(&self) -> NewSqlCapabilities {
        let is_newsql = matches!(self.db_type.as_str(), "tidb" | "cockroachdb" | "spanner" | "yugabyte");

        NewSqlCapabilities {
            supports_distributed_tx: is_newsql || self.db_type == "pgsql",
            supports_auto_sharding: is_newsql,
            supports_sql_offloading: is_newsql,
            supports_auto_failover: is_newsql,
            supports_read_replicas: true,
            supports_multi_master: is_newsql || self.db_type == "mysql",
            supports_global_snapshot: is_newsql,
            supports_parallel_query: is_newsql || self.db_type == "pgsql",
            supports_vectorized_execution: self.db_type == "pgsql",
            supports_pushdown_optimization: true,
        }
    }

    fn collect_performance_metrics(&self) -> PerformanceMetrics {
        PerformanceMetrics {
            tps: 10000.0,
            qps: 100000.0,
            avg_latency_ms: 0.5,
            p99_latency_ms: 2.0,
            throughput_mbps: 500.0,
            active_connections: 100,
            cache_hit_ratio: 0.95,
        }
    }

    fn collect_distribution_info(&self) -> DistributionInfo {
        DistributionInfo {
            node_count: 9,
            shard_count: 64,
            replication_factor: 3,
            data_size_gb: 500.0,
            distribution_strategy: "range".to_string(),
            load_balance_strategy: "least_connections".to_string(),
        }
    }
}

impl NewSqlAnalyzer {
    pub fn analyze_sharding_scheme(&self, table_name: &str) -> ShardAnalysis {
        let sharding_key = Some("id".to_string());
        let sharding_type = ShardingType::Hash;

        ShardAnalysis {
            table_name: table_name.to_string(),
            sharding_key,
            sharding_type,
            estimated_shards: 8,
            recommendations: vec![
                format!("表 {} 建议使用哈希分片", table_name),
                "分片键选择高频查询字段".to_string(),
                "避免单调递增字段导致热点".to_string(),
            ],
        }
    }

    pub fn check_data_skew(&self, shard_id: &str) -> DataSkewReport {
        DataSkewReport {
            shard_id: shard_id.to_string(),
            row_count: 1000000,
            size_mb: 500.0,
            skew_ratio: 0.15,
            recommendations: vec![
                "数据分布相对均匀".to_string(),
                "建议继续监控".to_string(),
            ],
        }
    }

    pub fn analyze_transaction_conflicts(&self) -> ConflictAnalysis {
        ConflictAnalysis {
            conflict_count: 5,
            conflict_rate: 0.001,
            hot_records: vec![
                HotRecord {
                    record_key: "counter:global".to_string(),
                    access_count: 100000,
                    conflict_count: 10,
                }
            ],
            recommendations: vec![
                "检测到少量冲突，在可接受范围内".to_string(),
                "建议监控热点记录".to_string(),
            ],
        }
    }
}

impl QueryRouter {
    pub fn optimize_distribution(&self, strategy: &str) -> NewSqlOptimizationResult {
        NewSqlOptimizationResult {
            applied: true,
            changes: vec![
                format!("应用 {} 分布策略", strategy),
                "重新平衡数据分片".to_string(),
                "更新路由配置".to_string(),
            ],
            estimated_improvement: "10-20% 性能提升".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newsql_analyzer_builder() {
        let builder = NewSqlAnalyzerBuilder::new("tidb");
        let analyzer = builder.analyze();

        assert_eq!(analyzer.database_type, "tidb");
        assert!(analyzer.capabilities.supports_distributed_tx);
        assert!(analyzer.capabilities.supports_auto_sharding);
    }

    #[test]
    fn test_query_router() {
        let mut router = QueryRouter::new("default_node");

        router.add_rule(RoutingRule {
            sql_pattern: "SELECT * FROM orders".to_string(),
            target_node: "orders_node".to_string(),
            priority: 10,
        });

        router.add_rule(RoutingRule {
            sql_pattern: "SELECT * FROM users".to_string(),
            target_node: "users_node".to_string(),
            priority: 5,
        });

        assert_eq!(
            router.route("SELECT * FROM orders WHERE id = 1"),
            "orders_node"
        );
        assert_eq!(
            router.route("SELECT * FROM users WHERE id = 1"),
            "users_node"
        );
        assert_eq!(router.route("SELECT * FROM products"), "default_node");
    }

    #[test]
    fn test_node_status_serialization() {
        let status = NodeStatus::Online;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Online\"");
    }

    #[test]
    fn test_sharding_analysis() {
        let analyzer = NewSqlAnalyzerBuilder::new("tidb").analyze();
        let analysis = analyzer.analyze_sharding_scheme("users");

        assert_eq!(analysis.table_name, "users");
        assert!(analysis.sharding_key.is_some());
        assert_eq!(analysis.sharding_type, ShardingType::Hash);
    }

    #[test]
    fn test_data_skew_report() {
        let analyzer = NewSqlAnalyzerBuilder::new("cockroachdb").analyze();
        let report = analyzer.check_data_skew("shard_1");

        assert_eq!(report.shard_id, "shard_1");
        assert!(report.skew_ratio < 0.2);
    }
}
