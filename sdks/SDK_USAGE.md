# SQLTool - 多语言 SDK 调用指南

**核心**: Rust SQLTool 是核心二进制工具，其他语言可通过：
1. **subprocess 调用 CLI**（最简方式）
2. **HTTP API 客户端**（推荐，跨进程）
3. **SDK 高阶 API**（跨数据库迁移 + 智能分库分表）

---

## 目录

- [安装 SQLTool](#安装-sqltool)
- [支持的 SDK 语言](#支持的-sdk-语言)
- [Python SDK](#python-sdk)
- [Node.js SDK](#nodejs-sdk)
- [Go SDK](#go-sdk)
- [PHP SDK](#php-sdk)
- [Ruby SDK](#ruby-sdk)
- [Java SDK](#java-sdk)
- [C# SDK](#c-sdk)
- [Rust SDK](#rust-sdk)
- [CLI 命令参考](#cli-命令参考)

---

## 安装 SQLTool

```bash
# 方式1: cargo install（推荐）
cargo install sqltool

# 方式2: 下载二进制
# macOS
curl -L https://github.com/kodephp/sqltool/releases/latest/download/sqltool-macos.tar.gz | tar xz
# Linux
curl -L https://github.com/kodephp/sqltool/releases/latest/download/sqltool-linux.tar.gz | tar xz

# 方式3: 源码编译
git clone https://github.com/kodephp/sqltool.git
cd sqltool
cargo build --release
```

---

## 支持的 SDK 语言

| 语言 | 路径 | 核心能力 | 演示 |
|------|------|----------|------|
| Python | `sdks/python/sqltool_sdk.py` | HTTP+CLI+跨库迁移+分库分表 | `python3 sdks/python/sqltool_sdk.py` |
| Node.js | `sdks/node/sqltool_sdk.js` | HTTP+CLI+跨库迁移+分库分表 | `node sdks/node/sqltool_sdk.js` |
| Go | `sdks/go/sqltool_sdk.go` | HTTP+跨库迁移+分库分表 | `cd sdks/go/demo && go run .` |
| PHP | `sdks/php/sqltool_sdk.php` | CLI+跨库迁移+分库分表 | `php sdks/php/sqltool_sdk.php` |
| Ruby | `sdks/ruby/sqltool_sdk.rb` | HTTP+跨库迁移+分库分表 | `ruby sdks/ruby/sqltool_sdk.rb` |
| Java | `sdks/java/SqlTool.java` | HTTP+CLI+跨库迁移+分库分表 | `cd sdks/java && javac *.java && java -cp . com.sqltool.sdk.demo.SqlToolDemo` |
| C# | `sdks/csharp/SqlToolSdk.cs` | HTTP+CLI+跨库迁移+分库分表 | `cd sdks/csharp && dotnet run` |
| Rust | `examples/rust/src/main.rs` | 库 API + CLI（最强） | `cd examples/rust && cargo run` |

所有 SDK 共享同一套 **200+ 类型映射规则** 和 **6 级字段匹配算法**。

---

## Python SDK

### 安装依赖（可选）

```bash
pip install requests  # 仅 HTTP 模式需要
```

### 三种调用模式

```python
# 模式1: HTTP API 客户端（推荐）
from sqltool_sdk import SqlToolClient
client = SqlToolClient("http://localhost:8080")

# 模式2: CLI 包装器
from sqltool_sdk import SqlToolCLI
cli = SqlToolCLI()
result = cli.transfer("mysql://...", "postgresql://...")

# 模式3: 高阶 SDK API（无需启动服务）
from sqltool_sdk import CrossDbMigrator, SmartSharding
mig = CrossDbMigrator()
sharding = SmartSharding("orders", "user_id")
```

### 跨数据库迁移（高阶 API）

```python
from sqltool_sdk import CrossDbMigrator, TableSpec, FieldSpec

mig = CrossDbMigrator()

# 构造源表结构
table = TableSpec(name="orders", fields=[
    FieldSpec("id", "INT", primary_key=True, auto_increment=True),
    FieldSpec("user_id", "BIGINT"),
    FieldSpec("amount", "DECIMAL(10,2)"),
    FieldSpec("status", "VARCHAR(32)"),
    FieldSpec("created_at", "DATETIME"),
])

# 执行迁移（异构 + 跨版本）
result = mig.migrate_table(
    source="mysql://root:pass@localhost:3306/mydb",
    target="postgresql://postgres:pass@localhost:5432/mydb",
    table=table,
    source_version="5.7.40",
    target_version="16.2.0",
    auto_field_link=True,
    manual_field_map={"remark": "comment"},  # 字段重命名
)

print(f"方向: {result.direction}")
print(f"映射: {result.fields_mapped}/{result.fields_total} ({result.success_rate*100:.1f}%)")
print(f"DDL: {result.ddl}")
```

### 智能分库分表（高阶 API）

```python
from sqltool_sdk import SmartSharding, SpanningQuery, WriteOp

# 4 分片哈希分片
sharding = SmartSharding("orders", "user_id", strategy="hash")
sharding.add_shard("s0", "mysql://node1/orders_0", "orders_0")
sharding.add_shard("s1", "mysql://node1/orders_1", "orders_1")
sharding.add_shard("s2", "mysql://node2/orders_2", "orders_2")
sharding.add_shard("s3", "mysql://node2/orders_3", "orders_3")

# 路由
node = sharding.route("user_001")  # → s0/orders_0

# 跨分片查询
query = SpanningQuery(
    table="orders",
    columns=["id", "user_id", "amount"],
    where_clause="amount > 100",
    order_by=[("id", True)],
    limit=10,
    merge_strategy="sorted",
)
result = sharding.query(query)

# 批量写入
ops = [
    WriteOp("orders", "INSERT", "user_001", {"amount": 100}),
    WriteOp("orders", "INSERT", "user_042", {"amount": 200}),
]
write_result = sharding.write_batch(ops)

# Rebalance 计划
plan = sharding.rebalance_plan(total_rows=10_000_000)
```

### 演示运行

```bash
python3 sdks/python/sqltool_sdk.py
```

---

## Node.js SDK

### 调用方式

```javascript
// 方式1: HTTP API 客户端
const { SqlToolClient } = require('./sqltool_sdk');
const client = new SqlToolClient('http://localhost:8080');

// 方式2: CLI 包装器
const { execSync } = require('child_process');
execSync('sqltool backup -s mysql://...', { encoding: 'utf8' });

// 方式3: 高阶 SDK API
const { CrossDbMigrator, SmartSharding } = require('./sqltool_sdk');
```

### 跨数据库迁移

```javascript
const { CrossDbMigrator } = require('./sqltool_sdk');

const mig = new CrossDbMigrator();
const result = mig.migrateTable({
    source: 'mysql://root:pass@localhost:3306/mydb',
    target: 'postgresql://postgres:pass@localhost:5432/mydb',
    table: { name: 'orders', fields: [
        { name: 'id', dataType: 'INT', primaryKey: true },
        { name: 'user_id', dataType: 'BIGINT' },
        { name: 'amount', dataType: 'DECIMAL(10,2)' },
    ]},
    sourceVersion: '5.7.40',
    targetVersion: '16.2.0',
    manualFieldMap: { remark: 'comment' },
});

console.log(`方向: ${result.direction}`);
console.log(`DDL: ${result.ddl}`);
```

### 演示运行

```bash
node sdks/node/sqltool_sdk.js
```

---

## Go SDK

### 安装

```bash
cd sdks/go/demo
go mod tidy
go run .
```

### 调用方式

```go
package main

import (
    "sqlmap.local/sdks/go/sqltool"
)

func main() {
    // 1. HTTP 客户端
    client := sqltool.NewClient("http://localhost:8080")
    health, _ := client.Health()

    // 2. 跨数据库迁移
    mig := sqltool.NewCrossDbMigrator()
    result, _ := mig.MigrateTable(
        "mysql://root:pass@localhost:3306/mydb",
        "postgresql://postgres:pass@localhost:5432/mydb",
        sqltool.TableSpec{
            Name: "orders",
            Fields: []sqltool.FieldSpec{
                {Name: "id", DataType: "INT"},
                {Name: "user_id", DataType: "BIGINT"},
            },
        },
        "5.7.40", "16.2.0", nil,
    )
    fmt.Println(result.DDL)

    // 3. 智能分库分表
    sharding := sqltool.NewSmartSharding("orders", "user_id", "hash")
    sharding.AddShard("s0", "mysql://n1/orders_0", "orders_0")
    node, _ := sharding.Route("user_001")
    fmt.Println(node.ID)
}
```

---

## PHP SDK

### 调用方式

```php
<?php
require_once 'sqltool_sdk.php';

// CLI 包装
function sqltool(...$args) {
    $args = array_merge(['sqltool'], $args);
    $command = implode(' ', array_map('escapeshellarg', $args));
    $output = [];
    $returnCode = 0;
    exec($command, $output, $returnCode);
    if ($returnCode !== 0) {
        throw new RuntimeException("sqltool error: " . implode("\n", $output));
    }
    return implode("\n", $output);
}

// 跨数据库迁移
$mig = new SqlTool\CrossDbMigrator();
$result = $mig->migrateTable(
    'mysql://root:pass@localhost:3306/mydb',
    'postgresql://postgres:pass@localhost:5432/mydb',
    new SqlTool\TableSpec('orders', [
        new SqlTool\FieldSpec('id', 'INT'),
        new SqlTool\FieldSpec('user_id', 'BIGINT'),
    ]),
    '5.7.40', '16.2.0', []
);
echo $result->ddl;
```

### 演示运行

```bash
php sdks/php/sqltool_sdk.php
```

---

## Ruby SDK

### 调用方式

```ruby
require_relative 'sqltool_sdk'

# 跨数据库迁移
mig = SqlTool::CrossDbMigrator.new
result = mig.migrate_table(
    source: 'mysql://root:pass@localhost:3306/mydb',
    target: 'postgresql://postgres:pass@localhost:5432/mydb',
    table: SqlTool::TableSpec.new(name: 'orders', fields: [
        SqlTool::FieldSpec.new(name: 'id', data_type: 'INT'),
        SqlTool::FieldSpec.new(name: 'user_id', data_type: 'BIGINT'),
    ]),
    source_version: '5.7.40',
    target_version: '16.2.0'
)
puts result.ddl

# 智能分库分表
sharding = SqlTool::SmartSharding.new('orders', 'user_id', 'hash')
sharding.add_shard('s0', 'mysql://n1/orders_0', 'orders_0')
node = sharding.route('user_001')
puts node.id
```

### 演示运行

```bash
ruby sdks/ruby/sqltool_sdk.rb
```

---

## Java SDK

### 编译

```bash
cd sdks/java
javac SqlTool.java SqlToolDemo.java
java -cp . com.sqltool.sdk.demo.SqlToolDemo
```

### 调用方式

```java
import com.sqltool.sdk.SqlTool;
import com.sqltool.sdk.SqlTool.*;

// 跨数据库迁移
CrossDbMigrator mig = new CrossDbMigrator();
TableSpec table = new TableSpec("orders", Arrays.asList(
    new FieldSpec("id", "INT"),
    new FieldSpec("user_id", "BIGINT"),
    new FieldSpec("amount", "DECIMAL(10,2)")
));
MigrationResult result = mig.migrateTable(
    "mysql://root:pass@localhost:3306/mydb",
    "postgresql://postgres:pass@localhost:5432/mydb",
    table, "5.7.40", "16.2.0", null
);
System.out.println(result.ddl);

// 智能分库分表
SmartSharding sharding = new SmartSharding("orders", "user_id", "hash");
sharding.addShard("s0", "mysql://n1/orders_0", "orders_0");
sharding.addShard("s1", "mysql://n1/orders_1", "orders_1");
ShardNode node = sharding.route("user_001");
System.out.println(node.id);
```

---

## C# SDK

### 编译运行

```bash
cd sdks/csharp
dotnet run
```

### 调用方式

```csharp
using SqlTool.Sdk;

var mig = new CrossDbMigrator();
var table = new TableSpec("orders", new List<FieldSpec>
{
    new("id", "INT") { PrimaryKey = true },
    new("user_id", "BIGINT"),
    new("amount", "DECIMAL(10,2)"),
});
var result = mig.MigrateTable(
    "mysql://root:pass@localhost:3306/mydb",
    "postgresql://postgres:pass@localhost:5432/mydb",
    table, "5.7.40", "16.2.0", null
);
Console.WriteLine(result.Ddl);

var sharding = new SmartSharding("orders", "user_id", "hash");
sharding.AddShard("s0", "mysql://n1/orders_0", "orders_0");
var node = sharding.Route("user_001");
Console.WriteLine(node.Id);
```

---

## Rust SDK

Rust SDK 直接使用 sqltool 库 API，功能最完整。

### 运行

```bash
cd examples/rust
cargo run
```

### 调用方式

```rust
use sqltool::{
    CrossDbConverter, DataMigrator, MigrationConfig,
    ShardTopology, ShardNode, ShardStrategyKind,
    QueryCoordinator, WriteCoordinator, SpanningQuery, MergeStrategy,
    TargetDbKind,
};
use sqltool::databases::DatabaseVersion;
use sqltool::models::{Field, TableSchema};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 跨数据库迁移
    let source_table = TableSchema {
        name: "orders".to_string(),
        fields: vec![
            Field { name: "id".into(), data_type: "INT".into(), length: None, nullable: false, default_value: None, primary_key: true, auto_increment: true },
            Field { name: "user_id".into(), data_type: "BIGINT".into(), length: None, nullable: false, default_value: None, primary_key: false, auto_increment: false },
        ],
        indexes: vec![], foreign_keys: vec![],
    };

    let config = MigrationConfig {
        source_db: TargetDbKind::MySQL,
        target_db: TargetDbKind::PostgreSQL,
        source_version: Some(DatabaseVersion::new(5, 7, 40)),
        target_version: Some(DatabaseVersion::new(16, 2, 0)),
        auto_field_link: true,
        manual_field_map: HashMap::new(),
        batch_size: 5000,
        enable_version_upgrade: true,
        pre_check: true,
        default_source_version: None,
        default_target_version: None,
    };

    let migrator = DataMigrator::new();
    let result = migrator.migrate_table(&source_table, &config)?;
    println!("{}", result.ddl);

    // 2. 智能分库分表
    let topology = ShardTopology {
        logical_table: "orders".into(),
        shard_key: "user_id".into(),
        strategy: ShardStrategyKind::Hash { virtual_nodes: 64 },
        nodes: vec![
            ShardNode { id: "s0".into(), connection: "mysql://n1/orders_0".into(), table: "orders_0".into(), weight: 100, active: true },
        ],
        created_at: 0, updated_at: 0,
    };

    let mut write_coord = WriteCoordinator::new();
    write_coord.register(topology.clone());
    // 路由、查询合并、写入协调

    Ok(())
}
```

### Cargo.toml

```toml
[dependencies]
sqltool = "0.6"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

---

## CLI 命令参考

```bash
# 帮助
sqltool --help

# 数据库备份
sqltool backup -s mysql://user:pass@host:port/db --output ./backup.sql --backup-type full

# 备份恢复
sqltool restore --backup ./backup.sql -t mysql://user:pass@host:port/db

# 数据迁移
sqltool transfer -s mysql://source -t postgresql://target -B 5000

# 数据对比
sqltool compare-data -s db1 -t db2 --table users --primary-key id

# 创建分片
sqltool create-shard -s mysql://db --table orders --strategy row_count --threshold 1000000

# 跨分片查询
sqltool spanning-query -s mysql://db --table orders --condition "created_at > '2024-01-01'"

# 慢查询检测
sqltool detect-slow -s mysql://db --threshold-ms 1000

# 插入日志
sqltool insert-log -s mysql://db --table app_logs --level ERROR --message "test error"

# 查询日志
sqltool query-logs -s mysql://db --table app_logs --levels ERROR,WARN --limit 50

# SQL注入检测
sqltool detect-injection --input "' OR '1'='1"

# 构建安全SQL
sqltool build-safe-sql --table users --field name --operator = --value "test"

# 启动HTTP服务
sqltool server -p 8080 -s mysql://db --cors
```

---

## 连接字符串格式

| 数据库 | 格式 | 示例 |
|--------|------|------|
| MySQL | `mysql://user:pass@host:port/db` | `mysql://root:password@localhost:3306/mydb` |
| PostgreSQL | `postgresql://user:pass@host:port/db` | `postgresql://postgres:pass@localhost:5432/mydb` |
| SQLite | `sqlite:///path` 或 `sqlite:///:memory:` | `sqlite:///./mydb.sqlite` |
| Oracle | `oracle://user:pass@host:port/db` | `oracle://system:pass@localhost:1521/orcl` |
| Redis | `redis://host:port` | `redis://localhost:6379` |

---

## 跨数据库迁移方向（自动识别）

| 源 → 目标 | 方向 |
|-----------|------|
| MySQL 5.7 → MySQL 8.0 | `SameDbCrossVersion` |
| MySQL 8.0 → PostgreSQL 16 | `CrossDbSameVersion`（如源/目标版本一致） |
| MySQL 5.7 → PostgreSQL 16 | `CrossDbCrossVersion` |
| 同库同版本 | `SameDbSameVersion` |

---

## 智能分库分表

| 特性 | 描述 |
|------|------|
| 分片策略 | 哈希 / 范围 / 时间 / 一致性哈希 |
| 路由 | FNV-1a 64-bit 稳定哈希 |
| 跨分片查询 | 自动并行 + 结果归并 + 排序 + 分页 |
| 跨分片写入 | 按分片键路由到目标分片 |
| 动态扩容 | rebalance 计划生成 |
| 失败处理 | 失败重试 + 部分成功回滚（待扩展） |

---

## 字段自动连线（6 级匹配）

1. **精确匹配** - `user_id` == `user_id`
2. **大小写不敏感** - `UserId` == `userid`
3. **snake_case ↔ camelCase** - `user_id` == `userId`
4. **语义匹配** - `created_at` == `create_time`
5. **类型匹配** - `INT` == `INTEGER`
6. **未匹配** - 标记为需要手工处理

---

## 注意事项

1. **安全性**: 连接字符串中的密码包含特殊字符时，使用各语言的数组形式传参而非 shell 字符串
2. **路径**: 确保 `sqltool` 在 PATH 中，或使用完整路径 `/path/to/sqltool`
3. **错误处理**: 始终检查返回码和 stderr 输出
4. **超时**: 对于长时间运行的任务，考虑设置超时
5. **跨版本**: 建议先在测试环境验证，再用于生产
6. **有损转换**: TIMESTAMP→TIMESTAMPTZ 等可能损失精度，会在报告中标注
