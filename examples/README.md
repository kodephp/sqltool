# SQLTool 多语言调用示例

本目录包含 SQLTool 在各种编程语言中的调用示例。

## 安装 SQLTool

```bash
# 方式1: cargo install (推荐)
cargo install sqltool

# 方式2: GitHub 下载
curl -L https://github.com/kodephp/sqltool/releases/latest/download/sqltool-macos.tar.gz | tar xz
```

## 快速开始

### 方式1: HTTP API 模式 (需要先启动服务)

```bash
# 启动 SQLTool HTTP 服务
sqltool server -p 8080 -s mysql://localhost/mydb
```

然后运行各语言的示例：

```bash
# Python
pip install requests
python python/sqltool_demo.py

# Node.js
npm install axios
node node/sqltool_demo.js

# Go
go run go/sqltool_demo.go

# PHP
php php/sqltool_demo.php

# Ruby
ruby ruby/sqltool_demo.rb

# Java
javac java/SqlToolDemo.java
java SqlToolDemo

# C#
dotnet new console -o cs
cp cs/SqlToolDemo.cs cs/Program.cs
dotnet run
```

### 方式2: CLI 模式 (不需要启动服务)

所有语言都支持直接调用 CLI：

```bash
# Python
python python/sqltool_demo.py --cli

# Node.js
node node/sqltool_demo.js --cli

# Go
go run go/sqltool_demo.go --cli

# PHP
php php/sqltool_demo.php --cli

# Ruby
ruby ruby/sqltool_demo.rb --cli

# Java
javac java/SqlToolDemo.java
java SqlToolDemo --cli

# C#
dotnet run -- --cli
```

## 示例功能

每个示例都包含以下功能演示：

| 功能 | 说明 |
|------|------|
| 健康检查 | `/api/health` - 检查服务状态 |
| SQL注入检测 | 检测恶意 SQL 输入 |
| 构建安全SQL | 防止 SQL 注入的参数化构建 |

## 目录结构

```
examples/
├── README.md              # 本文件
├── cli/                  # Shell CLI 示例
│   └── all_examples.sh   # 所有 CLI 命令示例
├── rust/                 # Rust 库调用
│   ├── Cargo.toml
│   └── src/main.rs
├── python/               # Python 调用
│   └── sqltool_demo.py
├── node/                 # Node.js 调用
│   └── sqltool_demo.js
├── go/                   # Go 调用
│   └── sqltool_demo.go
├── php/                  # PHP 调用
│   └── sqltool_demo.php
├── ruby/                 # Ruby 调用
│   └── sqltool_demo.rb
├── java/                 # Java 调用
│   └── SqlToolDemo.java
└── cs/                   # C# 调用
    └── SqlToolDemo.cs
```

## Rust 库调用 (示例)

如果你使用 Rust，可以直接作为库依赖：

```toml
# Cargo.toml
[dependencies]
sqltool = "0.3"
tokio = { version = "1", features = ["full"] }
```

```rust
use sqltool::{create_connection, DatabaseType, DataTransfer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = create_connection(DatabaseType::SQLite, "sqlite://:memory:").await?;
    let target = create_connection(DatabaseType::SQLite, "sqlite://:memory:").await?;

    let transfer = DataTransfer::new(source, target);
    let mappings = transfer.generate_auto_mappings("source_table", "target_table").await?;
    let report = transfer.transfer(mappings).await?;

    println!("迁移完成: {} 行", report.rows_transferred);
    Ok(())
}
```

## CLI 常用命令

```bash
# SQL注入检测
sqltool detect-injection --input "' OR '1'='1"

# 构建安全SQL
sqltool build-safe-sql --table users --field name --operator = --value "test"

# 数据迁移
sqltool transfer -s mysql://localhost/source -t postgresql://localhost/target

# 数据库备份
sqltool backup -s mysql://localhost/db --output ./backup.sql

# 数据对比
sqltool compare-data -s db1 -t db2 --table users --primary-key id

# 创建分片
sqltool create-shard -s mysql://localhost/db --table orders --strategy row_count

# 启动HTTP服务
sqltool server -p 8080 -s mysql://localhost/db
```

## 完整文档

- API 文档: https://docs.rs/sqltool
- crates.io: https://crates.io/crates/sqltool
- GitHub: https://github.com/kodephp/sqltool
