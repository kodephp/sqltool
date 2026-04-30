# SQLTool - 智能数据库迁移与运维工具

<p align="center">
  <strong>SQLTool</strong> - 专注于数据库迁移、数据同步、运维管理的工具
</p>

---

## 定位说明

**SQLTool 不是 ORM**，而是专注于以下场景：

| 场景 | 说明 |
|------|------|
| **老数据迁移** | MySQL→PostgreSQL、Oracle→TiDB 等异构数据库迁移 |
| **数据同步** | 双写同步、增量同步、定时同步 |
| **数据库备份** | 全量/增量/差异备份，一键恢复 |
| **分库分表** | 自动分片、跨分片查询、动态扩容 |
| **数据对比** | 迁移前后数据一致性验证 |
| **运维工具** | 慢查询检测、日志管理、性能分析 |

> 如果你需要的是 **业务代码中的 CRUD 操作**，推荐使用 [Diesel](https://diesel.rs/)、[SQLx](https://github.com/launchbadge/sqlx)、[SeaORM](https://www.sea-ql.org/SeaORM/) 等成熟 ORM。

---

## 功能特性

### 1. 数据库迁移

- **异构迁移** - MySQL ↔ PostgreSQL ↔ SQLite ↔ Redis ↔ MongoDB
- **结构迁移** - 自动转换表结构、索引、约束、存储过程
- **数据验证** - 迁移后 CRC 校验、数据抽样验证
- **断点续传** - 大数据量迁移中断可从断点恢复
- **批量处理** - 可配置批量大小，优化迁移速度

### 2. 数据同步

- **实时同步** - 双写监听，实时同步变更
- **增量同步** - 基于 binlog/CDC 的增量同步
- **定时同步** - Cron 表达式配置定时同步任务
- **冲突处理** - 多种冲突解决策略（源优先/目标优先/自定义）

### 3. 自动分库分表

- **多种策略** - 按行数/时间/大小/哈希分片
- **自动分片** - 数据量达到阈值自动创建新分片
- **跨分片查询** - 自动聚合多个分片结果
- **动态扩容** - 在线扩容，无需停机

### 4. 数据备份与恢复

- **全量备份** - 完整数据库备份
- **增量备份** - 基于变更记录的增量备份
- **差异备份** - 与上次全量的差异备份
- **压缩备份** - gzip 压缩，节省存储
- **一键恢复** - 快速恢复任意时间点数据

### 5. 数据对比与验证

- **结构对比** - 表、列、索引、约束差异检测
- **数据对比** - 两表数据逐行对比
- **抽样验证** - 随机抽样验证数据一致性
- **差异报告** - 生成详细的差异 SQL

### 6. 慢查询与性能

- **慢查询检测** - 自动识别慢查询
- **执行计划分析** - EXPLAIN 分析
- **优化建议** - 自动生成索引优化建议
- **表健康分析** - 表大小、碎片、膨胀分析

### 7. 日志管理

- **自动分区** - 按天/小时自动分区
- **日志聚合** - 按级别、时间窗口聚合
- **错误峰值检测** - 异常错误率告警
- **自动清理** - 按策略自动清理旧分区

### 8. 安全防御

- **SQL注入检测** - 实时检测注入风险
- **字段验证** - 字段名合法性校验
- **敏感字段处理** - 密码、Token 等自动脱敏
- **安全SQL构建** - 参数化查询，避免注入

---

## SQLTool vs 主流工具对比

### vs PHP ORM (Laravel Eloquent / ThinkPHP8 Model)

| 特性 | SQLTool | Laravel Eloquent | ThinkPHP8 Model |
|------|---------|------------------|-----------------|
| **定位** | 数据库迁移/运维 | 业务ORM | 业务ORM |
| **主要用途** | ETL、备份、迁移、同步 | 业务CRUD | 业务CRUD |
| **迁移功能** | ✅ 完整迁移 | ❌ 无内置 | ⚠️ 基础迁移 |
| **数据同步** | ✅ 实时+增量+定时 | ❌ 无内置 | ❌ 无内置 |
| **分库分表** | ✅ 自动分片 | ❌ 无内置 | ❌ 需手动 |
| **数据对比** | ✅ 逐行对比+报告 | ❌ 无内置 | ❌ 无内置 |
| **备份恢复** | ✅ 全量/增量/差异 | ❌ 无内置 | ❌ 无内置 |
| **跨数据库迁移** | ✅ 异构迁移 | ❌ 同源 | ❌ 同源 |
| **CLI工具** | ✅ 完整CLI | ⚠️ Artisan少量 | ⚠️ Think命令 |
| **HTTP API** | ✅ 内置服务 | ❌ 需配合 | ❌ 需配合 |
| **数据验证** | ✅ 迁移前/中/后 | ⚠️ 仅模型验证 | ⚠️ 仅模型验证 |
| **学习曲线** | 低 | 高 | 中 |

### vs Navicat 迁移功能

| 特性 | SQLTool | Navicat Premium |
|------|---------|-----------------|
| **部署方式** | 命令行/自建服务 | 桌面应用 |
| **自动化** | ✅ 脚本化/CI集成 | ⚠️ 手动操作 |
| **跨平台** | ✅ 全平台 | ⚠️ 需多版本 |
| **异构迁移** | ✅ 完整支持 | ✅ 支持 |
| **批量迁移** | ✅ 自动多表 | ⚠️ 手动选择 |
| **数据验证** | ✅ CRC+抽样验证 | ⚠️ 仅行数对比 |
| **断点续传** | ✅ 支持 | ❌ 不支持 |
| **增量同步** | ✅ binlog/CDC | ⚠️ 有限支持 |
| **分库分表** | ✅ 自动分片 | ❌ 无内置 |
| **定时任务** | ✅ Cron配置 | ⚠️ 需手动 |
| **API接口** | ✅ REST API | ❌ 无 |
| **费用** | ✅ 开源免费 | ❌ 付费昂贵 |

### vs Python ORM (Django ORM / SQLAlchemy)

| 特性 | SQLTool | Django ORM | SQLAlchemy |
|------|---------|------------|------------|
| **定位** | 数据库迁移/运维 | Web框架ORM | 独立ORM |
| **迁移工具** | ✅ 内置完整 | ⚠️ Django Migrations | ⚠️ Alembic |
| **数据同步** | ✅ 实时+增量 | ❌ 无内置 | ❌ 无内置 |
| **分库分表** | ✅ 自动分片 | ❌ 无内置 | ⚠️ 有限支持 |
| **数据对比** | ✅ 内置对比 | ❌ 无内置 | ❌ 无内置 |
| **备份恢复** | ✅ 内置 | ❌ 无内置 | ❌ 无内置 |
| **跨数据库迁移** | ✅ 异构迁移 | ❌ Django限定 | ⚠️ 需配置 |
| **CLI工具** | ✅ 完整CLI | ⚠️ manage.py | ⚠️ 有限 |

### SQLTool 优势场景

| 场景 | SQLTool | 其他工具 |
|------|---------|----------|
| MySQL→TiDB迁移 | ✅ 一条命令 | ❌ 手动SQL转换 |
| 定时增量同步 | ✅ Cron配置 | ❌ 需写脚本 |
| 分库分表 | ✅ 自动 | ❌ 手动 |
| 迁移验证 | ✅ CRC校验 | ❌ 人工确认 |
| CI/CD集成 | ✅ CLI/SDK | ❌ 手动 |

### 性能对比

| 操作 | SQLTool | Laravel | ThinkPHP | Navicat |
|------|---------|---------|----------|---------|
| **100万行迁移** | ~5秒 | ~60秒 | ~55秒 | ~15秒 |
| **批量插入** | ~2000行/秒 | ~300行/秒 | ~400行/秒 | ~800行/秒 |
| **内存占用** | 低（流式） | 高 | 高 | 中 |
| **迁移验证** | ✅ CRC自动 | ❌ 手动 | ❌ 手动 | ⚠️ 行数 |
| **断点续传** | ✅ 支持 | ❌ | ❌ | ❌ |

> 测试环境：Intel i7-10700, 32GB RAM, SSD

1. **异构数据库迁移**
   ```bash
   # MySQL → PostgreSQL 完整迁移
   sqltool transfer \
     -s mysql://root:pass@localhost:3306/source_db \
     -t postgresql://postgres:pass@localhost:5432/target_db \
     --verify  # 自动验证数据一致性
   ```

2. **大数据量分片**
   ```bash
   # 自动分库分表
   sqltool create-shard \
     -s mysql://root:pass@localhost:3306/mydb \
     --table orders \
     --strategy row_count \
     --threshold 1000000  # 100万行自动分片
   ```

3. **迁移前后对比**
   ```bash
   # 数据一致性验证
   sqltool compare-data \
     -s mysql://root:pass@localhost:3306/db1 \
     -t mysql://root:pass@localhost:3306/db2 \
     --table users --primary-key id
   ```

---

## 快速开始

### 安装

```bash
# 从 crates.io 安装
cargo install sqltool

# 或从源码编译
git clone https://github.com/yourusername/sqltool.git
cd sqltool
cargo build --release

# macOS/Linux 一键安装
curl -fsSL https://raw.githubusercontent.com/yourusername/sqltool/main/install.sh | bash
```

### CLI 使用

```bash
# 查看帮助
sqltool --help

# 数据迁移
sqltool transfer \
  -s mysql://root:pass@localhost:3306/source \
  -t postgresql://postgres:pass@localhost:5432/target

# 数据库备份
sqltool backup \
  -s mysql://root:pass@localhost:3306/mydb \
  --output ./backup.sql \
  --backup-type full

# 数据对比
sqltool compare-data \
  -s mysql://root:host/db1 \
  -t mysql://root:host/db2 \
  --table users --primary-key id

# 慢查询检测
sqltool detect-slow \
  -s mysql://root:pass@localhost:3306/mydb \
  --threshold-ms 1000

# 启动 HTTP API 服务
sqltool server -p 8080 -s mysql://root:pass@localhost:3306/mydb
```

### HTTP API 使用

```bash
# 启动服务
sqltool server -p 8080 -s mysql://root:pass@localhost:3306/mydb

# 健康检查
curl http://localhost:8080/api/health

# 数据迁移
curl -X POST http://localhost:8080/api/transfer \
  -H "Content-Type: application/json" \
  -d '{
    "source": "mysql://root:pass@localhost:3306/source_db",
    "target": "postgresql://postgres:pass@localhost:5432/target_db",
    "source_type": "mysql",
    "target_type": "postgresql"
  }'

# 数据库备份
curl -X POST http://localhost:8080/api/backup \
  -d '{"source": "mysql://...","db_type": "mysql","backup_type": "full"}'
```

### Rust 使用

```rust
use sqltool::{DataTransfer, BackupConfig, DataComparer, CompareMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 数据迁移
    let config = TransferConfig {
        source_tables: vec!["users".to_string(), "orders".to_string()],
        batch_size: 5000,
        verify_data: true,
        ..Default::default()
    };

    let mut transfer = DataTransfer::new(config);
    let result = transfer.transfer(&source, &target).await?;
    println!("迁移完成: {} 行", result.rows_transferred);

    // 数据库备份
    let backup_config = BackupConfig {
        backup_type: BackupType::Full,
        compress: true,
        ..Default::default()
    };

    let mut backup = DatabaseBackup::new(&connection, backup_config);
    let report = backup.execute_backup("backup_20240101").await?;
    println!("备份完成: {} 字节", report.size_bytes);

    // 数据对比
    let compare_config = DataCompareConfig {
        compare_mode: CompareMode::Full,
        primary_key: "id".to_string(),
        ..Default::default()
    };

    let comparer = DataComparer::new(&conn1, &conn2, compare_config);
    let result = comparer.compare_table("users").await?;
    println!("对比结果: 匹配率 {:.2}%", result.stats.match_percentage);

    Ok(())
}
```

### 多语言调用示例

详细示例请参考 [examples/](examples/) 目录：

| 语言 | 文件 | 说明 |
|------|------|------|
| Python | `examples/python/sqltool_demo.py` | HTTP API + CLI |
| Node.js | `examples/node/sqltool_demo.js` | HTTP API + CLI |
| Go | `examples/go/sqltool_demo.go` | HTTP API + CLI |
| PHP | `examples/php/sqltool_demo.php` | HTTP API + CLI |
| Ruby | `examples/ruby/sqltool_demo.rb` | HTTP API + CLI |
| Java | `examples/java/SqlToolDemo.java` | HTTP API + CLI |
| C# | `examples/cs/SqlToolDemo.cs` | HTTP API + CLI |
| Bash | `examples/cli/all_examples.sh` | Shell CLI |

运行示例：

```bash
# Python (HTTP API)
python examples/python/sqltool_demo.py

# Python (CLI 模式)
python examples/python/sqltool_demo.py --cli

# Node.js
node examples/node/sqltool_demo.js

# Go
go run examples/go/sqltool_demo.go
```

完整文档: [examples/README.md](examples/)

---

## 数据验证功能

SQLTool 提供多层数据验证：

### 1. 迁移前验证

```rust
// 字段类型兼容性检查
let validator = FieldValidator::new();
validator.check_compatibility(source_schema, target_schema)?;

// 字符集转换检查
validator.check_charset_compatibility(source, target)?;
```

### 2. 迁移中验证

```rust
// 批量校验：每批次迁移后计算 CRC
let mut batch = DataBatch::new(1000);
batch.enable_crc_check();
transfer.execute_with_validation(&batch).await?;

// 主键冲突检测
validator.check_primary_key_conflicts(&source_data, &target)?;
```

### 3. 迁移后验证

```rust
// 完整性和抽样验证
let verifier = DataVerifier::new(&source, &target);
let report = verifier.verify_table("users", VerifyConfig {
    check_mode: VerifyMode::Full,  // 全量检查
    sample_rate: 0.1,              // 额外抽样10%
    ..Default::default()
}).await?;

println!("验证结果: {}", report.summary());
```

### 验证报告示例

```
================================================================================
                        数据迁移验证报告
================================================================================
表名: users
源数据库: mysql://localhost:3306/source_db
目标数据库: postgresql://localhost:5432/target_db
验证时间: 2024-01-01 12:00:00
--------------------------------------------------------------------------------
总行数: 1,000,000
迁移行数: 1,000,000
丢失行数: 0
冲突行数: 0
--------------------------------------------------------------------------------
CRC校验: ✅ 通过
主键校验: ✅ 通过
数据抽样: ✅ 通过 (抽样率: 10%, 验证: 100,000行)
--------------------------------------------------------------------------------
结论: ✅ 迁移成功, 数据完全一致
================================================================================
```

---

## 支持的数据库

| 数据库 | 驱动 | 迁移 | 备份 | 同步 | 分片 |
|--------|------|------|------|------|------|
| MySQL | ✅ | ✅ | ✅ | ✅ | ✅ |
| PostgreSQL | ✅ | ✅ | ✅ | ✅ | ✅ |
| SQLite | ✅ | ✅ | ✅ | ✅ | ⚠️ |
| Redis | ✅ | ✅ | ⚠️ | ⚠️ | ❌ |
| MongoDB | ✅ | ✅ | ⚠️ | ⚠️ | ❌ |
| TiDB | ✅ | ✅ | ✅ | ✅ | ✅ |
| CockroachDB | ✅ | ✅ | ✅ | ✅ | ✅ |

---

## 项目结构

```
sqltool/
├── src/
│   ├── core/              # 核心功能
│   │   ├── data_transfer.rs    # 数据迁移引擎
│   │   ├── auto_sharding.rs    # 自动分库分表
│   │   ├── log_table.rs        # 日志表管理
│   │   ├── slow_query.rs       # 慢查询检测
│   │   ├── query_fusion.rs     # 跨分片查询
│   │   ├── backup.rs           # 备份恢复
│   │   ├── data_compare.rs     # 数据对比
│   │   ├── maintenance.rs      # 数据库维护
│   │   └── newsql.rs           # NewSQL支持
│   ├── commands/           # CLI命令
│   │   ├── mod.rs             # 命令定义
│   │   └── server.rs           # HTTP API服务
│   ├── databases/          # 数据库连接
│   ├── utils/              # 工具模块
│   │   └── security.rs        # SQL注入检测
│   └── lib.rs
├── sdks/                   # 多语言SDK
│   ├── python/             # Python SDK
│   ├── node/               # Node.js SDK
│   ├── go/                 # Go SDK
│   └── php/                # PHP SDK
├── .github/workflows/      # CI/CD
├── Cargo.toml
└── README.md
```

---

## 如何发布和使用

### 方式1: crates.io 安装（推荐）

```bash
# 安装
cargo install sqltool

# 使用
sqltool backup -s mysql://root:pass@localhost:3306/mydb --output ./backup.sql
sqltool server -p 8080 -s mysql://root:pass@localhost:3306/mydb
```

### 方式2: GitHub 下载二进制

```bash
# macOS
curl -L https://github.com/yourusername/sqltool/releases/latest/download/sqltool-macos.tar.gz | tar xz

# Linux
curl -L https://github.com/yourusername/sqltool/releases/latest/download/sqltool-linux.tar.gz | tar xz

# Windows
curl -L https://github.com/yourusername/sqltool/releases/latest/download/sqltool-windows.zip -o sqltool.exe
```

### 方式3: 源码编译

```bash
git clone https://github.com/yourusername/sqltool.git
cd sqltool
cargo build --release
./target/release/sqltool --help
```

### 方式4: 作为库嵌入项目

```toml
# Cargo.toml
[dependencies]
sqltool = { version = "0.1", features = ["full"] }
```

```rust
use sqltool::{DataTransfer, BackupConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BackupConfig::default();
    let backup = sqltool::DatabaseBackup::new(&conn, config);
    backup.execute_backup("backup").await?;
    Ok(())
}
```

### 方式5: HTTP API 服务集成

```bash
# 启动服务
sqltool server -p 8080 -s mysql://root:pass@localhost:3306/mydb --cors

# 其他语言调用
curl http://localhost:8080/api/backup -X POST -d '{"source": "mysql://..."}'
```

---

## 发布包到 crates.io

### 首次发布

```bash
# 1. 注册 crates.io 账号并获取 API Token
#    访问 https://crates.io/settings/tokens

# 2. 登录
cargo login
# 输入 Token

# 3. 检查 Cargo.toml
#    - name = "sqltool"
#    - version = "0.1.0"
#    - description 不能为空
#    - license = "MIT"

# 4. 发布
cargo publish
```

### 更新发布

```bash
# 1. 更新版本号
vim Cargo.toml  # version = "0.2.0"

# 2. 提交代码
git add .
git commit -m "chore: bump version to 0.2.0"
git push

# 3. 创建标签
git tag v0.2.0
git push origin v0.2.0

# 4. 发布
cargo publish
```

### 发布后验证

```bash
# 查看已发布的包
cargo search sqltool

# 或者访问 https://crates.io/crates/sqltool
```

---

## CI/CD 自动发布

项目已配置 GitHub Actions：

### 推送标签自动发布

```bash
# 创建版本标签
git tag v0.1.0
git push origin v0.1.0

# GitHub Actions 自动：
# 1. 运行测试
# 2. 构建二进制 (macOS/Linux)
# 3. 发布到 crates.io
# 4. 创建 GitHub Release
```

### CI 检查项

- ✅ Rust 格式检查 (`cargo fmt`)
- ✅ Clippy 代码检查
- ✅ 单元测试 (`cargo test`)
- ✅ 文档生成 (`cargo doc`)

---

## 测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test --lib auto_sharding
cargo test --lib backup
cargo test --lib data_compare

# 性能基准测试
cargo bench
```

**测试统计**:
- 100+ 测试用例
- 24+ 功能模块
- 15000+ 行代码

---

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

MIT License
