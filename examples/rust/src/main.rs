//! SQLTool Rust 库使用示例
//!
//! Cargo.toml 添加依赖:
//! ```toml
//! [dependencies]
//! sqltool = "0.3"
//! tokio = { version = "1", features = ["full"] }
//! ```

use sqltool::{
    create_connection, DatabaseType,
    DatabaseBackup, BackupConfig, BackupType,
    DataTransfer, TransferConfig,
    DataComparer, DataCompareConfig, CompareMode,
    SqlInjectionDetector, RiskLevel,
    SafeSqlBuilder,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SQLTool Rust 示例 ===\n");

    // 示例1: 数据库备份
    backup_example().await?;

    // 示例2: 数据迁移
    transfer_example().await?;

    // 示例3: 数据对比
    compare_example().await?;

    // 示例4: SQL注入检测
    injection_example().await?;

    // 示例5: 构建安全SQL
    safe_sql_example().await?;

    println!("\n=== 所有示例执行完成 ===");
    Ok(())
}

/// 数据库备份示例
async fn backup_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. 数据库备份示例");
    println!("-------------------");

    // 连接到 SQLite 内存数据库（测试用）
    let conn = create_connection(
        DatabaseType::SQLite,
        "sqlite://:memory:"
    ).await?;

    // 创建测试表
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)"
    ).await?;
    conn.execute(
        "INSERT INTO users (name, email) VALUES ('张三', 'zhang@example.com')"
    ).await?;
    conn.execute(
        "INSERT INTO users (name, email) VALUES ('李四', 'li@example.com')"
    ).await?;

    println!("✓ 创建测试表并插入2条数据");

    // 配置备份参数
    let config = BackupConfig {
        backup_type: BackupType::Full,
        backup_path: "./backup_example.sql".to_string(),
        compress: false,
        encrypt: false,
        encryption_key: None,
        parallel_tables: 1,
        include_stored_procedures: false,
        include_functions: false,
        include_triggers: false,
        include_views: false,
        include_events: false,
        database_name: "example_db".to_string(),
    };

    // 执行备份
    let mut backup = DatabaseBackup::new(conn, config);
    match backup.execute_backup("example_backup").await {
        Ok(report) => {
            println!("✓ 备份成功!");
            println!("  - 备份路径: ./backup_example.sql");
            println!("  - 大小: {} 字节", report.size_bytes);
        }
        Err(e) => {
            println!("⚠ 备份跳过 (可能需要真实数据库): {}", e);
        }
    }

    // 清理
    let _ = std::fs::remove_file("./backup_example.sql");

    Ok(())
}

/// 数据迁移示例
async fn transfer_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n2. 数据迁移示例");
    println!("-------------------");

    // 源数据库
    let source = create_connection(
        DatabaseType::SQLite,
        "sqlite://:memory:"
    ).await?;

    // 目标数据库
    let target = create_connection(
        DatabaseType::SQLite,
        "sqlite://:memory:"
    ).await?;

    // 创建源表并插入数据
    source.execute(
        "CREATE TABLE source_users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)"
    ).await?;
    source.execute(
        "INSERT INTO source_users (name, age) VALUES ('Alice', 25)"
    ).await?;
    source.execute(
        "INSERT INTO source_users (name, age) VALUES ('Bob', 30)"
    ).await?;

    // 创建目标表
    target.execute(
        "CREATE TABLE target_users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)"
    ).await?;

    println!("✓ 源表插入2条数据");

    // 创建迁移配置
    let config = TransferConfig {
        source_tables: vec!["source_users".to_string()],
        batch_size: 100,
        verify_data: true,
        skip_errors: true,
        max_errors: 10,
        show_progress: false,
    };

    // 执行迁移
    let transfer = DataTransfer::new(source, target, config);

    // 自动生成字段映射
    let mappings = transfer.generate_auto_mappings("source_users", "target_users").await?;

    println!("✓ 生成字段映射: {:?}", mappings.len());

    // 执行迁移
    let report = transfer.transfer(mappings).await?;

    if report.success {
        println!("✓ 迁移成功!");
        println!("  - 迁移行数: {}", report.rows_transferred);
        println!("  - 成功率: {:.1}%", report.success_rate());
    } else {
        println!("⚠ 迁移有错误: {:?}", report.errors);
    }

    Ok(())
}

/// 数据对比示例
async fn compare_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n3. 数据对比示例");
    println!("-------------------");

    let conn1 = create_connection(DatabaseType::SQLite, "sqlite://:memory:").await?;
    let conn2 = create_connection(DatabaseType::SQLite, "sqlite://:memory:").await?;

    // 创建相同的表和数据
    conn1.execute(
        "CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price REAL)"
    ).await?;
    conn2.execute(
        "CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price REAL)"
    ).await?;

    // 插入相同数据
    for (name, price) in &[("Apple", 5.99), ("Banana", 3.99), ("Cherry", 7.99)] {
        conn1.execute(&format!(
            "INSERT INTO products (name, price) VALUES ('{}', {})", name, price
        )).await?;
        conn2.execute(&format!(
            "INSERT INTO products (name, price) VALUES ('{}', {})", name, price
        )).await?;
    }

    println!("✓ 两个数据库各有3条相同数据");

    // 配置对比
    let config = DataCompareConfig {
        compare_mode: CompareMode::Full,
        primary_key: "id".to_string(),
        ignore_fields: None,
        sample_rate: 1.0,
    };

    // 执行对比
    let comparer = DataComparer::new(conn1, conn2, config);
    let result = comparer.compare_table("products").await?;

    println!("✓ 对比结果:");
    println!("  - 总行数: {}", result.stats.total_rows);
    println!("  - 匹配行数: {}", result.stats.matched_rows);
    println!("  - 差异行数: {}", result.stats.different_rows);
    println!("  - 匹配率: {:.1}%", result.stats.match_percentage);

    Ok(())
}

/// SQL注入检测示例
async fn injection_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n4. SQL注入检测示例");
    println!("-------------------");

    let test_cases = vec![
        ("' OR '1'='1", "经典注入"),
        ("'; DROP TABLE users; --", "删除表注入"),
        ("1; DELETE FROM users WHERE 1=1", "删除数据注入"),
        ("' UNION SELECT * FROM passwords --", "联合查询注入"),
        ("<script>alert('xss')</script>", "XSS注入"),
        ("Normal input", "正常输入"),
    ];

    let detector = SqlInjectionDetector::new();

    for (input, desc) in test_cases {
        let report = detector.detect(input);

        let risk_icon = match report.risk_level {
            RiskLevel::High => "🔴",
            RiskLevel::Medium => "🟡",
            RiskLevel::Low => "🟢",
            RiskLevel::None => "⚪",
        };

        println!("  {} {} - {:?}", risk_icon, desc, report.risk_level);
        if !report.findings.is_empty() {
            for finding in &report.findings {
                println!("    - {}", finding);
            }
        }
    }

    Ok(())
}

/// 构建安全SQL示例
async fn safe_sql_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n5. 构建安全SQL示例");
    println!("-------------------");

    let test_cases = vec![
        ("users", "name", "=", "Zhangsan"),
        ("users", "email", "LIKE", "test%"),
        ("orders", "status", "=", "pending"),
        ("products", "price", ">", "100"),
    ];

    for (table, field, op, value) in test_cases {
        let safe_sql = SafeSqlBuilder::new()
            .table(table)
            .field(field)
            .operator(op)
            .value(value)
            .build();

        println!("  {} {} {} '{}'", table, field, op, value);
        println!("    -> {}", safe_sql);
    }

    Ok(())
}
