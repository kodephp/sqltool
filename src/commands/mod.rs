use clap::Parser;
use anyhow::Result;

mod server;
pub use server::start_http_server;

/// 命令行参数
#[derive(Parser, Debug)]
#[command(
    author = "SQLTool Team",
    version,
    about = "SQLTool - 智能数据库迁移与运维工具",
    long_about = "功能强大的数据库迁移、同步、运维工具，支持：
  - 数据库迁移与同步
  - 自动分库分表
  - 慢查询检测
  - 数据对比与备份
  - HTTP API 服务模式"
)]
pub struct Args {
    /// 启用详细输出
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,

    /// 子命令
    #[command(subcommand)]
    pub command: Command,
}

/// 子命令
#[derive(Parser, Debug)]
pub enum Command {
    /// ============ 数据迁移命令 ============
    /// 数据迁移 - 在两个数据库之间迁移数据
    Transfer {
        /// 源数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 目标数据库连接字符串
        #[arg(short = 't', long)]
        target: String,

        /// 源数据库类型 (mysql/postgresql/sqlite/redis/oracle)
        #[arg(short = 'S', long, default_value = "mysql")]
        source_type: String,

        /// 目标数据库类型
        #[arg(short = 'T', long, default_value = "postgresql")]
        target_type: String,

        /// 表名列表，逗号分隔，默认迁移所有表
        #[arg(short, long)]
        tables: Option<String>,

        /// 批量大小
        #[arg(short = 'B', long, default_value_t = 1000)]
        batch_size: usize,

        /// 验证数据完整性
        #[arg(short = 'v', long, default_value_t = true)]
        verify: bool,
    },

    /// ============ 结构迁移命令 ============
    /// 迁移表结构（索引、约束等）
    MigrateSchema {
        /// 源数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 目标数据库连接字符串
        #[arg(short = 't', long)]
        target: String,

        /// 源数据库类型
        #[arg(short = 'S', long, default_value = "mysql")]
        source_type: String,

        /// 目标数据库类型
        #[arg(short = 'T', long, default_value = "postgresql")]
        target_type: String,

        /// 表名
        #[arg(short, long)]
        table: String,
    },

    /// ============ 数据对比命令 ============
    /// 对比两个数据库的数据
    CompareData {
        /// 源数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 目标数据库连接字符串
        #[arg(short = 't', long)]
        target: String,

        /// 源数据库类型
        #[arg(short = 'S', long, default_value = "mysql")]
        source_type: String,

        /// 目标数据库类型
        #[arg(short = 'T', long, default_value = "postgresql")]
        target_type: String,

        /// 表名
        #[arg(short, long)]
        table: String,

        /// 主键字段
        #[arg(short, long, default_value = "id")]
        primary_key: String,

        /// 忽略的字段（逗号分隔）
        #[arg(short, long)]
        ignore_fields: Option<String>,

        /// 输出格式 (json/table/text)
        #[arg(short, long, default_value = "json")]
        output: String,
    },

    /// ============ 数据库备份命令 ============
    /// 备份数据库
    Backup {
        /// 数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 数据库类型
        #[arg(short = 'T', long, default_value = "mysql")]
        db_type: String,

        /// 备份保存路径
        #[arg(short, long)]
        output: String,

        /// 备份类型 (full/incremental/differential)
        #[arg(short, long, default_value = "full")]
        backup_type: String,

        /// 压缩备份
        #[arg(short = 'c', long, default_value_t = true)]
        compress: bool,

        /// 包含存储过程
        #[arg(long, default_value_t = true)]
        include_procedures: bool,

        /// 包含函数
        #[arg(long, default_value_t = true)]
        include_functions: bool,

        /// 包含触发器
        #[arg(long, default_value_t = true)]
        include_triggers: bool,
    },

    /// ============ 数据库恢复命令 ============
    /// 从备份恢复数据库
    Restore {
        /// 备份文件路径
        #[arg(short, long)]
        backup: String,

        /// 目标数据库连接字符串
        #[arg(short = 't', long)]
        target: String,

        /// 目标数据库类型
        #[arg(short = 'T', long, default_value = "mysql")]
        db_type: String,
    },

    /// ============ 分库分表命令 ============
    /// 创建分片
    CreateShard {
        /// 数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 表名
        #[arg(short, long)]
        table: String,

        /// 分片策略 (row_count/date_suffix/size_based/time_interval)
        #[arg(short, long, default_value = "row_count")]
        strategy: String,

        /// 最大行数/大小阈值
        #[arg(short, long)]
        threshold: Option<String>,

        /// 分片前缀
        #[arg(short, long, default_value = "shard")]
        prefix: String,
    },

    /// ============ 跨分片查询命令 ============
    /// 执行跨分片查询
    SpanningQuery {
        /// 数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 表名
        #[arg(short, long)]
        table: String,

        /// 查询条件 (WHERE 子句)
        #[arg(short, long, default_value = "1=1")]
        condition: String,

        /// 排序字段
        #[arg(short, long)]
        order_by: Option<String>,

        /// 排序方向 (ASC/DESC)
        #[arg(short, long, default_value = "ASC")]
        order_dir: String,

        /// 返回数量
        #[arg(short = 'L', long, default_value_t = 100)]
        limit: u64,

        /// 偏移量
        #[arg(short, long, default_value_t = 0)]
        offset: u64,

        /// 输出格式
        #[arg(short, long, default_value = "json")]
        output: String,
    },

    /// ============ 慢查询检测命令 ============
    /// 检测慢查询
    DetectSlowQuery {
        /// 数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 数据库类型
        #[arg(short = 'T', long, default_value = "mysql")]
        db_type: String,

        /// 慢查询阈值（毫秒）
        #[arg(short, long, default_value_t = 1000)]
        threshold_ms: u64,

        /// 查询SQL文件路径
        #[arg(short, long)]
        query_file: Option<String>,

        /// 直接执行的SQL
        #[arg(short, long)]
        sql: Option<String>,

        /// 输出格式
        #[arg(short, long, default_value = "json")]
        output: String,
    },

    /// ============ 日志表管理命令 ============
    /// 插入日志
    InsertLog {
        /// 数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 表名
        #[arg(short, long, default_value = "app_logs")]
        table: String,

        /// 日志级别 (DEBUG/INFO/WARN/ERROR)
        #[arg(short, long, default_value = "INFO")]
        level: String,

        /// 日志消息
        #[arg(short, long)]
        message: String,

        /// 来源
        #[arg(short, long)]
        source_name: Option<String>,
    },

    /// 查询日志
    QueryLogs {
        /// 数据库连接字符串
        #[arg(short = 's', long)]
        source: String,

        /// 表名
        #[arg(short, long, default_value = "app_logs")]
        table: String,

        /// 日志级别过滤（逗号分隔）
        #[arg(short, long)]
        levels: Option<String>,

        /// 关键字过滤
        #[arg(short, long)]
        keyword: Option<String>,

        /// 开始时间 (Unix时间戳)
        #[arg(short, long)]
        start_time: Option<i64>,

        /// 结束时间
        #[arg(long)]
        end_time: Option<i64>,

        /// 返回数量
        #[arg(short = 'L', long, default_value_t = 100)]
        limit: u64,

        /// 输出格式
        #[arg(short, long, default_value = "json")]
        output: String,
    },

    /// ============ HTTP API 服务模式 ============
    /// 启动 HTTP API 服务器
    Server {
        /// 监听地址
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,

        /// 监听端口
        #[arg(short = 'p', long, default_value_t = 8080)]
        port: u16,

        /// 数据库连接字符串（用于连接池）
        #[arg(short = 's', long)]
        source: Option<String>,

        /// 数据库类型
        #[arg(short = 'T', long, default_value = "mysql")]
        db_type: String,

        /// 启用 CORS
        #[arg(long, default_value_t = false)]
        cors: bool,

        /// API 密钥
        #[arg(long)]
        api_key: Option<String>,
    },

    /// ============ 安全检测命令 ============
    /// SQL注入检测
    DetectSqlInjection {
        /// 要检测的输入
        #[arg(short, long)]
        input: String,

        /// 严格模式
        #[arg(short, long, default_value_t = false)]
        strict: bool,
    },

    /// 安全SQL构建
    BuildSafeSql {
        /// 表名
        #[arg(short, long)]
        table: String,

        /// 字段
        #[arg(short, long)]
        field: String,

        /// 操作符
        #[arg(short, long, default_value = "=")]
        operator: String,

        /// 值
        #[arg(short, long)]
        value: String,
    },
}

/// 执行命令
pub async fn execute(args: Args) -> Result<()> {
    let verbose = args.verbose;
    let command = args.command;

    if verbose {
        println!("SQLTool v{} - 智能数据库迁移与运维工具", env!("CARGO_PKG_VERSION"));
        println!("=======================================\n");
    }

    match command {
        Command::Transfer { source, target, source_type, target_type, .. } => {
            validate_connection_string(&source, &source_type)?;
            validate_connection_string(&target, &target_type)?;
            println!("执行数据迁移命令...");
        }
        Command::MigrateSchema { source, target, source_type, target_type, .. } => {
            validate_connection_string(&source, &source_type)?;
            validate_connection_string(&target, &target_type)?;
            println!("执行结构迁移命令...");
        }
        Command::CompareData { source, target, source_type, target_type, .. } => {
            validate_connection_string(&source, &source_type)?;
            validate_connection_string(&target, &target_type)?;
            println!("执行数据对比命令...");
        }
        Command::Backup { source, db_type, .. } => {
            validate_connection_string(&source, &db_type)?;
            println!("执行数据库备份命令...");
        }
        Command::Restore { target, db_type, .. } => {
            validate_connection_string(&target, &db_type)?;
            println!("执行数据库恢复命令...");
        }
        Command::CreateShard { source, table, .. } => {
            validate_connection_string(&source, "mysql")?;
            validate_table_name(&table)?;
            println!("执行创建分片命令...");
        }
        Command::SpanningQuery { source, table, condition, .. } => {
            validate_connection_string(&source, "mysql")?;
            validate_table_name(&table)?;
            validate_condition(&condition)?;
            println!("执行跨分片查询命令...");
        }
        Command::DetectSlowQuery { source, db_type, sql, query_file, .. } => {
            validate_connection_string(&source, &db_type)?;
            if sql.is_none() && query_file.is_none() {
                anyhow::bail!("必须指定 --sql 或 --query-file");
            }
            println!("执行慢查询检测命令...");
        }
        Command::InsertLog { source, table, level, message, .. } => {
            validate_connection_string(&source, "mysql")?;
            validate_table_name(&table)?;
            validate_log_level(&level)?;
            if message.is_empty() {
                anyhow::bail!("日志消息不能为空");
            }
            println!("执行插入日志命令...");
        }
        Command::QueryLogs { source, levels, .. } => {
            validate_connection_string(&source, "mysql")?;
            if let Some(ref l) = levels {
                validate_log_level(l)?;
            }
            println!("执行查询日志命令...");
        }
        Command::Server { port, source, .. } => {
            if let Some(ref s) = source {
                validate_connection_string(s, "mysql")?;
            }
            if port < 1024 {
                println!("警告: 端口 {} 小于1024，可能需要root权限", port);
            }
            println!("启动 HTTP API 服务器...");
            start_server(port, source).await?;
        }
        Command::DetectSqlInjection { input, .. } => {
            if input.len() > 10000 {
                anyhow::bail!("输入过长，最大10000字符");
            }
            println!("执行 SQL 注入检测...");
        }
        Command::BuildSafeSql { table, field, operator, value, .. } => {
            validate_table_name(&table)?;
            validate_field_name(&field)?;
            validate_operator(&operator)?;
            if value.len() > 10000 {
                anyhow::bail!("值过长，最大10000字符");
            }
            println!("执行安全 SQL 构建...");
        }
    }

    Ok(())
}

fn validate_connection_string(conn: &str, db_type: &str) -> Result<()> {
    use crate::utils::validate_connection_string;

    let parsed = validate_connection_string(conn).map_err(|e| {
        anyhow::anyhow!("连接字符串验证失败: {}", e)
    })?;

    if parsed.db_type != db_type {
        log::warn!(
            "连接字符串类型 {} 与指定类型 {} 不匹配",
            parsed.db_type, db_type
        );
    }

    Ok(())
}

fn validate_table_name(table: &str) -> Result<()> {
    if table.is_empty() {
        anyhow::bail!("表名不能为空");
    }
    if table.len() > 64 {
        anyhow::bail!("表名过长，最大64字符");
    }
    if !table.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
        anyhow::bail!("表名只能包含字母、数字、下划线和点");
    }
    Ok(())
}

fn validate_field_name(field: &str) -> Result<()> {
    if field.is_empty() {
        anyhow::bail!("字段名不能为空");
    }
    if field.len() > 64 {
        anyhow::bail!("字段名过长，最大64字符");
    }
    if !field.chars().all(|c| c.is_alphanumeric() || c == '_') {
        anyhow::bail!("字段名只能包含字母、数字和下划线");
    }
    Ok(())
}

fn validate_condition(condition: &str) -> Result<()> {
    if condition.len() > 2000 {
        anyhow::bail!("条件过长，最大2000字符");
    }
    let dangerous = ["DROP ", "DELETE ", "TRUNCATE ", "ALTER ", "CREATE ", "INSERT ", "UPDATE "];
    for kw in dangerous {
        if condition.to_uppercase().contains(kw) {
            log::warn!("条件包含危险关键词: {}", kw);
        }
    }
    Ok(())
}

fn validate_operator(op: &str) -> Result<()> {
    let valid = ["=", "!=", "<>", "<", ">", "<=", ">=", "LIKE", "IN", "BETWEEN", "IS NULL", "IS NOT NULL"];
    if !valid.iter().any(|v| v.eq_ignore_ascii_case(op)) {
        anyhow::bail!("无效的操作符: {}，有效值: {:?}", op, valid);
    }
    Ok(())
}

fn validate_log_level(level: &str) -> Result<()> {
    let valid = ["DEBUG", "INFO", "WARN", "ERROR", "TRACE", "FATAL"];
    if !valid.iter().any(|v| v.eq_ignore_ascii_case(level)) {
        anyhow::bail!("无效的日志级别: {}，有效值: DEBUG, INFO, WARN, ERROR, TRACE, FATAL", level);
    }
    Ok(())
}

/// 启动 HTTP API 服务器
async fn start_server(port: u16, source: Option<String>) -> Result<()> {
    let host = "127.0.0.1".to_string();
    let db_type = "mysql".to_string();
    let cors = false;
    let api_key = None;

    println!("HTTP API 服务器配置:");
    println!("  监听地址: {}:{}", host, port);
    println!("  数据库类型: {}", db_type);
    println!("  CORS 启用: {}", cors);
    println!("  API 密钥: {}", if api_key.is_some() { "已设置" } else { "未设置" });

    start_http_server(host, port, source, db_type, cors, api_key).await?;

    Ok(())
}

/// 打印命令帮助信息
pub fn print_help() {
    println!(r#"
SQLTool - 智能数据库迁移与运维工具 v{}

用法:
  sqltool [选项] <子命令>

子命令:
  transfer          数据迁移 - 在两个数据库之间迁移数据
  migrate-schema   结构迁移 - 迁移表结构（索引、约束等）
  compare-data     数据对比 - 对比两个数据库的数据
  backup           数据库备份 - 备份整个数据库
  restore          数据库恢复 - 从备份恢复数据库
  create-shard     创建分片 - 为大表创建分片
  spanning-query   跨分片查询 - 查询多个分片的数据
  detect-slow     慢查询检测 - 检测和分析慢查询
  insert-log      插入日志 - 向日志表插入日志
  query-logs       查询日志 - 查询日志表数据
  server           HTTP API 服务器 - 启动 REST API 服务
  detect-injection SQL注入检测 - 检测 SQL 注入风险
  build-safe-sql   安全SQL构建 - 构建安全的 SQL 语句

选项:
  -v, --verbose    启用详细输出
  -h, --help       显示帮助信息
  -V, --version    显示版本信息

示例:
  # 数据迁移
  sqltool transfer -s mysql://root:pass@localhost:3306/source_db \\
                    -t postgresql://postgres:pass@localhost:5432/target_db

  # 数据库备份
  sqltool backup -s mysql://root:pass@localhost:3306/mydb \\
                  --output ./backup.sql

  # 启动 API 服务器
  sqltool server -p 8080 -s mysql://root:pass@localhost:3306/mydb

更多信息: https://github.com/yourusername/sqltool
"#, env!("CARGO_PKG_VERSION"));
}
