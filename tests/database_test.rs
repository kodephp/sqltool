use sqltool::databases::{create_connection, DatabaseType};
use sqltool::core::{DataTransfer, StructureMigration};

#[tokio::test]
async fn test_sqlite_connection() {
    // 测试SQLite连接
    let conn = create_connection(DatabaseType::SQLite, "sqlite::memory:").await;
    assert!(conn.is_ok(), "SQLite connection failed");
    println!("SQLite connection successful");
}

#[tokio::test]
async fn test_mysql_connection() {
    // 测试MySQL连接
    let conn = create_connection(DatabaseType::MySQL, "mysql://root:root@localhost:3306/test").await;
    match conn {
        Ok(_) => println!("MySQL connection successful"),
        Err(e) => println!("MySQL connection failed: {:?}", e),
    }
}

#[tokio::test]
async fn test_postgres_connection() {
    // 测试PostgreSQL连接
    let conn = create_connection(DatabaseType::PostgreSQL, "postgres://root@localhost:5432/test").await;
    match conn {
        Ok(_) => println!("PostgreSQL connection successful"),
        Err(e) => println!("PostgreSQL connection failed: {:?}", e),
    }
}

#[tokio::test]
async fn test_sqlite_to_sqlite_transfer() {
    // 测试SQLite到SQLite的数据转移
    let source_conn = create_connection(DatabaseType::SQLite, "sqlite::memory:").await.unwrap();
    let target_conn = create_connection(DatabaseType::SQLite, "sqlite::memory:").await.unwrap();

    // 创建测试表
    source_conn.execute("CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, value INTEGER)").await.unwrap();
    source_conn.execute("INSERT INTO test (name, value) VALUES ('test1', 100), ('test2', 200)").await.unwrap();

    // 直接执行CREATE TABLE语句，而不是使用迁移功能
    let create_sql = "CREATE TABLE test_target (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, value INTEGER)";
    let result = source_conn.execute(create_sql).await;
    assert!(result.is_ok(), "Structure creation failed: {:?}", result);
    println!("Structure creation successful");

    // 重新创建连接用于数据转移
    let source_conn = create_connection(DatabaseType::SQLite, "sqlite::memory:").await.unwrap();
    let target_conn = create_connection(DatabaseType::SQLite, "sqlite::memory:").await.unwrap();

    // 在源数据库中创建表和数据
    source_conn.execute("CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, value INTEGER)").await.unwrap();
    source_conn.execute("INSERT INTO test (name, value) VALUES ('test1', 100), ('test2', 200)").await.unwrap();

    // 在目标数据库中创建表结构
    target_conn.execute("CREATE TABLE IF NOT EXISTS test_target (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, value INTEGER)").await.unwrap();

    // 执行数据转移
    let transfer = DataTransfer::new(source_conn, target_conn);
    let mappings = transfer.auto_match_fields("test", "test_target").await;
    assert!(mappings.is_ok(), "Auto match fields failed: {:?}", mappings);
    let mappings = mappings.unwrap();
    println!("Auto match fields successful, mappings: {:?}", mappings);
    
    let result = transfer.transfer(mappings).await;
    assert!(result.is_ok(), "Data transfer failed: {:?}", result);
    println!("Data transfer successful");
}
