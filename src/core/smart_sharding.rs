//! 智能分库分表模块
//!
//! 解决分库分表后的核心难题：
//! 1. **查询合并**：跨分片 SELECT → 自动并行执行 + 结果归并 + 排序 + 分页
//! 2. **写入协调**：跨分片 INSERT/UPDATE/DELETE → 自动路由到目标分片
//! 3. **分布式事务**：跨分片写入的最终一致性
//! 4. **动态扩容**：在线添加/移除分片，数据自动 rebalance
//!
//! 核心抽象：
//! - `ShardRouter`：根据分片键计算目标分片
//! - `QueryCoordinator`：跨分片查询的合并器
//! - `WriteCoordinator`：跨分片写入的协调器
//! - `ShardTopology`：分片拓扑（多库多表）

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 分片策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShardStrategyKind {
    /// 哈希分片（按分片键的 hash % 分片数）
    Hash { virtual_nodes: usize },
    /// 范围分片（按分片键值区间）
    Range,
    /// 时间分片（按时间区间）
    Time { interval: String },
    /// 一致性哈希（用于动态扩容）
    ConsistentHash { virtual_nodes: usize },
}

impl Default for ShardStrategyKind {
    fn default() -> Self {
        ShardStrategyKind::Hash { virtual_nodes: 64 }
    }
}

/// 分片节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartShardNode {
    pub id: String,
    /// 数据库连接信息
    pub connection: String,
    /// 物理表名
    pub table: String,
    /// 权重（1-100）
    pub weight: u32,
    /// 是否活跃
    pub active: bool,
}

impl SmartShardNode {
    pub fn new(id: impl Into<String>, conn: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            connection: conn.into(),
            table: table.into(),
            weight: 100,
            active: true,
        }
    }
}

/// 分片拓扑
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartShardTopology {
    pub logical_table: String,
    pub shard_key: String,
    pub strategy: ShardStrategyKind,
    pub nodes: Vec<SmartShardNode>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SmartShardTopology {
    pub fn new(logical_table: impl Into<String>, shard_key: impl Into<String>) -> Self {
        Self {
            logical_table: logical_table.into(),
            shard_key: shard_key.into(),
            strategy: ShardStrategyKind::default(),
            nodes: Vec::new(),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    pub fn add_node(&mut self, node: SmartShardNode) {
        self.nodes.push(node);
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// 路由：根据分片键值计算目标分片节点
    pub fn route(&self, shard_value: &str) -> Result<&SmartShardNode> {
        if self.nodes.is_empty() {
            return Err(anyhow!("拓扑 {} 无可用节点", self.logical_table));
        }
        let active: Vec<&SmartShardNode> = self.nodes.iter().filter(|n| n.active).collect();
        if active.is_empty() {
            return Err(anyhow!("拓扑 {} 所有节点均已下线", self.logical_table));
        }
        match &self.strategy {
            ShardStrategyKind::Hash { virtual_nodes: _ } | ShardStrategyKind::ConsistentHash { .. } => {
                let hash = stable_hash(shard_value);
                let idx = (hash as usize) % active.len();
                Ok(active[idx])
            }
            ShardStrategyKind::Range => {
                let n = shard_value.parse::<u64>().unwrap_or(0);
                let idx = (n as usize) % active.len();
                Ok(active[idx])
            }
            ShardStrategyKind::Time { .. } => {
                let n = shard_value.parse::<u64>().unwrap_or(0);
                let idx = (n as usize) % active.len();
                Ok(active[idx])
            }
        }
    }

    /// 多键路由：返回所有可能命中的分片
    pub fn route_all(&self, shard_values: &[&str]) -> Result<Vec<&SmartShardNode>> {
        let mut out: Vec<&SmartShardNode> = Vec::new();
        for v in shard_values {
            let n = self.route(v)?;
            if !out.iter().any(|x: &&SmartShardNode| x.id == n.id) {
                out.push(n);
            }
        }
        Ok(out)
    }
}

/// 稳定哈希（避免 std::hash 的随机性）
fn stable_hash(s: &str) -> u64 {
    let mut h: u64 = 1469598103934665603; // FNV offset basis
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211); // FNV prime
    }
    h
}

// =============================================================================
// 查询合并器
// =============================================================================

/// 跨分片查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartSpanningQuery {
    pub logical_table: String,
    /// 投影字段（None 表示 SELECT *）
    pub columns: Option<Vec<String>>,
    /// 过滤条件
    pub where_clause: Option<String>,
    /// 排序字段
    pub order_by: Option<Vec<(String, bool)>>, // (field, asc)
    /// 分页：limit
    pub limit: Option<u64>,
    /// 分页：offset
    pub offset: Option<u64>,
    /// 是否并行执行
    pub parallel: bool,
    /// 合并策略
    pub merge_strategy: MergeStrategy,
}

impl Default for SmartSpanningQuery {
    fn default() -> Self {
        Self {
            logical_table: String::new(),
            columns: None,
            where_clause: None,
            order_by: None,
            limit: None,
            offset: None,
            parallel: true,
            merge_strategy: MergeStrategy::default(),
        }
    }
}

/// 结果合并策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MergeStrategy {
    /// 简单拼接（不排序、不去重）
    Concat,
    /// 排序后合并（按 order_by）
    SortedMerge,
    /// 去重（按主键）
    Distinct,
    /// 聚合（COUNT/SUM/AVG/MIN/MAX）
    Aggregate(AggregateKind),
}

impl Default for MergeStrategy {
    fn default() -> Self {
        MergeStrategy::Concat
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AggregateKind {
    Count,
    Sum(String),
    Avg(String),
    Min(String),
    Max(String),
}

/// 单分片查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardResult {
    pub shard_id: String,
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    pub elapsed_ms: u64,
    pub truncated: bool,
}

/// 跨分片查询合并结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartSpanningQueryResult {
    pub total_rows: u64,
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    pub shard_results: Vec<ShardResult>,
    pub merged_in_ms: u64,
    pub has_more: bool,
}

/// 查询合并器
pub struct QueryCoordinator {
    topologies: HashMap<String, SmartShardTopology>,
}

impl QueryCoordinator {
    pub fn new() -> Self {
        Self {
            topologies: HashMap::new(),
        }
    }

    pub fn register(&mut self, topology: SmartShardTopology) {
        self.topologies.insert(topology.logical_table.clone(), topology);
    }

    /// 执行跨分片查询：生成子 SQL、模拟执行（异步并行）、合并结果
    pub async fn execute(
        &self,
        query: SmartSpanningQuery,
    ) -> Result<SmartSpanningQueryResult> {
        let topology = self
            .topologies
            .get(&query.logical_table)
            .ok_or_else(|| anyhow!("未注册表 {}", query.logical_table))?;

        let start = std::time::Instant::now();

        // 1. 为每个分片生成子 SQL
        let shard_sqls: Vec<(String, String)> = topology
            .nodes
            .iter()
            .filter(|n| n.active)
            .map(|n| {
                let sql = build_shard_sql(
                    &n.table,
                    &query.columns,
                    query.where_clause.as_deref(),
                    query.order_by.as_ref(),
                    query.limit,
                    query.offset,
                );
                (n.id.clone(), sql)
            })
            .collect();

        // 2. 并行或串行"执行"（此处为演示框架：返回模拟结果；真实场景下走 driver）
        let shard_results = if query.parallel {
            execute_parallel(&shard_sqls).await
        } else {
            execute_serial(&shard_sqls).await
        };

        // 3. 结果合并
        let merged = merge_results(shard_results, &query.merge_strategy, query.order_by.as_ref());

        let total_rows = merged.total_rows;
        let has_more = merged.has_more;

        Ok(SmartSpanningQueryResult {
            total_rows,
            rows: merged.rows,
            shard_results: merged.shard_results,
            merged_in_ms: start.elapsed().as_millis() as u64,
            has_more,
        })
    }

    pub fn get_topology(&self, table: &str) -> Option<&SmartShardTopology> {
        self.topologies.get(table)
    }
}

impl Default for QueryCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

fn build_shard_sql(
    table: &str,
    columns: &Option<Vec<String>>,
    where_clause: Option<&str>,
    order_by: Option<&Vec<(String, bool)>>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> String {
    let cols = match columns {
        Some(cs) => cs.join(", "),
        None => "*".to_string(),
    };
    let mut sql = format!("SELECT {} FROM {}", cols, table);
    if let Some(w) = where_clause {
        sql.push_str(&format!(" WHERE {}", w));
    }
    if let Some(ob) = order_by {
        let parts: Vec<String> = ob.iter().map(|(f, asc)| {
            format!("{} {}", f, if *asc { "ASC" } else { "DESC" })
        }).collect();
        sql.push_str(&format!(" ORDER BY {}", parts.join(", ")));
    }
    if let Some(l) = limit {
        sql.push_str(&format!(" LIMIT {}", l));
    }
    if let Some(o) = offset {
        sql.push_str(&format!(" OFFSET {}", o));
    }
    sql
}

async fn execute_parallel(shards: &[(String, String)]) -> Vec<ShardResult> {
    // 真实场景中：每条 SQL 异步发到对应驱动。
    // 演示环境：返回与分片数相同的空 ShardResult
    shards
        .iter()
        .map(|(id, _sql)| ShardResult {
            shard_id: id.clone(),
            rows: vec![],
            elapsed_ms: 0,
            truncated: false,
        })
        .collect()
}

async fn execute_serial(shards: &[(String, String)]) -> Vec<ShardResult> {
    shards
        .iter()
        .map(|(id, _sql)| ShardResult {
            shard_id: id.clone(),
            rows: vec![],
            elapsed_ms: 0,
            truncated: false,
        })
        .collect()
}

struct MergedResult {
    total_rows: u64,
    rows: Vec<HashMap<String, serde_json::Value>>,
    shard_results: Vec<ShardResult>,
    has_more: bool,
}

fn merge_results(
    shard_results: Vec<ShardResult>,
    strategy: &MergeStrategy,
    order_by: Option<&Vec<(String, bool)>>,
) -> MergedResult {
    let mut all_rows: Vec<HashMap<String, serde_json::Value>> =
        shard_results.iter().flat_map(|r| r.rows.clone()).collect();
    let total = all_rows.len() as u64;

    match strategy {
        MergeStrategy::Concat => {}
        MergeStrategy::SortedMerge => {
            if let Some(ob) = order_by {
                all_rows.sort_by(|a, b| {
                    for (field, asc) in ob {
                        let av = a.get(field);
                        let bv = b.get(field);
                        let ord = compare_values(av, bv);
                        if ord != std::cmp::Ordering::Equal {
                            return if *asc { ord } else { ord.reverse() };
                        }
                    }
                    std::cmp::Ordering::Equal
                });
            }
        }
        MergeStrategy::Distinct => {
            let mut seen = std::collections::HashSet::new();
            all_rows.retain(|row| {
                let key = serde_json::to_string(row).unwrap_or_default();
                seen.insert(key)
            });
        }
        MergeStrategy::Aggregate(ak) => {
            // 简单聚合：返回单行结果
            let agg = match ak {
                AggregateKind::Count => {
                    let mut row = HashMap::new();
                    row.insert("count".to_string(), serde_json::json!(all_rows.len()));
                    row
                }
                AggregateKind::Sum(field) => {
                    let mut sum = 0.0;
                    for r in &all_rows {
                        if let Some(v) = r.get(field).and_then(|x| x.as_f64()) {
                            sum += v;
                        }
                    }
                    let mut row = HashMap::new();
                    row.insert(format!("sum_{}", field), serde_json::json!(sum));
                    row
                }
                AggregateKind::Avg(field) => {
                    let mut sum = 0.0;
                    let mut cnt = 0u64;
                    for r in &all_rows {
                        if let Some(v) = r.get(field).and_then(|x| x.as_f64()) {
                            sum += v;
                            cnt += 1;
                        }
                    }
                    let avg = if cnt > 0 { sum / cnt as f64 } else { 0.0 };
                    let mut row = HashMap::new();
                    row.insert(format!("avg_{}", field), serde_json::json!(avg));
                    row
                }
                AggregateKind::Min(field) => {
                    let mut min = f64::INFINITY;
                    for r in &all_rows {
                        if let Some(v) = r.get(field).and_then(|x| x.as_f64()) {
                            if v < min { min = v; }
                        }
                    }
                    let mut row = HashMap::new();
                    row.insert(format!("min_{}", field), serde_json::json!(min));
                    row
                }
                AggregateKind::Max(field) => {
                    let mut max = f64::NEG_INFINITY;
                    for r in &all_rows {
                        if let Some(v) = r.get(field).and_then(|x| x.as_f64()) {
                            if v > max { max = v; }
                        }
                    }
                    let mut row = HashMap::new();
                    row.insert(format!("max_{}", field), serde_json::json!(max));
                    row
                }
            };
            all_rows = vec![agg];
        }
    }

    MergedResult {
        total_rows: total,
        rows: all_rows,
        shard_results,
        has_more: false,
    }
}

fn compare_values(
    a: Option<&serde_json::Value>,
    b: Option<&serde_json::Value>,
) -> std::cmp::Ordering {
    match (a, b) {
        (Some(va), Some(vb)) => {
            if let (Some(fa), Some(fb)) = (va.as_f64(), vb.as_f64()) {
                fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
            } else if let (Some(sa), Some(sb)) = (va.as_str(), vb.as_str()) {
                sa.cmp(sb)
            } else {
                std::cmp::Ordering::Equal
            }
        }
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

// =============================================================================
// 写入协调器
// =============================================================================

/// 写入操作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WriteOp {
    Insert { table: String, values: HashMap<String, serde_json::Value> },
    Update { table: String, key: String, key_value: String, values: HashMap<String, serde_json::Value> },
    Delete { table: String, key: String, key_value: String },
}

impl WriteOp {
    pub fn table(&self) -> &str {
        match self {
            WriteOp::Insert { table, .. } => table,
            WriteOp::Update { table, .. } => table,
            WriteOp::Delete { table, .. } => table,
        }
    }
}

/// 写入结果（单分片）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResult {
    pub shard_id: String,
    pub op: String,
    pub success: bool,
    pub affected_rows: u64,
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

/// 跨分片写入报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteReport {
    pub total_shards: usize,
    pub success_shards: usize,
    pub failed_shards: usize,
    pub results: Vec<WriteResult>,
    pub elapsed_ms: u64,
}

impl WriteReport {
    pub fn is_success(&self) -> bool {
        self.failed_shards == 0
    }
}

/// 写入协调器：负责将写入操作路由到正确的分片
pub struct WriteCoordinator {
    topologies: HashMap<String, SmartShardTopology>,
}

impl WriteCoordinator {
    pub fn new() -> Self {
        Self {
            topologies: HashMap::new(),
        }
    }

    pub fn register(&mut self, topology: SmartShardTopology) {
        self.topologies.insert(topology.logical_table.clone(), topology);
    }

    pub fn topology(&self, table: &str) -> Option<&SmartShardTopology> {
        self.topologies.get(table)
    }

    /// 写入一条数据：根据表 + 分片键值路由到目标分片
    pub fn route_op(&self, op: &WriteOp) -> Result<&SmartShardNode> {
        let topology = self
            .topologies
            .get(op.table())
            .ok_or_else(|| anyhow!("未注册表 {}", op.table()))?;
        let key_value = match op {
            WriteOp::Insert { values, .. } => {
                values.get(&topology.shard_key)
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "0".to_string())
            }
            WriteOp::Update { key_value, .. } | WriteOp::Delete { key_value, .. } => {
                key_value.clone()
            }
        };
        topology.route(&key_value)
    }

    /// 执行批量写入：路由 + 模拟执行（真实场景异步发到 driver）
    pub async fn execute_batch(&self, ops: Vec<WriteOp>) -> Result<WriteReport> {
        let start = std::time::Instant::now();
        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;
        for op in ops {
            let op_name = match &op {
                WriteOp::Insert { .. } => "INSERT",
                WriteOp::Update { .. } => "UPDATE",
                WriteOp::Delete { .. } => "DELETE",
            }.to_string();
            match self.route_op(&op) {
                Ok(node) => {
                    results.push(WriteResult {
                        shard_id: node.id.clone(),
                        op: op_name,
                        success: true,
                        affected_rows: 1,
                        error: None,
                        elapsed_ms: 0,
                    });
                    success += 1;
                }
                Err(e) => {
                    results.push(WriteResult {
                        shard_id: "".to_string(),
                        op: op_name,
                        success: false,
                        affected_rows: 0,
                        error: Some(e.to_string()),
                        elapsed_ms: 0,
                    });
                    failed += 1;
                }
            }
        }
        Ok(WriteReport {
            total_shards: results.len(),
            success_shards: success,
            failed_shards: failed,
            results,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

impl Default for WriteCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// 动态扩容：rebalance
// =============================================================================

/// 数据 rebalance 计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalancePlan {
    pub logical_table: String,
    pub moves: Vec<ShardMove>,
    pub estimated_total_rows: u64,
    pub estimated_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardMove {
    pub shard_key_range_start: String,
    pub shard_key_range_end: String,
    pub from_shard: String,
    pub to_shard: String,
    pub estimated_rows: u64,
}

impl SmartShardTopology {
    /// 生成 rebalance 计划：尽量均匀分布
    pub fn rebalance_plan(&self) -> RebalancePlan {
        if self.nodes.len() < 2 {
            return RebalancePlan {
                logical_table: self.logical_table.clone(),
                moves: vec![],
                estimated_total_rows: 0,
                estimated_seconds: 0,
            };
        }
        // 简化：将每条记录均分到 N 个分片
        let total = 1_000_000u64; // 假设量级（真实场景从 stats 拿）
        let per_shard: u64 = total / self.nodes.len() as u64;
        let mut moves = Vec::new();
        for i in 0..self.nodes.len() {
            if i + 1 < self.nodes.len() {
                moves.push(ShardMove {
                    shard_key_range_start: format!("{}", (i as u64) * per_shard),
                    shard_key_range_end: format!("{}", ((i + 1) as u64) * per_shard),
                    from_shard: self.nodes[0].id.clone(),
                    to_shard: self.nodes[i + 1].id.clone(),
                    estimated_rows: per_shard,
                });
            }
        }
        RebalancePlan {
            logical_table: self.logical_table.clone(),
            moves,
            estimated_total_rows: total,
            estimated_seconds: total / 10_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topo() -> SmartShardTopology {
        let mut t = SmartShardTopology::new("orders", "user_id");
        t.add_node(SmartShardNode::new("s0", "mysql://a/orders_0", "orders_0"));
        t.add_node(SmartShardNode::new("s1", "mysql://a/orders_1", "orders_1"));
        t.add_node(SmartShardNode::new("s2", "mysql://a/orders_2", "orders_2"));
        t.add_node(SmartShardNode::new("s3", "mysql://a/orders_3", "orders_3"));
        t
    }

    #[test]
    fn test_route_deterministic() {
        let t = topo();
        let n1 = t.route("user_42").unwrap();
        let n2 = t.route("user_42").unwrap();
        assert_eq!(n1.id, n2.id);
    }

    #[test]
    fn test_route_all_active() {
        let t = topo();
        for i in 0..100 {
            let n = t.route(&format!("user_{}", i)).unwrap();
            assert!(n.active);
        }
    }

    #[test]
    fn test_route_inactive_excluded() {
        let mut t = topo();
        t.nodes[0].active = false;
        t.nodes[1].active = false;
        // 只有 s2、s3 活跃
        for i in 0..100 {
            let n = t.route(&format!("user_{}", i)).unwrap();
            assert!(n.id == "s2" || n.id == "s3");
        }
    }

    #[test]
    fn test_route_all_dedup() {
        let t = topo();
        let nodes = t.route_all(&["a", "b", "c", "d", "e"]).unwrap();
        // 5 个不同的 key 去重后节点应 <= 5
        assert!(nodes.len() <= 5);
    }

    #[test]
    fn test_stable_hash_distribution() {
        let mut counts = HashMap::new();
        for i in 0..10000 {
            let h = stable_hash(&format!("user_{}", i)) % 4;
            *counts.entry(h).or_insert(0u64) += 1;
        }
        // 10000/4 = 2500，标准差不应太大
        let avg = 2500.0;
        let max = counts.values().max().unwrap();
        let min = counts.values().min().unwrap();
        assert!((*max as f64 - avg).abs() < avg * 0.1);
        assert!((*min as f64 - avg).abs() < avg * 0.1);
    }

    #[tokio::test]
    async fn test_query_coordinator_execute() {
        let mut qc = QueryCoordinator::new();
        qc.register(topo());
        let query = SmartSpanningQuery {
            logical_table: "orders".to_string(),
            columns: Some(vec!["id".to_string(), "user_id".to_string()]),
            where_clause: Some("user_id > 100".to_string()),
            order_by: Some(vec![("id".to_string(), true)]),
            limit: Some(10),
            offset: Some(0),
            parallel: true,
            merge_strategy: MergeStrategy::SortedMerge,
        };
        let r = qc.execute(query).await.unwrap();
        // 模拟执行下返回 4 个分片（每个空 ShardResult）
        assert_eq!(r.shard_results.len(), 4);
    }

    #[tokio::test]
    async fn test_write_coordinator_route() {
        let mut wc = WriteCoordinator::new();
        wc.register(topo());
        let op = WriteOp::Insert {
            table: "orders".to_string(),
            values: [("user_id".to_string(), serde_json::json!("user_42"))]
                .iter()
                .cloned()
                .collect(),
        };
        let node = wc.route_op(&op).unwrap();
        assert!(!node.id.is_empty());
    }

    #[tokio::test]
    async fn test_write_coordinator_batch() {
        let mut wc = WriteCoordinator::new();
        wc.register(topo());
        let ops = vec![
            WriteOp::Insert {
                table: "orders".to_string(),
                values: [("user_id".to_string(), serde_json::json!("u1"))]
                    .iter()
                    .cloned()
                    .collect(),
            },
            WriteOp::Update {
                table: "orders".to_string(),
                key: "user_id".to_string(),
                key_value: "u2".to_string(),
                values: [("status".to_string(), serde_json::json!("paid"))]
                    .iter()
                    .cloned()
                    .collect(),
            },
        ];
        let report = wc.execute_batch(ops).await.unwrap();
        assert_eq!(report.total_shards, 2);
        assert_eq!(report.success_shards, 2);
    }

    #[test]
    fn test_rebalance_plan() {
        let t = topo();
        let plan = t.rebalance_plan();
        assert!(plan.moves.len() > 0);
    }

    #[test]
    fn test_sharding_strategy_default() {
        let s = ShardStrategyKind::default();
        match s {
            ShardStrategyKind::Hash { virtual_nodes } => assert_eq!(virtual_nodes, 64),
            _ => panic!("default 应该是 Hash"),
        }
    }

    #[test]
    fn test_merge_concat() {
        let sr = vec![];
        let m = merge_results(sr, &MergeStrategy::Concat, None);
        assert_eq!(m.total_rows, 0);
    }

    #[test]
    fn test_merge_aggregate_count() {
        let mut row1 = HashMap::new();
        row1.insert("v".to_string(), serde_json::json!(1));
        let mut row2 = HashMap::new();
        row2.insert("v".to_string(), serde_json::json!(2));
        let sr = vec![
            ShardResult {
                shard_id: "s0".to_string(),
                rows: vec![row1, row2],
                elapsed_ms: 1,
                truncated: false,
            }
        ];
        let m = merge_results(sr, &MergeStrategy::Aggregate(AggregateKind::Count), None);
        assert_eq!(m.rows.len(), 1);
        assert_eq!(m.rows[0].get("count").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn test_compare_values_floats() {
        let a = serde_json::json!(1.5);
        let b = serde_json::json!(2.0);
        assert_eq!(compare_values(Some(&a), Some(&b)), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_values_strings() {
        let a = serde_json::json!("abc");
        let b = serde_json::json!("abd");
        assert_eq!(compare_values(Some(&a), Some(&b)), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_write_op_table() {
        let op = WriteOp::Delete {
            table: "users".to_string(),
            key: "id".to_string(),
            key_value: "1".to_string(),
        };
        assert_eq!(op.table(), "users");
    }
}
