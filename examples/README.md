# SQLTool 多语言调用示例

本目录包含使用 SQLTool 的多种编程语言调用示例。

## 目录结构

```
examples/
├── README.md           # 本文档
├── cli/                # Shell CLI 示例
│   └── all_examples.sh
├── go/                 # Go 语言示例
│   └── sqltool_demo.go
├── java/               # Java 语言示例
│   └── SqlToolDemo.java
├── node/               # Node.js 示例
│   └── sqltool_demo.js
├── php/                # PHP 示例
│   └── sqltool_demo.php
├── python/              # Python 示例
│   └── sqltool_demo.py
├── ruby/               # Ruby 示例
│   └── sqltool_demo.rb
└── cs/                 # C# 示例
    └── SqlToolDemo.cs
```

## 快速开始

### 1. 构建 SQLTool

```bash
cargo build --release
```

### 2. CLI 模式 (所有语言通用)

```bash
# SQL 注入检测
./target/release/sqltool detect-sql-injection --input "' OR '1'='1"

# 构建安全 SQL
./target/release/sqltool build-safe-sql --table users --field name --value "test"

# 健康检查
./target/release/sqltool health
```

### 3. HTTP API 模式

```bash
# 启动服务器
./target/release/sqltool server -p 8080 -s mysql://localhost/mydb
```

## 各语言示例

### Go

```bash
cd examples/go
go run sqltool_demo.go           # HTTP API 模式
go run sqltool_demo.go --cli     # CLI 模式
```

### Python

```bash
cd examples/python
python3 sqltool_demo.py           # HTTP API 模式
python3 sqltool_demo.py --cli    # CLI 模式
```

### Node.js

```bash
cd examples/node
node sqltool_demo.js             # HTTP API 模式
node sqltool_demo.js --cli       # CLI 模式
```

### PHP

```bash
cd examples/php
php sqltool_demo.php             # HTTP API 模式
php sqltool_demo.php --cli       # CLI 模式
```

### Ruby

```bash
cd examples/ruby
ruby sqltool_demo.rb             # HTTP API 模式
ruby sqltool_demo.rb --cli       # CLI 模式
```

### Java

```bash
cd examples/java
javac SqlToolDemo.java
java SqlToolDemo                  # HTTP API 模式
java SqlToolDemo cli              # CLI 模式
```

### C#

```bash
cd examples/cs
dotnet run                        # HTTP API 模式
dotnet run -- cli                  # CLI 模式
```

## HTTP API 端点

启动服务器后可用：

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/health` | GET | 健康检查 |
| `/api/security/detect-injection` | POST | SQL 注入检测 |
| `/api/security/build-safe-sql` | POST | 构建安全 SQL |

## CLI 命令

| 命令 | 描述 |
|------|------|
| `detect-sql-injection` | 检测 SQL 注入风险 |
| `build-safe-sql` | 构建参数化 SQL |
| `health` | 健康检查 |
| `server` | 启动 HTTP API 服务器 |

## 示例输出

### CLI 模式

```
==================================================
1. SQL注入检测
==================================================
风险等级: High
发现: 经典 OR 注入

==================================================
2. 构建安全SQL
==================================================
table: users
field: name
value: test'; DROP TABLE
sanitized_value: test''; DROP TABLE
```

### HTTP API 模式

```json
{
  "success": true,
  "data": {
    "risk_level": "High",
    "findings": [
      {
        "pattern": "经典 OR 注入",
        "position": 0,
        "matched_text": "' OR '1'='1"
      }
    ]
  }
}
```
