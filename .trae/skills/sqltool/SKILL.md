---
name: "sqltool"
description: "SQLTool数据库迁移运维工具开发。Invoke when working on SQLTool Rust project - database migration, backup, sharding, sync features."
---

# SQLTool - 智能数据库迁移与运维工具

## 项目概述

SQLTool 是一个 Rust 编写的数据库迁移、运维工具，支持：
- **跨数据库异构迁移**: MySQL ↔ PostgreSQL ↔ SQLite ↔ Oracle ↔ MSSQL ↔ TiDB ↔ MariaDB
- **数据迁移**: 同库跨版本、异构同版本、异构跨版本自动转换
- **数据同步**: 实时同步、增量同步、定时同步
- **智能分库分表**: 哈希/范围/时间/一致性哈希 + 跨分片查询合并 + 写入协调
- **备份恢复**: 全量/增量/差异备份
- **数据对比**: 结构对比、数据对比、验证报告

- **当前版本**: 0.6.1
- **许可证**: Apache-2.0
- **Rust 版本**: 1.96+
- **支持数据库**: 8+ (MySQL, MariaDB, PostgreSQL, SQLite, TiDB, Oracle, MSSQL, CockroachDB, Redis, MongoDB)

## 项目结构

```
sqlmap/
├── src/
│   ├── core/                    # 核心功能
│   │   ├── cross_db_conversion.rs   # 跨数据库异构转换
│   │   ├── data_migration.rs        # 数据迁移
│   │   ├── smart_sharding.rs        # 智能分库分表
│   │   ├── distributed_tx.rs        # 分布式事务
│   │   ├── data_transfer.rs         # 数据迁移引擎
│   │   ├── auto_sharding.rs         # 自动分库分表
│   │   ├── log_table.rs             # 日志管理
│   │   ├── slow_query.rs            # 慢查询检测
│   │   ├── query_fusion.rs          # 跨分片查询
│   │   └── ...
│   ├── databases/              # 数据库驱动
│   │   ├── mysql.rs
│   │   ├── postgres.rs
│   │   ├── sqlite.rs
│   │   ├── oracle.rs
│   │   └── redis.rs
│   ├── models/                 # 数据模型
│   ├── utils/                  # 工具模块
│   ├── lib.rs
│   └── main.rs
├── sdks/                       # 多语言 SDK（8 种）
│   ├── python/                 # Python SDK
│   ├── node/                   # Node.js SDK
│   ├── go/                     # Go SDK（含 demo/ 子目录）
│   ├── php/                    # PHP SDK
│   ├── ruby/                   # Ruby SDK
│   ├── java/                   # Java SDK
│   ├── csharp/                 # C# SDK
│   └── SDK_USAGE.md            # 多语言 SDK 调用指南
├── tests/                      # 集成测试
├── examples/                   # 多语言示例
│   ├── python/  node/  go/  php/  ruby/  java/  cs/  rust/
│   └── cli/
├── .trae/
│   ├── rules/                  # 项目规则
│   └── skills/                 # 技能配置
├── Cargo.toml
├── LICENSE                     # Apache-2.0
└── README.md
```

## 常用命令

```bash
# 构建
cargo build --release

# 测试
cargo test
cargo test --lib
cargo test --test '*'

# 代码检查
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# 基准测试
cargo bench

# 运行 CLI
./target/release/sqltool backup -s mysql://... --output ./backup.sql

# 启动 HTTP 服务
./target/release/sqltool server -p 8080 -s mysql://...
```

## CLI 子命令

| 命令 | 说明 | 示例 |
|------|------|------|
| `transfer` | 数据迁移（异构 + 跨版本） | `sqltool transfer -s mysql://... -t pgsql://...` |
| `backup` | 数据库备份 | `sqltool backup -s mysql://...` |
| `restore` | 备份恢复 | `sqltool restore --backup ./backup.sql` |
| `compare-data` | 数据对比 | `sqltool compare-data -s db1 -t db2 --table users` |
| `create-shard` | 分库分表 | `sqltool create-shard --table orders --strategy row_count` |
| `spanning-query` | 跨分片查询 | `sqltool spanning-query --table orders` |
| `detect-slow-query` | 慢查询检测 | `sqltool detect-slow-query --threshold-ms 1000` |
| `insert-log` | 插入日志 | `sqltool insert-log --table app_logs --level ERROR` |
| `query-logs` | 查询日志 | `sqltool query-logs --table app_logs --levels ERROR` |
| `detect-sql-injection` | SQL 注入检测 | `sqltool detect-sql-injection --input "'; DROP TABLE"` |
| `build-safe-sql` | 安全 SQL 构建 | `sqltool build-safe-sql --table users` |
| `server` | HTTP API 服务 | `sqltool server -p 8080` |

## 核心模块使用

### 跨数据库迁移

```rust
use sqltool::core::cross_db_conversion::TargetDbKind;
use sqltool::databases::DatabaseVersion;
use sqltool::models::{Field, TableSchema};
use sqltool::{DataMigrator, MigrationConfig};
use std::collections::HashMap;

let source_table = TableSchema {
    name: "orders".to_string(),
    fields: vec![
        Field { name: "id".into(), data_type: "INT".into(), length: None,
                nullable: false, default_value: None, primary_key: true, auto_increment: true },
    ],
    indexes: vec![], foreign_keys: vec![],
};

let mut manual_field_map = HashMap::new();
manual_field_map.insert("remark".to_string(), "comment".to_string());

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
println!("{}", result.ddl);
```

### 智能分库分表

```rust
use sqltool::{
    ShardTopology, ShardNode, ShardStrategyKind,
    QueryCoordinator, WriteCoordinator, SpanningQuery, MergeStrategy,
    ShardWriteOp,
};
use std::collections::HashMap;

let topology = ShardTopology {
    logical_table: "orders".into(),
    shard_key: "user_id".into(),
    strategy: ShardStrategyKind::Hash { virtual_nodes: 64 },
    nodes: vec![
        ShardNode { id: "s0".into(), connection: "mysql://n1/orders_0".into(),
                    table: "orders_0".into(), weight: 100, active: true },
    ],
    created_at: 0, updated_at: 0,
};

let mut write_coord = WriteCoordinator::new();
write_coord.register(topology.clone());

// 路由
let mut values = HashMap::new();
values.insert("user_id".to_string(), serde_json::Value::String("u1".into()));
let op = ShardWriteOp::Insert { table: "orders".into(), values };
let node = write_coord.route_op(&op)?;

// 跨分片查询
let mut query_coord = QueryCoordinator::new();
query_coord.register(topology);
let query = SpanningQuery {
    logical_table: "orders".into(),
    columns: Some(vec!["id".into(), "user_id".into()]),
    where_clause: Some("amount > 100".into()),
    order_by: Some(vec![("id".into(), true)]),
    limit: Some(10), offset: None,
    merge_strategy: MergeStrategy::SortedMerge,
    parallel: true,
};
let result = query_coord.execute(query).await?;
```

### 数据备份

```rust
use sqltool::core::{DatabaseBackup, BackupConfig, BackupType};

let config = BackupConfig {
    backup_type: BackupType::Full,
    compress: true,
    ..Default::default()
};
let mut backup = DatabaseBackup::new(&conn, config);
let report = backup.execute_backup("backup_20240101").await?;
```

### 数据对比

```rust
use sqltool::core::{DataComparer, DataCompareConfig, CompareMode};

let config = DataCompareConfig {
    compare_mode: CompareMode::Full,
    primary_key: "id".to_string(),
    ..Default::default()
};
let comparer = DataComparer::new(&conn1, &conn2, config);
let result = comparer.compare_table("users").await?;
```

## 发布流程

### 完整发布流程

```bash
# ====== 1. 更新版本与元信息 ======

# 1.1 更新版本号（同时更新 Cargo.toml 头部注释和 README）
vim Cargo.toml
#   - version = "0.6.2"
#   - license = "Apache-2.0"
#   - 顶部注释更新版本号和功能描述

# 1.2 更新 README 中的版本引用
#   - 标题 v0.6.x
#   - 徽章 crates.io v0.6.x
#   - 测试徽章数字
#   - Cargo.toml 依赖示例 sqltool = "0.6.x"
#   - 测试统计表
#   - 许可证 Apache-2.0

# 1.3 同步更新 .trae/rules/project_rules.md 的「当前版本」
vim .trae/rules/project_rules.md

# ====== 2. 质量门禁 ======

# 2.1 全量测试
cargo test

# 2.2 本地构建
cargo build --release

# 2.3 Clippy 检查
cargo clippy --all-targets -- -D warnings

# 2.4 格式化
cargo fmt --check

# 2.5 预览打包
cargo package --list

# 2.6 校验打包
cargo package --allow-dirty

# ====== 3. 登录 crates.io（首次） ======

# 3.1 在 https://crates.io/settings/tokens 生成 API token

# 3.2 登录
cargo login
# 提示输入 token

# 凭据默认存放在 ~/.cargo/credentials.toml

# ====== 4. 推送到 GitHub ======

git add -A
git commit -m "chore: bump version to v0.6.x"
git tag v0.6.x
git push origin master
git push origin v0.6.x

# ====== 5. 发布到 crates.io ======

cargo publish
# 或 cargo publish --allow-dirty 在未提交时强制发布

# ====== 6. 验证 ======

cargo search sqltool
# 或浏览器打开 https://crates.io/crates/sqltool
```

### 常用发布命令速查

| 命令 | 用途 |
|------|------|
| `cargo login` | 登录 crates.io（首次） |
| `cargo login <token>` | 使用 token 登录 |
| `cargo package --list` | 预览包内容 |
| `cargo package` | 打包到 `target/package/` |
| `cargo publish` | 推送到 crates.io |
| `cargo publish --dry-run` | 干跑（不上传） |
| `cargo publish --allow-dirty` | 允许未提交修改时发布 |
| `cargo yank --version 0.6.1` | 撤回已发布版本 |
| `cargo owner --add github:user` | 添加包协作者 |

### crates.io 必填字段

`Cargo.toml` 中必须有：
- `name` - 包名
- `version` - SemVer
- `edition` - 推荐 2021
- `license` - SPDX 标识符（如 `Apache-2.0`）
- `description` - 一句话描述
- `repository` - 仓库 URL
- `readme` - README 文件名

### 故障排查

| 现象 | 解决 |
|------|------|
| `failed to authenticate` | 重新 `cargo login` |
| `crate name already taken` | 修改 `name` 字段 |
| `no targets to publish` | 添加 `[[bin]]` 或 `examples` |
| `invalid version` | 遵循 SemVer 规则 |
| `missing license` | 设置 `license = "Apache-2.0"` |
| `missing description` | 添加功能描述 |
| `package size > 10MB` | 调整 `exclude` 或 `.gitignore` |

## 多语言 SDK

8 种语言官方 SDK，统一支持跨数据库迁移 + 智能分库分表。详见 `sdks/SDK_USAGE.md`。

### Python
```python
from sqltool_sdk import CrossDbMigrator, SmartSharding

mig = CrossDbMigrator()
result = mig.migrate_table(
    source="mysql://root:pass@localhost:3306/mydb",
    target="postgresql://postgres:pass@localhost:5432/mydb",
    table=table, source_version="5.7.40", target_version="16.2.0",
    auto_field_link=True, manual_field_map={"remark": "comment"},
)
print(result.ddl)
```

### Node.js
```javascript
const { CrossDbMigrator } = require('./sqltool_sdk');
const result = new CrossDbMigrator().migrateTable({
    source: 'mysql://...', target: 'postgresql://...',
    table: { name: 'orders', fields: [...] },
    sourceVersion: '5.7.40', targetVersion: '16.2.0',
});
```

### Go
```go
import "sqlmap.local/sdks/go/sqltool"

mig := sqltool.NewCrossDbMigrator()
result, _ := mig.MigrateTable(
    "mysql://...", "postgresql://...",
    sqltool.TableSpec{Name: "orders", Fields: ...},
    "5.7.40", "16.2.0", nil,
)
```

### PHP / Ruby / Java / C# / Rust
详见 `sdks/SDK_USAGE.md`

## 开发规范

1. **方法命名**: 简洁明了，如 `backup`, `restore`, `compare`, `sync`
2. **错误处理**: 使用 `anyhow::Result<T>`
3. **异步代码**: 使用 `#[tokio::main]` 和 `async/await`
4. **文档注释**: 为公开 API 添加 `///` 文档注释
5. **版本号**: 遵循 SemVer 规范
6. **提交消息**: 简明扼要，使用约定式提交（feat/fix/docs/chore）

## 项目规则

详见 [.trae/rules/project_rules.md](file:///Users/Zhuanz/Desktop/website/composer/sqlmap/.trae/rules/project_rules.md)
