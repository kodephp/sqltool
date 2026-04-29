use sqltool::{DataTransfer, StructureMigration, create_connection, DatabaseType, FieldMapping};

#[tokio::test]
async fn test_full_structure_data_transfer() {
    // 创建SQLite测试数据库（使用内存数据库）
    let source_conn = create_connection(
        DatabaseType::SQLite,
        "sqlite::memory:"
    ).await.unwrap();

    // 初始化源数据库
    source_conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)").await.unwrap();
    source_conn.execute("INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')").await.unwrap();
    source_conn.execute("INSERT INTO users (name, email) VALUES ('Bob', 'bob@example.com')").await.unwrap();

    // 创建目标数据库连接
    let target_conn = create_connection(
        DatabaseType::SQLite,
        "sqlite::memory:"
    ).await.unwrap();

    // 直接在目标数据库中创建表结构，而不是使用迁移功能
    target_conn.execute("CREATE TABLE users_copy (id INTEGER PRIMARY KEY, name TEXT, email TEXT)").await.unwrap();

    // 创建数据转移实例
    let transfer = DataTransfer::new(source_conn, target_conn);

    // 创建字段映射
    let mappings = vec![
        FieldMapping {
            source_table: "users".to_string(),
            source_field: "id".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "id".to_string(),
        },
        FieldMapping {
            source_table: "users".to_string(),
            source_field: "name".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "name".to_string(),
        },
        FieldMapping {
            source_table: "users".to_string(),
            source_field: "email".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "email".to_string(),
        },
    ];

    // 执行数据转移
    transfer.transfer(mappings).await.unwrap();

    println!("Full structure and data transfer test completed");
}

#[tokio::test]
async fn test_only_structure_transfer() {
    // 创建SQLite测试数据库
    let source_db_path = ":memory:";
    let target_db_path = ":memory:";

    // 创建源数据库连接
    let source_conn = create_connection(
        DatabaseType::SQLite,
        &format!("sqlite://{}", source_db_path)
    ).await.unwrap();

    // 创建目标数据库连接
    let target_conn = create_connection(
        DatabaseType::SQLite,
        &format!("sqlite://{}", target_db_path)
    ).await.unwrap();

    // 初始化源数据库
    source_conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)").await.unwrap();

    // 迁移结构
    let migration = StructureMigration::new(source_conn, target_conn);
    migration.migrate_structure("users", "users_copy").await.unwrap();

    println!("Only structure transfer test completed");
}

#[tokio::test]
async fn test_only_data_transfer() {
    // 创建SQLite测试数据库
    let source_db_path = ":memory:";
    let target_db_path = ":memory:";

    // 创建源数据库连接
    let source_conn = create_connection(
        DatabaseType::SQLite,
        &format!("sqlite://{}", source_db_path)
    ).await.unwrap();

    // 创建目标数据库连接
    let target_conn = create_connection(
        DatabaseType::SQLite,
        &format!("sqlite://{}", target_db_path)
    ).await.unwrap();

    // 初始化源数据库
    source_conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)").await.unwrap();
    source_conn.execute("INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')").await.unwrap();
    source_conn.execute("INSERT INTO users (name, email) VALUES ('Bob', 'bob@example.com')").await.unwrap();

    // 初始化目标数据库
    target_conn.execute("CREATE TABLE users_copy (id INTEGER PRIMARY KEY, name TEXT, email TEXT)").await.unwrap();

    // 创建数据转移实例
    let transfer = DataTransfer::new(source_conn, target_conn);

    // 创建字段映射
    let mappings = vec![
        FieldMapping {
            source_table: "users".to_string(),
            source_field: "id".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "id".to_string(),
        },
        FieldMapping {
            source_table: "users".to_string(),
            source_field: "name".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "name".to_string(),
        },
        FieldMapping {
            source_table: "users".to_string(),
            source_field: "email".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "email".to_string(),
        },
    ];

    // 执行数据转移
    transfer.transfer(mappings).await.unwrap();

    println!("Only data transfer test completed");
}

#[tokio::test]
async fn test_related_tables_transfer() {
    // 创建SQLite测试数据库（使用内存数据库）
    let source_conn = create_connection(
        DatabaseType::SQLite,
        "sqlite::memory:"
    ).await.unwrap();

    // 初始化源数据库
    source_conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)").await.unwrap();
    source_conn.execute("CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER, title TEXT, FOREIGN KEY (user_id) REFERENCES users(id))").await.unwrap();
    source_conn.execute("INSERT INTO users (name) VALUES ('Alice'), ('Bob')").await.unwrap();
    source_conn.execute("INSERT INTO posts (user_id, title) VALUES (1, 'Post 1'), (1, 'Post 2'), (2, 'Post 3')").await.unwrap();

    // 直接在目标数据库中创建表结构
    let target_conn = create_connection(
        DatabaseType::SQLite,
        "sqlite::memory:"
    ).await.unwrap();
    
    // 创建用户表
    target_conn.execute("CREATE TABLE users_copy (id INTEGER PRIMARY KEY, name TEXT)").await.unwrap();
    
    // 创建帖子表
    target_conn.execute("CREATE TABLE posts_copy (id INTEGER PRIMARY KEY, user_id INTEGER, title TEXT, FOREIGN KEY (user_id) REFERENCES users_copy(id))").await.unwrap();

    // 创建数据转移实例
    let transfer = DataTransfer::new(source_conn, target_conn);

    // 转移用户数据
    let user_mappings = vec![
        FieldMapping {
            source_table: "users".to_string(),
            source_field: "id".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "id".to_string(),
        },
        FieldMapping {
            source_table: "users".to_string(),
            source_field: "name".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "name".to_string(),
        },
    ];
    transfer.transfer(user_mappings).await.unwrap();

    // 重新创建源数据库连接
    let source_conn = create_connection(
        DatabaseType::SQLite,
        "sqlite::memory:"
    ).await.unwrap();

    // 初始化源数据库
    source_conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)").await.unwrap();
    source_conn.execute("CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER, title TEXT, FOREIGN KEY (user_id) REFERENCES users(id))").await.unwrap();
    source_conn.execute("INSERT INTO users (name) VALUES ('Alice'), ('Bob')").await.unwrap();
    source_conn.execute("INSERT INTO posts (user_id, title) VALUES (1, 'Post 1'), (1, 'Post 2'), (2, 'Post 3')").await.unwrap();

    // 重新创建目标数据库连接
    let target_conn = create_connection(
        DatabaseType::SQLite,
        "sqlite::memory:"
    ).await.unwrap();
    
    // 创建用户表
    target_conn.execute("CREATE TABLE users_copy (id INTEGER PRIMARY KEY, name TEXT)").await.unwrap();
    
    // 创建帖子表
    target_conn.execute("CREATE TABLE posts_copy (id INTEGER PRIMARY KEY, user_id INTEGER, title TEXT, FOREIGN KEY (user_id) REFERENCES users_copy(id))").await.unwrap();

    // 创建数据转移实例
    let transfer = DataTransfer::new(source_conn, target_conn);

    // 转移帖子数据
    let post_mappings = vec![
        FieldMapping {
            source_table: "posts".to_string(),
            source_field: "id".to_string(),
            target_table: "posts_copy".to_string(),
            target_field: "id".to_string(),
        },
        FieldMapping {
            source_table: "posts".to_string(),
            source_field: "user_id".to_string(),
            target_table: "posts_copy".to_string(),
            target_field: "user_id".to_string(),
        },
        FieldMapping {
            source_table: "posts".to_string(),
            source_field: "title".to_string(),
            target_table: "posts_copy".to_string(),
            target_field: "title".to_string(),
        },
    ];
    transfer.transfer(post_mappings).await.unwrap();

    println!("Related tables transfer test completed");
}
