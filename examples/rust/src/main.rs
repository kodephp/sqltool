//! SQLTool Rust SDK 演示程序
//!
//! 演示内容：
//!   1. 跨数据库迁移（异构 + 跨版本 + 字段自动连线）
//!   2. 智能分库分表（查询合并 + 写入协调 + 动态扩容）
//!   3. 自定义类型映射规则
//!
//! 依赖：
//! ```toml
//! [dependencies]
//! sqltool = "0.6"
//! tokio = { version = "1", features = ["full"] }
//! ```

use sqltool::databases::DatabaseVersion;
use sqltool::models::{Field, TableSchema};
use sqltool::{
    CrossDbConverter, DataMigrator, MigrationConfig,
    ShardTopology, ShardNode, ShardStrategyKind,
    QueryCoordinator, WriteCoordinator, SpanningQuery, MergeStrategy,
    ShardWriteOp, ShardResult, TargetDbKind, TypeMappingRule,
};
use std::collections::HashMap;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "=".repeat(70));
    println!("SQLTool Rust SDK 演示 v{}", sqltool::VERSION);
    println!("{}", "=".repeat(70));

    // 演示 1: 跨数据库迁移（异构 + 跨版本）
    demo_cross_db_migration()?;

    // 演示 2: 智能分库分表
    demo_smart_sharding().await?;

    // 演示 3: 自定义类型映射规则
    demo_custom_type_rule()?;

    println!("\n{}", "=".repeat(70));
    println!("✓ 演示完成");
    println!("{}", "=".repeat(70));
    Ok(())
}

/// 演示 1: 跨数据库迁移
fn demo_cross_db_migration() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[1] 跨数据库迁移 (MySQL 5.7 → PostgreSQL 16)");
    println!("{}", "-".repeat(70));

    // 构造源表结构
    let source_table = TableSchema {
        name: "orders".to_string(),
        fields: vec![
            Field { name: "id".into(), data_type: "INT".into(), length: None, nullable: false, default_value: None, primary_key: true, auto_increment: true },
            Field { name: "user_id".into(), data_type: "BIGINT".into(), length: None, nullable: false, default_value: None, primary_key: false, auto_increment: false },
            Field { name: "amount".into(), data_type: "DECIMAL".into(), length: Some(10), nullable: false, default_value: None, primary_key: false, auto_increment: false },
            Field { name: "status".into(), data_type: "VARCHAR".into(), length: Some(32), nullable: false, default_value: None, primary_key: false, auto_increment: false },
            Field { name: "created_at".into(), data_type: "DATETIME".into(), length: None, nullable: false, default_value: None, primary_key: false, auto_increment: false },
            Field { name: "updated_at".into(), data_type: "TIMESTAMP".into(), length: None, nullable: false, default_value: None, primary_key: false, auto_increment: false },
            Field { name: "remark".into(), data_type: "TEXT".into(), length: None, nullable: true, default_value: None, primary_key: false, auto_increment: false },
        ],
        indexes: vec![],
        foreign_keys: vec![],
    };

    // 字段重命名映射：remark → comment
    let mut manual_field_map = HashMap::new();
    manual_field_map.insert("remark".to_string(), "comment".to_string());

    // 配置迁移
    let config = MigrationConfig {
        source_db: TargetDbKind::MySQL,
        target_db: TargetDbKind::PostgreSQL,
        source_version: Some(DatabaseVersion::new(5, 7, 40)),
        target_version: Some(DatabaseVersion::new(16, 2, 0)),
        auto_field_link: true,
        manual_field_map,
        batch_size: 5000,
        enable_version_upgrade: true,
        pre_check: true,
        default_source_version: None,
        default_target_version: None,
    };

    let migrator = DataMigrator::new();
    let result = migrator.migrate_table(&source_table, &config)?;

    println!("  表名: {}", result.table_name);
    println!("  方向: {} ({:?})", result.direction.name(), result.direction);
    println!("  源库: {:?} ({})", result.source_db, result.source_version);
    println!("  目标库: {:?} ({})", result.target_db, result.target_version);
    println!("  字段映射: {}/{} (有损 {} 个)", result.fields_mapped, result.fields_total, result.lossy_conversions);
    println!("  成功率: {:.1}%", result.success_rate() * 100.0);
    println!("  耗时: {}ms", result.elapsed_ms);
    println!("\n  生成的 DDL:");
    println!("{}", result.ddl);

    println!("\n  字段映射详情:");
    for fm in &result.field_migrations {
        let flag = if fm.lossy { "⚠️" } else { "  " };
        println!("    {} {} ({}) → {} ({}) [transform:{}]",
            flag, fm.source_field, fm.source_type, fm.target_field, fm.target_type, fm.transform);
    }

    Ok(())
}

/// 演示 2: 智能分库分表
async fn demo_smart_sharding() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[2] 智能分库分表 (4 分片哈希)");
    println!("{}", "-".repeat(70));

    // 构造分片拓扑
    let topology = ShardTopology {
        logical_table: "orders".to_string(),
        shard_key: "user_id".to_string(),
        strategy: ShardStrategyKind::Hash { virtual_nodes: 64 },
        nodes: vec![
            ShardNode { id: "s0".into(), connection: "mysql://n1/orders_0".into(), table: "orders_0".into(), weight: 100, active: true },
            ShardNode { id: "s1".into(), connection: "mysql://n1/orders_1".into(), table: "orders_1".into(), weight: 100, active: true },
            ShardNode { id: "s2".into(), connection: "mysql://n2/orders_2".into(), table: "orders_2".into(), weight: 100, active: true },
            ShardNode { id: "s3".into(), connection: "mysql://n2/orders_3".into(), table: "orders_3".into(), weight: 100, active: true },
        ],
        created_at: 0,
        updated_at: 0,
    };

    // 路由演示
    let mut write_coord = WriteCoordinator::new();
    write_coord.register(topology.clone());

    println!("\n  路由演示（相同 key 路由到固定分片）:");
    for uid in &["user_001", "user_042", "user_999", "user_001"] {
        let mut values = HashMap::new();
        values.insert("user_id".to_string(), serde_json::Value::String(uid.to_string()));
        let op = ShardWriteOp::Insert {
            table: "orders".to_string(),
            values,
        };
        let node = write_coord.route_op(&op)?;
        println!("    {} → 分片 {} (表 {})", uid, node.id, node.table);
    }

    // 跨分片查询
    let mut query_coord = QueryCoordinator::new();
    query_coord.register(topology.clone());

    let query = SpanningQuery {
        logical_table: "orders".into(),
        columns: Some(vec!["id".into(), "user_id".into(), "amount".into()]),
        where_clause: Some("amount > 100".into()),
        order_by: Some(vec![("id".into(), true)]),
        limit: Some(10),
        offset: None,
        merge_strategy: MergeStrategy::SortedMerge,
        parallel: true,
    };

    println!("\n  跨分片查询演示:");
    let result = query_coord.execute(query).await?;
    println!("    涉及分片数: {}", result.shard_results.len());
    println!("    总行数: {}", result.total_rows);
    println!("    合并耗时: {}ms", result.merged_in_ms);
    for sr in &result.shard_results {
        let _ = sr; // suppress unused
    }

    // 批量写入演示
    println!("\n  批量写入演示:");
    let mut ops: Vec<ShardWriteOp> = Vec::new();
    for (i, uid) in ["user_001", "user_042", "user_999"].iter().enumerate() {
        let mut values = HashMap::new();
        values.insert("user_id".to_string(), serde_json::Value::String(uid.to_string()));
        values.insert("amount".to_string(), serde_json::Value::Number((100 * (i as i64 + 1)).into()));
        ops.push(ShardWriteOp::Insert {
            table: "orders".to_string(),
            values,
        });
    }
    let write_report = write_coord.execute_batch(ops).await?;
    println!("    写入报告: 成功 {}/{} 分片", write_report.success_shards, write_report.total_shards);

    println!("\n  Rebalance 计划（扩容演示）:");
    let plan = topology.rebalance_plan();
    println!("    逻辑表: {}", plan.logical_table);
    println!("    预计迁移行数: {}", plan.estimated_total_rows);
    println!("    预计耗时: {}s", plan.estimated_seconds);
    println!("    移动步骤: {}", plan.moves.len());
    for m in &plan.moves {
        println!("    移动 {} → {}: ~{} 行 (range {}-{})",
            m.from_shard, m.to_shard, m.estimated_rows, m.shard_key_range_start, m.shard_key_range_end);
    }

    // 显式使用未引用的类型
    let _ = ShardResult { shard_id: "".into(), rows: vec![], elapsed_ms: 0, truncated: false };

    Ok(())
}

/// 演示 3: 自定义类型映射规则
fn demo_custom_type_rule() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[3] 自定义类型映射规则");
    println!("{}", "-".repeat(70));

    let mut converter = CrossDbConverter::new();

    // 自定义规则：MySQL ENUM → PostgreSQL VARCHAR
    converter.add_rule(TypeMappingRule {
        source_db: TargetDbKind::MySQL,
        source_type_pattern: "ENUM".to_string(),
        target_db: TargetDbKind::PostgreSQL,
        target_type: "VARCHAR(64)".to_string(),
        note: "自定义：ENUM → VARCHAR".to_string(),
        lossy: true,
    });

    println!("  ✓ 添加自定义规则: MySQL ENUM → PostgreSQL VARCHAR(64)");
    println!("  ✓ 规则集已就绪");

    println!("\n  内置规则覆盖:");
    println!("    - MySQL 5.5/5.7/8.0 ↔ PostgreSQL 9.x/12/14/16");
    println!("    - MySQL ↔ SQLite (3.20 - 3.45)");
    println!("    - MySQL ↔ Oracle (11g/12c/19c/21c)");
    println!("    - MySQL ↔ MS SQL Server (2016+)");
    println!("    - TiDB / MariaDB ↔ MySQL (类型/语法兼容)");
    println!("    - 200+ 字段类型映射规则");

    Ok(())
}
