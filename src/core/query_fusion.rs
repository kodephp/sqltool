use crate::core::sharding::ShardRouter;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionQuery {
    pub sql: String,
    pub target_shards: Vec<String>,
    pub parameters: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionResult {
    pub shard_results: HashMap<String, Vec<serde_json::Value>>,
    pub merged: Vec<serde_json::Value>,
    pub total_count: usize,
    pub execution_time_ms: u64,
    pub strategy_used: String,
}

#[derive(Debug, Clone)]
pub struct UnionStrategy {
    pub merge_fields: Vec<String>,
    pub deduplicate: bool,
}

#[derive(Debug, Clone)]
pub struct JoinStrategy {
    pub join_type: JoinType,
    pub left_field: String,
    pub right_field: String,
}

#[derive(Debug, Clone)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

pub struct QueryFusionEngine {
    shard_router: Arc<ShardRouter>,
    default_strategy: FusionStrategy,
}

#[derive(Debug, Clone)]
pub enum FusionStrategy {
    UnionAll,
    UnionDistinct,
    HashJoin { left_key: String, right_key: String },
    BroadcastJoin { key: String },
    SortMergeJoin { left_key: String, right_key: String },
    AggregationMerge { group_by: Vec<String>, aggregations: Vec<String> },
    TopN { field: String, n: usize, ascending: bool },
}

impl QueryFusionEngine {
    pub fn new(shard_router: Arc<ShardRouter>) -> Self {
        Self {
            shard_router,
            default_strategy: FusionStrategy::UnionAll,
        }
    }

    pub fn with_strategy(shard_router: Arc<ShardRouter>, strategy: FusionStrategy) -> Self {
        Self {
            shard_router,
            default_strategy: strategy,
        }
    }

    pub async fn execute_fusion_query(
        &self,
        query: &FusionQuery,
        strategy: Option<FusionStrategy>,
    ) -> Result<FusionResult> {
        let start = std::time::Instant::now();
        let strategy = strategy.unwrap_or(self.default_strategy.clone());

        let mut shard_results = HashMap::new();

        for shard_id in &query.target_shards {
            let result = self.execute_on_shard(shard_id, query).await?;
            shard_results.insert(shard_id.clone(), result);
        }

        let merged = self.merge_results(&shard_results, &strategy)?;
        let total_count = merged.len();

        Ok(FusionResult {
            shard_results,
            merged,
            total_count,
            execution_time_ms: start.elapsed().as_millis() as u64,
            strategy_used: format!("{:?}", strategy),
        })
    }

    async fn execute_on_shard(
        &self,
        _shard_id: &str,
        _query: &FusionQuery,
    ) -> Result<Vec<serde_json::Value>> {
        Ok(Vec::new())
    }

    fn merge_results(
        &self,
        shard_results: &HashMap<String, Vec<serde_json::Value>>,
        strategy: &FusionStrategy,
    ) -> Result<Vec<serde_json::Value>> {
        match strategy {
            FusionStrategy::UnionAll => {
                let mut merged = Vec::new();
                for (_, results) in shard_results {
                    merged.extend(results.clone());
                }
                Ok(merged)
            }
            FusionStrategy::UnionDistinct => {
                let mut merged = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for (_, results) in shard_results {
                    for result in results {
                        let key = serde_json::to_string(result)?;
                        if !seen.contains(&key) {
                            seen.insert(key);
                            merged.push(result.clone());
                        }
                    }
                }
                Ok(merged)
            }
            FusionStrategy::HashJoin { left_key, right_key } => {
                self.hash_join(shard_results, left_key, right_key)
            }
            FusionStrategy::BroadcastJoin { key } => {
                self.broadcast_join(shard_results, key)
            }
            FusionStrategy::SortMergeJoin { left_key, right_key } => {
                self.sort_merge_join(shard_results, left_key, right_key)
            }
            FusionStrategy::AggregationMerge { group_by, aggregations } => {
                self.aggregation_merge(shard_results, group_by, aggregations)
            }
            FusionStrategy::TopN { field, n, ascending } => {
                self.top_n_merge(shard_results, field, *n, *ascending)
            }
        }
    }

    fn hash_join(
        &self,
        shard_results: &HashMap<String, Vec<serde_json::Value>>,
        left_key: &str,
        right_key: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut left_results = Vec::new();
        let mut right_results = Vec::new();
        
        let shards: Vec<_> = shard_results.keys().collect();
        if shards.len() >= 2 {
            left_results = shard_results.get(shards[0]).cloned().unwrap_or_default();
            right_results = shard_results.get(shards[1]).cloned().unwrap_or_default();
        } else {
            for (_, results) in shard_results {
                left_results.extend(results.clone());
                right_results.extend(results.clone());
            }
        }
        
        let mut right_map: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
        for right_row in &right_results {
            if let Some(key_val) = right_row.get(right_key).map(|v| v.to_string()) {
                right_map.entry(key_val).or_default().push(right_row.clone());
            }
        }
        
        let mut merged = Vec::new();
        for left_row in &left_results {
            if let Some(key_val) = left_row.get(left_key).map(|v| v.to_string()) {
                if let Some(right_rows) = right_map.get(&key_val) {
                    for right_row in right_rows {
                        let mut combined = left_row.clone();
                        for (k, v) in right_row.as_object().unwrap_or(&serde_json::Map::new()) {
                            if k != right_key {
                                combined.as_object_mut().unwrap().insert(k.clone(), v.clone());
                            }
                        }
                        merged.push(combined);
                    }
                }
            }
        }
        
        Ok(merged)
    }

    fn broadcast_join(
        &self,
        shard_results: &HashMap<String, Vec<serde_json::Value>>,
        key: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut broadcast_data = Vec::new();
        let mut main_data = Vec::new();
        
        let shards: Vec<_> = shard_results.keys().collect();
        if !shards.is_empty() {
            broadcast_data = shard_results.get(shards[0]).cloned().unwrap_or_default();
            for i in 1..shards.len() {
                main_data.extend(shard_results.get(shards[i]).cloned().unwrap_or_default());
            }
        }
        
        if main_data.is_empty() {
            main_data = broadcast_data.clone();
            broadcast_data.clear();
        }
        
        let mut broadcast_map: HashMap<String, serde_json::Value> = HashMap::new();
        for row in &broadcast_data {
            if let Some(key_val) = row.get(key) {
                broadcast_map.insert(key_val.to_string(), row.clone());
            }
        }
        
        let mut merged = Vec::new();
        for main_row in &main_data {
            if let Some(key_val) = main_row.get(key).map(|v| v.to_string()) {
                if let Some(broadcast_row) = broadcast_map.get(&key_val) {
                    let mut combined = main_row.clone();
                    for (k, v) in broadcast_row.as_object().unwrap_or(&serde_json::Map::new()) {
                        if k != key {
                            combined.as_object_mut().unwrap().insert(k.clone(), v.clone());
                        }
                    }
                    merged.push(combined);
                }
            }
        }
        
        Ok(merged)
    }

    fn sort_merge_join(
        &self,
        shard_results: &HashMap<String, Vec<serde_json::Value>>,
        left_key: &str,
        right_key: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut left_results = Vec::new();
        let mut right_results = Vec::new();
        
        let shards: Vec<_> = shard_results.keys().collect();
        if shards.len() >= 2 {
            left_results = shard_results.get(shards[0]).cloned().unwrap_or_default();
            right_results = shard_results.get(shards[1]).cloned().unwrap_or_default();
        } else {
            for (_, results) in shard_results {
                left_results.extend(results.clone());
                right_results.extend(results.clone());
            }
        }
        
        left_results.sort_by(|a, b| {
            let a_key = a.get(left_key).map(|v| v.to_string()).unwrap_or_default();
            let b_key = b.get(left_key).map(|v| v.to_string()).unwrap_or_default();
            a_key.cmp(&b_key)
        });
        
        right_results.sort_by(|a, b| {
            let a_key = a.get(right_key).map(|v| v.to_string()).unwrap_or_default();
            let b_key = b.get(right_key).map(|v| v.to_string()).unwrap_or_default();
            a_key.cmp(&b_key)
        });
        
        let mut merged = Vec::new();
        let mut right_idx = 0;
        
        for left_row in &left_results {
            let left_key_val = left_row.get(left_key).map(|v| v.to_string()).unwrap_or_default();
            
            while right_idx < right_results.len() {
                let right_key_val = right_results[right_idx].get(right_key).map(|v| v.to_string()).unwrap_or_default();
                
                match right_key_val.cmp(&left_key_val) {
                    std::cmp::Ordering::Less => right_idx += 1,
                    std::cmp::Ordering::Equal => {
                        let mut combined = left_row.clone();
                        for (k, v) in right_results[right_idx].as_object().unwrap_or(&serde_json::Map::new()) {
                            if k != right_key {
                                combined.as_object_mut().unwrap().insert(k.clone(), v.clone());
                            }
                        }
                        merged.push(combined);
                        right_idx += 1;
                        break;
                    }
                    std::cmp::Ordering::Greater => break,
                }
            }
        }
        
        Ok(merged)
    }

    fn aggregation_merge(
        &self,
        shard_results: &HashMap<String, Vec<serde_json::Value>>,
        group_by: &[String],
        aggregations: &[String],
    ) -> Result<Vec<serde_json::Value>> {
        let mut all_results = Vec::new();
        for (_, results) in shard_results {
            all_results.extend(results.clone());
        }

        let mut groups: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

        for row in &all_results {
            let mut key_parts = Vec::new();
            for field in group_by {
                let val = row.get(field).map(|v| v.to_string()).unwrap_or_default();
                key_parts.push(format!("{}={}", field, val));
            }
            let group_key = key_parts.join("|");
            groups.entry(group_key).or_default().push(row.clone());
        }

        let mut merged = Vec::new();
        for (_group_key, rows) in groups {
            let mut aggregated = serde_json::Map::new();

            for field in group_by {
                if let Some(first) = rows.first() {
                    if let Some(val) = first.get(field) {
                        aggregated.insert(field.clone(), val.clone());
                    }
                }
            }

            for agg in aggregations {
                if agg.starts_with("count(") {
                    aggregated.insert(format!("count_{}", agg), serde_json::json!(rows.len()));
                } else if agg.starts_with("sum(") {
                    let field = agg.trim_start_matches("sum(").trim_end_matches(")");
                    let sum: f64 = rows.iter()
                        .filter_map(|r| r.get(field).and_then(|v| v.as_f64()))
                        .sum();
                    aggregated.insert(format!("sum_{}", field), serde_json::json!(sum));
                }
            }

            merged.push(serde_json::Value::Object(aggregated));
        }

        Ok(merged)
    }

    fn top_n_merge(
        &self,
        shard_results: &HashMap<String, Vec<serde_json::Value>>,
        field: &str,
        n: usize,
        ascending: bool,
    ) -> Result<Vec<serde_json::Value>> {
        let mut all_results = Vec::new();
        for (_, results) in shard_results {
            all_results.extend(results.clone());
        }

        all_results.sort_by(|a, b| {
            let a_val = a.get(field).map(|v| v.to_string()).unwrap_or_default();
            let b_val = b.get(field).map(|v| v.to_string()).unwrap_or_default();
            if ascending {
                a_val.cmp(&b_val)
            } else {
                b_val.cmp(&a_val)
            }
        });

        Ok(all_results.into_iter().take(n).collect())
    }

    pub fn route_query_to_shards(&self, sql: &str, params: &[serde_json::Value]) -> Result<FusionQuery> {
        let target_shards = self.analyze_and_route(sql);

        Ok(FusionQuery {
            sql: sql.to_string(),
            target_shards,
            parameters: params.to_vec(),
        })
    }

    fn analyze_and_route(&self, sql: &str) -> Vec<String> {
        let sql_lower = sql.to_lowercase();
        if sql_lower.contains("users") {
            vec!["users_db".to_string()]
        } else if sql_lower.contains("orders") {
            vec!["orders_db_0".to_string(), "orders_db_1".to_string()]
        } else {
            vec!["default".to_string()]
        }
    }

    pub fn get_cross_shard_query_plan(&self, sql: &str) -> Result<CrossShardPlan> {
        let analysis = self.analyze_query(sql)?;
        let shards = vec!["shard_0".to_string()];

        let recommended_strategy = match analysis.query_type {
            QueryType::Select => {
                if analysis.has_aggregation {
                    FusionStrategy::AggregationMerge {
                        group_by: vec!["group_field".to_string()],
                        aggregations: vec!["count(*)".to_string()],
                    }
                } else {
                    self.default_strategy.clone()
                }
            }
            _ => self.default_strategy.clone(),
        };

        Ok(CrossShardPlan {
            original_sql: sql.to_string(),
            target_shards: shards,
            analysis,
            recommended_strategy,
        })
    }

    fn analyze_query(&self, sql: &str) -> Result<QueryAnalysis> {
        let sql_lower = sql.to_lowercase();
        
        let query_type = if sql_lower.contains("select") {
            QueryType::Select
        } else if sql_lower.contains("insert") {
            QueryType::Insert
        } else if sql_lower.contains("update") {
            QueryType::Update
        } else if sql_lower.contains("delete") {
            QueryType::Delete
        } else {
            QueryType::Other
        };
        
        let has_join = sql_lower.contains("join");
        let has_aggregation = sql_lower.contains("group by") || sql_lower.contains("count(") || sql_lower.contains("sum(");
        let has_order_by = sql_lower.contains("order by");
        let has_limit = sql_lower.contains("limit");
        
        Ok(QueryAnalysis {
            query_type,
            has_join,
            has_aggregation,
            has_order_by,
            has_limit,
            estimated_complexity: Self::estimate_complexity(&sql_lower),
        })
    }

    fn estimate_complexity(sql: &str) -> ComplexityLevel {
        let score = sql.matches("join").count() * 3
            + sql.matches("group by").count() * 2
            + sql.matches("order by").count() * 1
            + sql.matches("subquery").count() * 2;
        
        match score {
            0..=2 => ComplexityLevel::Simple,
            3..=5 => ComplexityLevel::Medium,
            _ => ComplexityLevel::Complex,
        }
    }

    pub fn auto_select_strategy(&self, sql: &str) -> FusionStrategy {
        let analysis = self.analyze_query(sql).unwrap_or(
            QueryAnalysis {
                query_type: QueryType::Select,
                has_join: false,
                has_aggregation: false,
                has_order_by: false,
                has_limit: false,
                estimated_complexity: ComplexityLevel::Simple,
            }
        );

        if analysis.has_aggregation {
            FusionStrategy::AggregationMerge {
                group_by: vec![],
                aggregations: vec!["count(*)".to_string()],
            }
        } else if analysis.has_order_by && analysis.has_limit {
            FusionStrategy::TopN {
                field: "id".to_string(),
                n: 100,
                ascending: true,
            }
        } else if analysis.has_join {
            FusionStrategy::HashJoin {
                left_key: "id".to_string(),
                right_key: "user_id".to_string(),
            }
        } else {
            FusionStrategy::UnionAll
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrossShardPlan {
    pub original_sql: String,
    pub target_shards: Vec<String>,
    pub analysis: QueryAnalysis,
    pub recommended_strategy: FusionStrategy,
}

#[derive(Debug, Clone)]
pub struct QueryAnalysis {
    pub query_type: QueryType,
    pub has_join: bool,
    pub has_aggregation: bool,
    pub has_order_by: bool,
    pub has_limit: bool,
    pub estimated_complexity: ComplexityLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryType {
    Select,
    Insert,
    Update,
    Delete,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComplexityLevel {
    Simple,
    Medium,
    Complex,
}

impl std::fmt::Display for ComplexityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComplexityLevel::Simple => write!(f, "Simple"),
            ComplexityLevel::Medium => write!(f, "Medium"),
            ComplexityLevel::Complex => write!(f, "Complex"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fusion_strategy() {
        let strategy = FusionStrategy::HashJoin {
            left_key: "id".to_string(),
            right_key: "user_id".to_string(),
        };
        
        match strategy {
            FusionStrategy::HashJoin { left_key, right_key } => {
                assert_eq!(left_key, "id");
                assert_eq!(right_key, "user_id");
            }
            _ => panic!("Expected HashJoin"),
        }
    }

    #[test]
    fn test_query_analysis() {
        let sql = "SELECT * FROM users JOIN orders ON users.id = orders.user_id GROUP BY users.id ORDER BY users.name";
        let complexity = QueryFusionEngine::estimate_complexity(&sql.to_lowercase());
        assert_eq!(complexity, ComplexityLevel::Complex);
    }

    #[test]
    fn test_auto_select_strategy() {
        let router = Arc::new(ShardRouter::new());
        let engine = QueryFusionEngine::new(router);

        let agg_sql = "SELECT COUNT(*) FROM users GROUP BY status";
        let strategy = engine.auto_select_strategy(agg_sql);
        assert!(matches!(strategy, FusionStrategy::AggregationMerge { .. }));

        let join_sql = "SELECT * FROM users JOIN orders ON users.id = orders.user_id";
        let strategy = engine.auto_select_strategy(join_sql);
        assert!(matches!(strategy, FusionStrategy::HashJoin { .. }));

        let simple_sql = "SELECT * FROM users";
        let strategy = engine.auto_select_strategy(simple_sql);
        assert!(matches!(strategy, FusionStrategy::UnionAll));
    }

    #[test]
    fn test_top_n_merge() {
        let router = Arc::new(ShardRouter::new());
        let engine = QueryFusionEngine::new(router);

        let mut shard_results = HashMap::new();
        shard_results.insert("shard1".to_string(), vec![
            serde_json::json!({"id": 3, "name": "Charlie"}),
            serde_json::json!({"id": 1, "name": "Alice"}),
        ]);
        shard_results.insert("shard2".to_string(), vec![
            serde_json::json!({"id": 2, "name": "Bob"}),
            serde_json::json!({"id": 4, "name": "David"}),
        ]);

        let result = engine.top_n_merge(&shard_results, "id", 2, true).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_aggregation_merge() {
        let router = Arc::new(ShardRouter::new());
        let engine = QueryFusionEngine::new(router);

        let mut shard_results = HashMap::new();
        shard_results.insert("shard1".to_string(), vec![
            serde_json::json!({"status": "active", "amount": 100}),
            serde_json::json!({"status": "active", "amount": 200}),
        ]);
        shard_results.insert("shard2".to_string(), vec![
            serde_json::json!({"status": "inactive", "amount": 50}),
            serde_json::json!({"status": "active", "amount": 300}),
        ]);

        let result = engine.aggregation_merge(
            &shard_results,
            &["status".to_string()],
            &["count(*)".to_string()]
        ).unwrap();

        assert!(!result.is_empty());
    }
}
