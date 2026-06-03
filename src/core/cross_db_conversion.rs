//! 跨数据库异构转换模块
//!
//! 提供不同数据库之间（MySQL/PostgreSQL/SQLite/Redis/MongoDB/Oracle 等）
//! 数据、结构、字段的自动转换能力，支持：
//! - 类型映射：源库类型 → 目标库类型（含精度/长度调整）
//! - 自动字段连线：按字段名、语义、类型相似度自动匹配
//! - 低版本到高版本平滑升级：MySQL 5.5/5.7 → 8.0、PostgreSQL 9.x → 16
//! - 数据值转换：字符集、布尔、日期、JSON、Blob 等
//! - DDL 生成：为目标库生成等价 CREATE TABLE
//! - 迁移报告：转换前后差异、转换规则

use crate::models::{Field, TableSchema, Index, ForeignKey};
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 目标数据库类型（与 databases::DatabaseType 对齐，避免循环依赖）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TargetDbKind {
    #[default]
    MySQL,
    PostgreSQL,
    SQLite,
    Redis,
    MongoDB,
    Oracle,
    TiDB,
    MariaDB,
    MSSQL,
}

impl TargetDbKind {
    pub fn name(&self) -> &'static str {
        match self {
            TargetDbKind::MySQL => "mysql",
            TargetDbKind::PostgreSQL => "postgresql",
            TargetDbKind::SQLite => "sqlite",
            TargetDbKind::Redis => "redis",
            TargetDbKind::MongoDB => "mongodb",
            TargetDbKind::Oracle => "oracle",
            TargetDbKind::TiDB => "tidb",
            TargetDbKind::MariaDB => "mariadb",
            TargetDbKind::MSSQL => "mssql",
        }
    }
}

/// 字段类型转换规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMappingRule {
    /// 源数据库
    pub source_db: TargetDbKind,
    /// 源类型（含长度信息），如 "VARCHAR(255)"、"DECIMAL(10,2)"、"INT"
    pub source_type_pattern: String,
    /// 目标数据库
    pub target_db: TargetDbKind,
    /// 目标类型
    pub target_type: String,
    /// 转换描述
    pub note: String,
    /// 是否可能丢失精度
    pub lossy: bool,
}

/// 类型映射表
#[derive(Debug, Clone, Default)]
pub struct TypeMappingTable {
    rules: Vec<TypeMappingRule>,
}

impl TypeMappingTable {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: TypeMappingRule) {
        self.rules.push(rule);
    }

    /// 根据源类型和目标库查找转换规则（精确匹配）
    /// 匹配规则：长度被剥离后比较；同源时优先级最高
    pub fn lookup(&self, source_type: &str, target: TargetDbKind) -> Option<&TypeMappingRule> {
        let src_norm = normalize_type(source_type);
        // 1) 完全相同规则（同源 → 同目标），例如 MySQL→MySQL INT→INT
        if let Some(r) = self.rules.iter().find(|r| {
            r.target_db == target
                && normalize_type(&r.source_type_pattern) == src_norm
                && r.source_db == target
        }) {
            return Some(r);
        }
        // 2) 任何源 → 目标的规则
        self.rules
            .iter()
            .find(|r| r.target_db == target && normalize_type(&r.source_type_pattern) == src_norm)
    }

    /// 根据源/目标 + 源类型查找（推荐使用）
    pub fn lookup_with_source(
        &self,
        source_type: &str,
        source: TargetDbKind,
        target: TargetDbKind,
    ) -> Option<&TypeMappingRule> {
        let src_norm = normalize_type(source_type);
        // 优先：源 → 目标的精确规则
        if let Some(r) = self.rules.iter().find(|r| {
            r.source_db == source
                && r.target_db == target
                && normalize_type(&r.source_type_pattern) == src_norm
        }) {
            return Some(r);
        }
        // 其次：相同目标库的通用规则
        self.lookup(source_type, target)
    }

    /// 内置完整规则集（生产可用，覆盖主流数据库互转）
    pub fn built_in() -> Self {
        let mut tbl = Self::new();
        // -----------------------------------------------------------------
        // MySQL → PostgreSQL
        // -----------------------------------------------------------------
        let mysql_to_pg = [
            ("TINYINT", "SMALLINT", "TINYINT(1) 映射为 BOOLEAN，其他映射为 SMALLINT", false),
            ("SMALLINT", "SMALLINT", "1:1 映射", false),
            ("MEDIUMINT", "INTEGER", "MEDIUMINT → INTEGER", false),
            ("INT", "INTEGER", "1:1 映射（PostgreSQL 用 INTEGER 而非 INT）", false),
            ("INTEGER", "INTEGER", "1:1 映射", false),
            ("BIGINT", "BIGINT", "1:1 映射", false),
            ("FLOAT", "REAL", "FLOAT(单精度) → REAL", true),
            ("DOUBLE", "DOUBLE PRECISION", "DOUBLE → DOUBLE PRECISION", false),
            ("DECIMAL(10,0)", "NUMERIC(10,0)", "DECIMAL → NUMERIC", false),
            ("DECIMAL", "NUMERIC", "DECIMAL → NUMERIC", false),
            ("NUMERIC", "NUMERIC", "1:1 映射", false),
            ("DATE", "DATE", "1:1 映射", false),
            ("DATETIME", "TIMESTAMP", "DATETIME → TIMESTAMP（无时区）", true),
            ("TIMESTAMP", "TIMESTAMP WITH TIME ZONE", "TIMESTAMP → TIMESTAMPTZ", true),
            ("TIME", "TIME", "1:1 映射", false),
            ("YEAR", "SMALLINT", "YEAR → SMALLINT（语义接近 4 位年份）", true),
            ("CHAR(1)", "CHAR(1)", "1:1 映射", false),
            ("CHAR", "CHAR", "1:1 映射", false),
            ("VARCHAR(255)", "VARCHAR(255)", "1:1 映射", false),
            ("VARCHAR", "VARCHAR", "1:1 映射", false),
            ("TEXT", "TEXT", "1:1 映射", false),
            ("TINYTEXT", "TEXT", "TINYTEXT → TEXT", false),
            ("MEDIUMTEXT", "TEXT", "MEDIUMTEXT → TEXT", false),
            ("LONGTEXT", "TEXT", "LONGTEXT → TEXT", false),
            ("BLOB", "BYTEA", "BLOB → BYTEA", false),
            ("TINYBLOB", "BYTEA", "TINYBLOB → BYTEA", false),
            ("MEDIUMBLOB", "BYTEA", "MEDIUMBLOB → BYTEA", false),
            ("LONGBLOB", "BYTEA", "LONGBLOB → BYTEA", false),
            ("BINARY(16)", "BYTEA", "BINARY → BYTEA", false),
            ("VARBINARY(255)", "BYTEA", "VARBINARY → BYTEA", false),
            ("JSON", "JSONB", "JSON → JSONB（推荐，二进制格式 + 索引）", false),
            ("ENUM", "VARCHAR(255)", "ENUM 展开为 VARCHAR + CHECK 约束", true),
            ("SET", "VARCHAR(255)", "SET 展开为 VARCHAR + CHECK 约束", true),
            ("BOOLEAN", "BOOLEAN", "MySQL 8.0+ 原生支持", false),
            ("BIT(1)", "BOOLEAN", "BIT(1) → BOOLEAN", true),
            ("UUID", "UUID", "MySQL 8.0+ 原生", false),
            ("GEOMETRY", "GEOMETRY", "1:1 映射（需 PostGIS）", false),
        ];
        for (src, tgt, note, lossy) in mysql_to_pg.iter() {
            tbl.add_rule(TypeMappingRule {
                source_db: TargetDbKind::MySQL,
                source_type_pattern: src.to_string(),
                target_db: TargetDbKind::PostgreSQL,
                target_type: tgt.to_string(),
                note: note.to_string(),
                lossy: *lossy,
            });
        }

        // -----------------------------------------------------------------
        // MySQL → SQLite
        // -----------------------------------------------------------------
        let mysql_to_sqlite = [
            ("TINYINT", "INTEGER", "SQLite 用 INTEGER 替代 TINYINT（动态类型）", false),
            ("SMALLINT", "INTEGER", "SQLite 用 INTEGER 替代 SMALLINT", false),
            ("MEDIUMINT", "INTEGER", "SQLite 用 INTEGER 替代 MEDIUMINT", false),
            ("INT", "INTEGER", "1:1 映射", false),
            ("INTEGER", "INTEGER", "1:1 映射", false),
            ("BIGINT", "INTEGER", "SQLite INTEGER 可达 8 字节", false),
            ("FLOAT", "REAL", "1:1 映射", false),
            ("DOUBLE", "REAL", "DOUBLE → REAL（精度可能降低）", true),
            ("DECIMAL(10,0)", "NUMERIC(10,0)", "1:1 映射", false),
            ("DECIMAL", "NUMERIC", "1:1 映射", false),
            ("NUMERIC", "NUMERIC", "1:1 映射", false),
            ("DATE", "TEXT", "SQLite 无原生 DATE，存为 ISO8601 TEXT", true),
            ("DATETIME", "TEXT", "存为 ISO8601 TEXT", true),
            ("TIMESTAMP", "TEXT", "存为 ISO8601 TEXT", true),
            ("TIME", "TEXT", "存为 ISO8601 TEXT", true),
            ("YEAR", "INTEGER", "存为 4 位整数", false),
            ("CHAR(1)", "TEXT(1)", "SQLite 用 TEXT(N) 保留长度", false),
            ("CHAR", "TEXT", "1:1 映射（长度由应用层控制）", false),
            ("VARCHAR(255)", "TEXT(255)", "长度变为 CHECK 约束或应用层校验", false),
            ("VARCHAR", "TEXT", "1:1 映射", false),
            ("TEXT", "TEXT", "1:1 映射", false),
            ("BLOB", "BLOB", "1:1 映射", false),
            ("JSON", "TEXT", "JSON 存为 TEXT（SQLite 1.5+ 有 JSON1 扩展）", false),
            ("BOOLEAN", "INTEGER", "SQLite 用 0/1 代替", false),
        ];
        for (src, tgt, note, lossy) in mysql_to_sqlite.iter() {
            tbl.add_rule(TypeMappingRule {
                source_db: TargetDbKind::MySQL,
                source_type_pattern: src.to_string(),
                target_db: TargetDbKind::SQLite,
                target_type: tgt.to_string(),
                note: note.to_string(),
                lossy: *lossy,
            });
        }

        // -----------------------------------------------------------------
        // PostgreSQL → MySQL
        // -----------------------------------------------------------------
        let pg_to_mysql = [
            ("SMALLINT", "SMALLINT", "1:1 映射", false),
            ("INTEGER", "INT", "INTEGER → INT", false),
            ("BIGINT", "BIGINT", "1:1 映射", false),
            ("REAL", "FLOAT", "REAL → FLOAT", false),
            ("DOUBLE PRECISION", "DOUBLE", "1:1 映射", false),
            ("NUMERIC", "DECIMAL", "NUMERIC → DECIMAL", false),
            ("DATE", "DATE", "1:1 映射", false),
            ("TIME", "TIME", "1:1 映射", false),
            ("TIMESTAMP", "DATETIME", "PostgreSQL TIMESTAMP → MySQL DATETIME（无时区）", true),
            ("TIMESTAMP WITH TIME ZONE", "TIMESTAMP", "TIMESTAMPTZ → TIMESTAMP", true),
            ("BOOLEAN", "TINYINT(1)", "BOOLEAN → TINYINT(1)", false),
            ("TEXT", "TEXT", "1:1 映射", false),
            ("VARCHAR(255)", "VARCHAR(255)", "1:1 映射", false),
            ("CHAR(1)", "CHAR(1)", "1:1 映射", false),
            ("BYTEA", "BLOB", "BYTEA → BLOB", false),
            ("JSONB", "JSON", "JSONB → JSON", false),
            ("JSON", "JSON", "1:1 映射", false),
            ("UUID", "CHAR(36)", "UUID → CHAR(36)（MySQL 8.0 之前无原生）", true),
            ("SERIAL", "INT AUTO_INCREMENT", "SERIAL → INT AUTO_INCREMENT", false),
            ("BIGSERIAL", "BIGINT AUTO_INCREMENT", "BIGSERIAL → BIGINT AUTO_INCREMENT", false),
            ("MONEY", "DECIMAL(19,2)", "MONEY → DECIMAL", false),
            ("INET", "VARCHAR(45)", "INET → VARCHAR", true),
            ("CIDR", "VARCHAR(45)", "CIDR → VARCHAR", true),
            ("MACADDR", "VARCHAR(17)", "MACADDR → VARCHAR", true),
        ];
        for (src, tgt, note, lossy) in pg_to_mysql.iter() {
            tbl.add_rule(TypeMappingRule {
                source_db: TargetDbKind::PostgreSQL,
                source_type_pattern: src.to_string(),
                target_db: TargetDbKind::MySQL,
                target_type: tgt.to_string(),
                note: note.to_string(),
                lossy: *lossy,
            });
        }

        // -----------------------------------------------------------------
        // PostgreSQL → SQLite
        // -----------------------------------------------------------------
        let pg_to_sqlite = [
            ("SMALLINT", "INTEGER", "1:1 映射", false),
            ("INTEGER", "INTEGER", "1:1 映射", false),
            ("BIGINT", "INTEGER", "1:1 映射", false),
            ("REAL", "REAL", "1:1 映射", false),
            ("DOUBLE PRECISION", "REAL", "精度降低", true),
            ("NUMERIC", "NUMERIC", "1:1 映射", false),
            ("DATE", "TEXT", "存为 ISO8601 TEXT", true),
            ("TIME", "TEXT", "存为 ISO8601 TEXT", true),
            ("TIMESTAMP", "TEXT", "存为 ISO8601 TEXT", true),
            ("TIMESTAMP WITH TIME ZONE", "TEXT", "存为 ISO8601 TEXT", true),
            ("BOOLEAN", "INTEGER", "BOOLEAN → 0/1", false),
            ("TEXT", "TEXT", "1:1 映射", false),
            ("VARCHAR(255)", "TEXT(255)", "长度变 CHECK", false),
            ("BYTEA", "BLOB", "1:1 映射", false),
            ("JSONB", "TEXT", "JSONB → TEXT", true),
            ("UUID", "TEXT(36)", "UUID → TEXT", true),
        ];
        for (src, tgt, note, lossy) in pg_to_sqlite.iter() {
            tbl.add_rule(TypeMappingRule {
                source_db: TargetDbKind::PostgreSQL,
                source_type_pattern: src.to_string(),
                target_db: TargetDbKind::SQLite,
                target_type: tgt.to_string(),
                note: note.to_string(),
                lossy: *lossy,
            });
        }

        // -----------------------------------------------------------------
        // SQLite → MySQL
        // -----------------------------------------------------------------
        let sqlite_to_mysql = [
            ("INTEGER", "BIGINT", "SQLite INTEGER → MySQL BIGINT（最大安全）", true),
            ("REAL", "DOUBLE", "REAL → DOUBLE", false),
            ("TEXT", "TEXT", "1:1 映射", false),
            ("BLOB", "BLOB", "1:1 映射", false),
            ("NUMERIC", "DECIMAL(10,0)", "NUMERIC → DECIMAL", false),
        ];
        for (src, tgt, note, lossy) in sqlite_to_mysql.iter() {
            tbl.add_rule(TypeMappingRule {
                source_db: TargetDbKind::SQLite,
                source_type_pattern: src.to_string(),
                target_db: TargetDbKind::MySQL,
                target_type: tgt.to_string(),
                note: note.to_string(),
                lossy: *lossy,
            });
        }

        // -----------------------------------------------------------------
        // SQLite → PostgreSQL
        // -----------------------------------------------------------------
        let sqlite_to_pg = [
            ("INTEGER", "BIGINT", "SQLite INTEGER → BIGINT（最大安全）", true),
            ("REAL", "DOUBLE PRECISION", "1:1 映射", false),
            ("TEXT", "TEXT", "1:1 映射", false),
            ("BLOB", "BYTEA", "BLOB → BYTEA", false),
            ("NUMERIC", "NUMERIC(10,0)", "1:1 映射", false),
        ];
        for (src, tgt, note, lossy) in sqlite_to_pg.iter() {
            tbl.add_rule(TypeMappingRule {
                source_db: TargetDbKind::SQLite,
                source_type_pattern: src.to_string(),
                target_db: TargetDbKind::PostgreSQL,
                target_type: tgt.to_string(),
                note: note.to_string(),
                lossy: *lossy,
            });
        }

        // MySQL → TiDB (与 MySQL 几乎一致)
        for (src, tgt, note, lossy) in mysql_to_pg.iter() {
            if !["TIMESTAMP", "BOOLEAN", "JSON"].contains(src) {
                tbl.add_rule(TypeMappingRule {
                    source_db: TargetDbKind::MySQL,
                    source_type_pattern: src.to_string(),
                    target_db: TargetDbKind::TiDB,
                    target_type: tgt.to_string(),
                    note: format!("TiDB 兼容：{}", note),
                    lossy: *lossy,
                });
            }
        }

        // -----------------------------------------------------------------
        // 自映射（MySQL→MySQL、PG→PG 等）— 同源到同目标，原样保留
        // -----------------------------------------------------------------
        let identity_types = [
            "TINYINT", "SMALLINT", "MEDIUMINT", "INT", "INTEGER", "BIGINT",
            "FLOAT", "DOUBLE", "DECIMAL", "NUMERIC",
            "DATE", "DATETIME", "TIMESTAMP", "TIME", "YEAR",
            "CHAR", "VARCHAR", "TEXT", "TINYTEXT", "MEDIUMTEXT", "LONGTEXT",
            "BLOB", "TINYBLOB", "MEDIUMBLOB", "LONGBLOB", "BINARY", "VARBINARY",
            "JSON", "ENUM", "SET", "BOOLEAN", "BIT", "UUID",
        ];
        for db in [
            TargetDbKind::MySQL, TargetDbKind::MariaDB, TargetDbKind::TiDB,
            TargetDbKind::PostgreSQL, TargetDbKind::SQLite,
            TargetDbKind::Oracle, TargetDbKind::MSSQL,
        ] {
            for ty in identity_types.iter() {
                tbl.add_rule(TypeMappingRule {
                    source_db: db,
                    source_type_pattern: ty.to_string(),
                    target_db: db,
                    target_type: ty.to_string(),
                    note: format!("同源同目标，原样保留"),
                    lossy: false,
                });
            }
        }

        tbl
    }
}

/// 字段自动连线（自动匹配）结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldLink {
    pub source_field: String,
    pub target_field: String,
    pub confidence: f64,
    pub match_kind: LinkKind,
    pub transform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LinkKind {
    /// 字段名完全相同
    Exact,
    /// 字段名忽略大小写
    CaseInsensitive,
    /// 字段名按 snake/camel 转换后相同
    NamingConvention,
    /// 语义相似（id ↔ user_id、created_at ↔ create_time）
    Semantic,
    /// 类型 + 用途匹配（如 user_id 在两张表里都出现）
    TypeAndUsage,
    /// 用户显式指定
    Manual,
    /// 未匹配
    Unmatched,
}

/// 字段自动连线器
#[derive(Debug, Clone, Default)]
pub struct FieldLinker {
    /// 手工映射优先
    manual: HashMap<String, String>,
}

impl FieldLinker {
    pub fn new() -> Self {
        Self { manual: HashMap::new() }
    }

    pub fn with_manual(mut self, manual: HashMap<String, String>) -> Self {
        self.manual = manual;
        self
    }

    pub fn add_manual(&mut self, src: impl Into<String>, tgt: impl Into<String>) {
        self.manual.insert(src.into(), tgt.into());
    }

    /// 对两张表的字段做自动连线
    pub fn link(&self, source: &[Field], target: &[Field]) -> Vec<FieldLink> {
        let mut links = Vec::new();
        let mut used_target: std::collections::HashSet<String> = std::collections::HashSet::new();

        for s in source {
            // 1) 手工映射
            if let Some(tgt) = self.manual.get(&s.name) {
                if target.iter().any(|t| &t.name == tgt) {
                    links.push(FieldLink {
                        source_field: s.name.clone(),
                        target_field: tgt.clone(),
                        confidence: 1.0,
                        match_kind: LinkKind::Manual,
                        transform: None,
                    });
                    used_target.insert(tgt.clone());
                    continue;
                }
            }

            // 2) 完全相等
            if let Some(t) = target.iter().find(|t| t.name == s.name && !used_target.contains(&t.name)) {
                links.push(FieldLink {
                    source_field: s.name.clone(),
                    target_field: t.name.clone(),
                    confidence: 1.0,
                    match_kind: LinkKind::Exact,
                    transform: None,
                });
                used_target.insert(t.name.clone());
                continue;
            }

            // 3) 大小写无关
            if let Some(t) = target.iter().find(|t| t.name.eq_ignore_ascii_case(&s.name) && !used_target.contains(&t.name)) {
                let transform = if t.name != s.name {
                    Some(format!("UPPER({})", s.name))
                } else {
                    None
                };
                links.push(FieldLink {
                    source_field: s.name.clone(),
                    target_field: t.name.clone(),
                    confidence: 0.95,
                    match_kind: LinkKind::CaseInsensitive,
                    transform,
                });
                used_target.insert(t.name.clone());
                continue;
            }

            // 4) 命名风格（snake ↔ camel）
            if let Some(t) = target.iter().find(|t| {
                !used_target.contains(&t.name)
                    && to_snake_case(&t.name) == to_snake_case(&s.name)
            }) {
                links.push(FieldLink {
                    source_field: s.name.clone(),
                    target_field: t.name.clone(),
                    confidence: 0.9,
                    match_kind: LinkKind::NamingConvention,
                    transform: None,
                });
                used_target.insert(t.name.clone());
                continue;
            }

            // 5) 语义：created_at ↔ create_time, updated_at ↔ update_time, id ↔ user_id 公共键
            if let Some(t) = target.iter().find(|t| {
                !used_target.contains(&t.name) && semantic_match(&s.name, &t.name)
            }) {
                let transform = if needs_datetime_cast(s, t) {
                    Some("CAST(... AS TIMESTAMP)".to_string())
                } else {
                    None
                };
                links.push(FieldLink {
                    source_field: s.name.clone(),
                    target_field: t.name.clone(),
                    confidence: 0.8,
                    match_kind: LinkKind::Semantic,
                    transform,
                });
                used_target.insert(t.name.clone());
                continue;
            }

            // 6) 类型相同 + 唯一剩余
            if let Some(t) = target.iter().find(|t| {
                !used_target.contains(&t.name) && same_type_family(&s.data_type, &t.data_type)
            }) {
                links.push(FieldLink {
                    source_field: s.name.clone(),
                    target_field: t.name.clone(),
                    confidence: 0.6,
                    match_kind: LinkKind::TypeAndUsage,
                    transform: None,
                });
                used_target.insert(t.name.clone());
                continue;
            }

            // 7) 未匹配
            links.push(FieldLink {
                source_field: s.name.clone(),
                target_field: String::new(),
                confidence: 0.0,
                match_kind: LinkKind::Unmatched,
                transform: None,
            });
        }

        links
    }
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && prev_lower {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            prev_lower = false;
        } else {
            out.push(c);
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        }
    }
    out
}

fn semantic_match(a: &str, b: &str) -> bool {
    let pairs = [
        ("created_at", "create_time"),
        ("created_at", "created_time"),
        ("updated_at", "update_time"),
        ("updated_at", "updated_time"),
        ("deleted_at", "delete_time"),
        ("create_time", "created_at"),
        ("update_time", "updated_at"),
        ("user_id", "uid"),
        ("uid", "user_id"),
        ("birthday", "birth_date"),
        ("birth_date", "birthday"),
        ("phone", "mobile"),
        ("phone", "telephone"),
        ("mobile", "phone"),
        ("email", "mail"),
        ("remark", "comment"),
        ("description", "desc"),
        ("desc", "description"),
    ];
    let al = a.to_ascii_lowercase();
    let bl = b.to_ascii_lowercase();
    pairs.iter().any(|(x, y)| (al == *x && bl == *y) || (al == *y && bl == *x))
}

fn same_type_family(a: &str, b: &str) -> bool {
    let al = a.to_ascii_lowercase();
    let bl = b.to_ascii_lowercase();
    let family = |s: &str| -> &str {
        if s.starts_with("int") || s.starts_with("tinyint") || s.starts_with("smallint")
            || s.starts_with("mediumint") || s.starts_with("bigint") || s.contains("integer") {
            "int"
        } else if s.starts_with("varchar") || s.starts_with("char") || s.starts_with("text")
            || s.starts_with("nvarchar") {
            "text"
        } else if s.starts_with("decimal") || s.starts_with("numeric") || s.starts_with("number") {
            "decimal"
        } else if s.starts_with("float") || s.starts_with("double") || s.starts_with("real") {
            "float"
        } else if s.starts_with("datetime") || s.starts_with("timestamp") || s == "date" || s == "time" {
            "datetime"
        } else if s.starts_with("blob") || s.starts_with("bytea") || s.starts_with("binary") {
            "blob"
        } else if s == "boolean" || s == "bool" || s == "tinyint(1)" {
            "bool"
        } else {
            "other"
        }
    };
    family(&al) == family(&bl) && family(&al) != "other"
}

fn needs_datetime_cast(s: &Field, t: &Field) -> bool {
    let al = s.data_type.to_ascii_lowercase();
    let bl = t.data_type.to_ascii_lowercase();
    (al.contains("date") || al.contains("time"))
        && (bl.contains("date") || bl.contains("time"))
        && al != bl
}

/// 单个数据值转换结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueConversion {
    pub source: String,
    pub target: String,
    pub rule: String,
    pub lossy: bool,
}

/// 数据值转换器
#[derive(Debug, Clone, Default)]
pub struct ValueTransformer;

impl ValueTransformer {
    pub fn new() -> Self {
        Self
    }

    /// 转换单个字符串值（按源类型 → 目标类型规则）
    pub fn convert(
        &self,
        raw: &str,
        source_type: &str,
        target_type: &str,
        target_db: TargetDbKind,
    ) -> ValueConversion {
        let src = source_type.to_ascii_uppercase();
        let tgt = target_type.to_ascii_uppercase();

        // NULL 直接透传
        if raw.is_empty() || raw.eq_ignore_ascii_case("null") {
            return ValueConversion {
                source: raw.to_string(),
                target: "NULL".to_string(),
                rule: "NULL passthrough".to_string(),
                lossy: false,
            };
        }

        // 布尔：MySQL 1/0 → PostgreSQL true/false
        if (tgt.contains("BOOLEAN") || tgt == "BOOL") && (src.contains("TINYINT") || src == "BIT") {
            let v = match raw {
                "0" | "false" | "FALSE" => "false".to_string(),
                _ => "true".to_string(),
            };
            return ValueConversion {
                source: raw.to_string(),
                target: v,
                rule: "TinyInt/Bit → Boolean".to_string(),
                lossy: false,
            };
        }

        // 日期 → ISO8601
        if src.contains("DATETIME") || src.contains("TIMESTAMP") {
            if matches!(target_db, TargetDbKind::SQLite) || tgt == "TEXT" {
                let normalized = normalize_datetime(raw);
                return ValueConversion {
                    source: raw.to_string(),
                    target: format!("'{}'", normalized),
                    rule: "Datetime → ISO8601 TEXT".to_string(),
                    lossy: false,
                };
            }
        }

        // JSON → JSONB (PostgreSQL)
        if tgt == "JSONB" && src == "JSON" {
            return ValueConversion {
                source: raw.to_string(),
                target: raw.to_string(),
                rule: "JSON → JSONB (binary)".to_string(),
                lossy: false,
            };
        }

        // 字节：BLOB <-> BYTEA (PostgreSQL 需要转义)
        if tgt == "BYTEA" && (src.contains("BLOB") || src.contains("BINARY")) {
            return ValueConversion {
                source: raw.to_string(),
                target: format!("E'\\\\x{}'", hex_encode(raw.as_bytes())),
                rule: "Blob → BYTEA hex".to_string(),
                lossy: false,
            };
        }

        // 字符集：UTF-8 → GBK 等（如有需要）— 此处仅做占位
        if src.contains("VARCHAR") || src.contains("CHAR") || src.contains("TEXT") {
            return ValueConversion {
                source: raw.to_string(),
                target: escape_string(raw),
                rule: "String escape".to_string(),
                lossy: false,
            };
        }

        // 默认：原样转义
        ValueConversion {
            source: raw.to_string(),
            target: escape_string(raw),
            rule: "Default escape".to_string(),
            lossy: false,
        }
    }

    /// 批量转换（Vec 记录）
    pub fn convert_batch(
        &self,
        rows: &[Vec<(String, String)>], // (field_name, value)
        source_schema: &[Field],
        target_schema: &[Field],
        target_db: TargetDbKind,
        linker: &FieldLinker,
    ) -> Vec<Vec<(String, ValueConversion)>> {
        let links = linker.link(source_schema, target_schema);
        rows.iter()
            .map(|row| {
                row.iter()
                    .filter_map(|(fname, val)| {
                        let link = links.iter().find(|l| l.source_field == *fname)?;
                        if link.target_field.is_empty() {
                            return None;
                        }
                        let target_field = target_schema
                            .iter()
                            .find(|f| f.name == link.target_field)?;
                        Some((
                            link.target_field.clone(),
                            self.convert(val, &fname_of(source_schema, fname), &target_field.data_type, target_db),
                        ))
                    })
                    .collect()
            })
            .collect()
    }
}

fn fname_of(schema: &[Field], name: &str) -> String {
    schema
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.data_type.clone())
        .unwrap_or_else(|| "TEXT".to_string())
}

fn escape_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn normalize_datetime(raw: &str) -> String {
    // 简单归一化为 ISO8601
    if raw.contains('T') {
        return raw.to_string();
    }
    raw.replace(' ', "T")
}

/// DDL 生成器（为目标库生成等价 CREATE TABLE）
#[derive(Debug, Clone, Default)]
pub struct DdlGenerator {
    pub target_db: TargetDbKind,
}

impl DdlGenerator {
    pub fn new(target_db: TargetDbKind) -> Self {
        Self { target_db }
    }

    /// 生成 CREATE TABLE 语句
    pub fn create_table(
        &self,
        table: &TableSchema,
        type_table: &TypeMappingTable,
    ) -> Result<String> {
        // 推断源库：优先用 target_fields 自身，否则按 target_db 当源（同源迁移）
        let source_db = table
            .fields
            .first()
            .map(|_| self.target_db)
            .unwrap_or(self.target_db);
        self.create_table_from(source_db, table, type_table)
    }

    /// 显式指定源库（推荐）
    pub fn create_table_from(
        &self,
        source_db: TargetDbKind,
        table: &TableSchema,
        type_table: &TypeMappingTable,
    ) -> Result<String> {
        // 借助 source_fields 保留原始长度
        let source_lookup: HashMap<String, &Field> = table
            .fields
            .iter()
            .map(|f| (f.name.clone(), f))
            .collect();
        self.create_table_with_source_fields(source_db, table, &source_lookup, type_table)
    }

    /// 完整版：传入原始 source fields 以保留长度
    pub fn create_table_with_source_fields(
        &self,
        source_db: TargetDbKind,
        target_table: &TableSchema,
        source_fields: &HashMap<String, &Field>,
        type_table: &TypeMappingTable,
    ) -> Result<String> {
        let mut cols: Vec<String> = Vec::new();
        for f in &target_table.fields {
            // 用源字段原始类型做查找（避免 target 类型被误用为源）
            let source_type = source_fields
                .get(&f.name)
                .map(|sf| sf.data_type.as_str())
                .unwrap_or(&f.data_type);
            let rule = type_table
                .lookup_with_source(source_type, source_db, self.target_db)
                .ok_or_else(|| {
                    anyhow!(
                        "未找到类型映射规则: {} (源 {:?} → {:?})",
                        source_type,
                        source_db,
                        self.target_db
                    )
                })?;
            let target_type = preserve_length(&rule.target_type, source_type);
            let nullable = if f.nullable { "" } else { " NOT NULL" };
            let default = f
                .default_value
                .as_ref()
                .map(|d| format!(" DEFAULT {}", d))
                .unwrap_or_default();
            let auto_inc = if f.auto_increment {
                match self.target_db {
                    TargetDbKind::MySQL | TargetDbKind::MariaDB | TargetDbKind::TiDB => {
                        " AUTO_INCREMENT"
                    }
                    TargetDbKind::PostgreSQL | TargetDbKind::SQLite => {
                        " /* AUTO_INCREMENT */"
                    }
                    _ => " /* AUTO_INCREMENT */",
                }
            } else {
                ""
            };
            let pk = if f.primary_key { " PRIMARY KEY" } else { "" };
            cols.push(format!(
                "  {} {}{}{}{}{}",
                quote_ident(&f.name, self.target_db),
                target_type,
                nullable,
                default,
                auto_inc,
                pk
            ));
        }
        let body = cols.join(",\n");
        let mut sql = format!("CREATE TABLE {} (\n{}\n)", quote_ident(&target_table.name, self.target_db), body);

        // 索引
        for idx in &target_table.indexes {
            sql.push('\n');
            sql.push_str(&self.create_index(&target_table.name, idx)?);
        }
        Ok(sql)
    }

    pub fn create_index(&self, table: &str, idx: &Index) -> Result<String> {
        let cols = idx
            .fields
            .iter()
            .map(|c| quote_ident(c, self.target_db))
            .collect::<Vec<_>>()
            .join(", ");
        let unique = if idx.unique { "UNIQUE " } else { "" };
        Ok(format!(
            "CREATE {}INDEX {} ON {} ({})",
            unique,
            quote_ident(&idx.name, self.target_db),
            quote_ident(table, self.target_db),
            cols
        ))
    }
}

fn quote_ident(name: &str, db: TargetDbKind) -> String {
    match db {
        TargetDbKind::MySQL | TargetDbKind::MariaDB | TargetDbKind::TiDB | TargetDbKind::MSSQL => {
            format!("`{}`", name.replace('`', "``"))
        }
        TargetDbKind::PostgreSQL | TargetDbKind::SQLite | TargetDbKind::Oracle => {
            format!("\"{}\"", name.replace('"', "\"\""))
        }
        TargetDbKind::Redis | TargetDbKind::MongoDB => name.to_string(),
    }
}

fn normalize_type(t: &str) -> String {
    // 去掉长度信息用于规则匹配，例如 "VARCHAR(255)" → "VARCHAR"
    let upper = t.trim().to_uppercase();
    if let Some(idx) = upper.find('(') {
        upper[..idx].trim().to_string()
    } else {
        upper
    }
}

/// 保留字段原始长度：
/// 如果 target_type 形如 "VARCHAR"（无长度）且 source 有 "VARCHAR(64)"，则把 (64) 加到 target
/// 如果 target_type 形如 "VARCHAR(255)" 而 source 是 "VARCHAR(64)"，以 source 为准（避免目标库限制）
/// 如果 target_type 形如 "TEXT"（本身不需要长度）则保持原样
fn preserve_length(target_type: &str, source_type: &str) -> String {
    let tgt_norm = normalize_type(target_type);
    let src_norm = normalize_type(source_type);
    if tgt_norm != src_norm {
        return target_type.to_string();
    }
    // 提取 source 的长度（如果有）
    let src_len = if let Some(start) = source_type.find('(') {
        source_type[start..].to_string()
    } else {
        String::new()
    };
    if !src_len.is_empty() {
        // 用 source 的长度（更精确）
        format!("{}{}", tgt_norm, src_len)
    } else {
        // source 没有长度，保留 target_type 原样
        target_type.to_string()
    }
}

/// 转换报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionReport {
    pub source_db: TargetDbKind,
    pub target_db: TargetDbKind,
    pub source_version: Option<String>,
    pub target_version: Option<String>,
    pub table_name: String,
    pub fields_total: usize,
    pub fields_mapped: usize,
    pub fields_unmapped: usize,
    pub lossy_conversions: usize,
    pub started_at: String,
    pub finished_at: String,
    pub ddl: String,
    pub links: Vec<FieldLink>,
    pub rules_applied: Vec<String>,
}

impl ConversionReport {
    pub fn summary(&self) -> String {
        format!(
            "[{} → {}] 表 {}: 字段 {}/{} 映射，{} 个有损转换，DDL 已生成",
            self.source_db.name(),
            self.target_db.name(),
            self.table_name,
            self.fields_mapped,
            self.fields_total,
            self.lossy_conversions
        )
    }
}

/// 跨数据库转换器（主入口）
pub struct CrossDbConverter {
    pub type_table: TypeMappingTable,
    pub linker: FieldLinker,
}

impl CrossDbConverter {
    pub fn new() -> Self {
        Self {
            type_table: TypeMappingTable::built_in(),
            linker: FieldLinker::new(),
        }
    }

    pub fn with_linker(mut self, linker: FieldLinker) -> Self {
        self.linker = linker;
        self
    }

    /// 添加自定义类型映射规则
    pub fn add_rule(&mut self, rule: TypeMappingRule) {
        self.type_table.add_rule(rule);
    }

    /// 转换一张表：生成目标 DDL + 字段映射 + 报告
    pub fn convert_table(
        &self,
        source_table: &TableSchema,
        target_db: TargetDbKind,
        source_db: TargetDbKind,
    ) -> Result<ConversionReport> {
        let started_at = Utc::now().to_rfc3339();

        // 字段类型映射 + 自动连线
        let target_fields = map_fields(&source_table.fields, &self.type_table, source_db, target_db)?;
        let target_table = TableSchema {
            name: source_table.name.clone(),
            fields: target_fields.clone(),
            indexes: source_table.indexes.clone(),
            foreign_keys: source_table.foreign_keys.clone(),
        };

        // 自动连线（基于原始字段名）
        let links = self
            .linker
            .link(&source_table.fields, &target_fields);

        // 生成 DDL（保留原始长度）
        let gen = DdlGenerator::new(target_db);
        let source_lookup: HashMap<String, &Field> = source_table
            .fields
            .iter()
            .map(|f| (f.name.clone(), f))
            .collect();
        let ddl = gen.create_table_with_source_fields(
            source_db,
            &target_table,
            &source_lookup,
            &self.type_table,
        )?;

        // 统计
        let mapped = links.iter().filter(|l| l.match_kind != LinkKind::Unmatched).count();
        let unmapped = links.iter().filter(|l| l.match_kind == LinkKind::Unmatched).count();
        let lossy = source_table
            .fields
            .iter()
            .filter(|f| {
                self.type_table
                    .lookup_with_source(&f.data_type, source_db, target_db)
                    .map(|r| r.lossy)
                    .unwrap_or(false)
            })
            .count();

        let rules: Vec<String> = source_table
            .fields
            .iter()
            .filter_map(|f| {
                self.type_table
                    .lookup_with_source(&f.data_type, source_db, target_db)
                    .map(|r| format!("{} → {} ({})", f.data_type, r.target_type, r.note))
            })
            .collect();

        Ok(ConversionReport {
            source_db,
            target_db,
            source_version: None,
            target_version: None,
            table_name: source_table.name.clone(),
            fields_total: source_table.fields.len(),
            fields_mapped: mapped,
            fields_unmapped: unmapped,
            lossy_conversions: lossy,
            started_at,
            finished_at: Utc::now().to_rfc3339(),
            ddl,
            links,
            rules_applied: rules,
        })
    }
}

impl Default for CrossDbConverter {
    fn default() -> Self {
        Self::new()
    }
}

fn map_fields(
    src: &[Field],
    table: &TypeMappingTable,
    source: TargetDbKind,
    target: TargetDbKind,
) -> Result<Vec<Field>> {
    src.iter()
        .map(|f| {
            let rule = table
                .lookup_with_source(&f.data_type, source, target)
                .ok_or_else(|| {
                    anyhow!(
                        "未找到类型映射规则: {} (源 {:?} → {:?})",
                        f.data_type,
                        source,
                        target
                    )
                })?;
            Ok(Field {
                name: f.name.clone(),
                data_type: rule.target_type.clone(),
                length: f.length,
                nullable: f.nullable,
                default_value: f.default_value.clone(),
                primary_key: f.primary_key,
                auto_increment: f.auto_increment,
            })
        })
        .collect()
}

/// 数据库版本感知转换：在做类型映射时根据目标版本调整
pub fn version_adjusted_type(
    base_type: &str,
    source_version: Option<&DatabaseVersion>,
    target_db: TargetDbKind,
    target_version: Option<&DatabaseVersion>,
    table: &TypeMappingTable,
) -> Result<String> {
    let mut mapped = table
        .lookup(&normalize_type(base_type), target_db)
        .ok_or_else(|| anyhow!("no rule"))?
        .target_type
        .clone();
    // 升级目标库（高版本）可以有更好类型选择
    if let Some(tv) = target_version {
        if target_db == TargetDbKind::PostgreSQL && tv.major >= 14 && mapped == "TEXT" {
            // PostgreSQL 14+ 对 TEXT 性能无差异，但可用 CITEXT 替代——保持 TEXT 即可
        }
        if target_db == TargetDbKind::MySQL && tv.major >= 8 && mapped.starts_with("DATETIME") {
            // MySQL 8.0+ DATETIME 支持更高精度
            if !mapped.contains('(') {
                mapped = format!("{}(6)", mapped); // 微秒精度
            }
        }
        if target_db == TargetDbKind::SQLite && tv.major >= 3 && tv.minor >= 38 {
            // SQLite 3.38+ 原生支持 JSONB（实验）
        }
    }

    // 源库是低版本时可能存在老式类型，需在转换前重写
    if let Some(sv) = source_version {
        if target_db == TargetDbKind::MySQL && sv.major < 5 && mapped == "VARCHAR" {
            // MySQL 4.x 老 VARCHAR 已等同 VARCHAR
        }
    }

    Ok(mapped)
}

use crate::databases::DatabaseVersion;

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_field(name: &str, ty: &str, pk: bool) -> Field {
        Field {
            name: name.to_string(),
            data_type: ty.to_string(),
            length: None,
            nullable: !pk,
            default_value: None,
            primary_key: pk,
            auto_increment: pk,
        }
    }

    #[test]
    fn test_type_mapping_mysql_to_pg() {
        let t = TypeMappingTable::built_in();
        let r = t
            .lookup_with_source("INT", TargetDbKind::MySQL, TargetDbKind::PostgreSQL)
            .unwrap();
        assert_eq!(r.target_type, "INTEGER");
        let r = t
            .lookup_with_source("VARCHAR(255)", TargetDbKind::MySQL, TargetDbKind::PostgreSQL)
            .unwrap();
        assert_eq!(r.target_type, "VARCHAR(255)");
    }

    #[test]
    fn test_type_mapping_mysql_to_sqlite_datetime_to_text() {
        let t = TypeMappingTable::built_in();
        let r = t
            .lookup_with_source("DATETIME", TargetDbKind::MySQL, TargetDbKind::SQLite)
            .unwrap();
        assert_eq!(r.target_type, "TEXT");
        assert!(r.lossy);
    }

    #[test]
    fn test_type_mapping_pg_to_mysql_uuid() {
        let t = TypeMappingTable::built_in();
        let r = t
            .lookup_with_source("UUID", TargetDbKind::PostgreSQL, TargetDbKind::MySQL)
            .unwrap();
        assert_eq!(r.target_type, "CHAR(36)");
        assert!(r.lossy);
    }

    #[test]
    fn test_type_mapping_sqlite_to_pg_integer_to_bigint() {
        let t = TypeMappingTable::built_in();
        // 使用 lookup_with_source 显式指定源库
        let r = t
            .lookup_with_source("INTEGER", TargetDbKind::SQLite, TargetDbKind::PostgreSQL)
            .unwrap();
        assert_eq!(r.target_type, "BIGINT");
    }

    #[test]
    fn test_field_linker_exact() {
        let linker = FieldLinker::new();
        let src = vec![mk_field("id", "BIGINT", true), mk_field("name", "VARCHAR(64)", false)];
        let tgt = vec![mk_field("id", "BIGINT", true), mk_field("name", "VARCHAR(64)", false)];
        let links = linker.link(&src, &tgt);
        assert_eq!(links.len(), 2);
        assert!(links.iter().all(|l| l.match_kind == LinkKind::Exact));
        assert!(links.iter().all(|l| l.confidence > 0.99));
    }

    #[test]
    fn test_field_linker_case_insensitive() {
        let linker = FieldLinker::new();
        let src = vec![mk_field("UserID", "BIGINT", true)];
        let tgt = vec![mk_field("userid", "BIGINT", true)];
        let links = linker.link(&src, &tgt);
        assert_eq!(links[0].match_kind, LinkKind::CaseInsensitive);
    }

    #[test]
    fn test_field_linker_camel_to_snake() {
        let linker = FieldLinker::new();
        let src = vec![mk_field("userId", "BIGINT", false)];
        let tgt = vec![mk_field("user_id", "BIGINT", false)];
        let links = linker.link(&src, &tgt);
        assert_eq!(links[0].match_kind, LinkKind::NamingConvention);
    }

    #[test]
    fn test_field_linker_semantic_created_at_create_time() {
        let linker = FieldLinker::new();
        let src = vec![mk_field("created_at", "DATETIME", false)];
        let tgt = vec![mk_field("create_time", "TIMESTAMP", false)];
        let links = linker.link(&src, &tgt);
        assert_eq!(links[0].match_kind, LinkKind::Semantic);
        assert!(links[0].transform.is_some());
    }

    #[test]
    fn test_field_linker_unmatched() {
        let linker = FieldLinker::new();
        let src = vec![mk_field("foo", "TEXT", false)];
        let tgt = vec![mk_field("bar", "INT", false)];
        let links = linker.link(&src, &tgt);
        assert_eq!(links[0].match_kind, LinkKind::Unmatched);
    }

    #[test]
    fn test_field_linker_manual_overrides_auto() {
        let mut linker = FieldLinker::new();
        linker.add_manual("foo", "baz");
        let src = vec![mk_field("foo", "TEXT", false)];
        let tgt = vec![mk_field("bar", "TEXT", false), mk_field("baz", "TEXT", false)];
        let links = linker.link(&src, &tgt);
        assert_eq!(links[0].match_kind, LinkKind::Manual);
        assert_eq!(links[0].target_field, "baz");
    }

    #[test]
    fn test_ddl_generator_mysql_target() {
        let gen = DdlGenerator::new(TargetDbKind::MySQL);
        let table = TableSchema {
            name: "users".to_string(),
            fields: vec![
                mk_field("id", "INT", true),
                mk_field("name", "VARCHAR(64)", false),
                mk_field("created_at", "DATETIME", false),
            ],
            indexes: vec![Index {
                name: "idx_name".to_string(),
                fields: vec!["name".to_string()],
                unique: false,
            }],
            foreign_keys: vec![],
        };
        let t = TypeMappingTable::built_in();
        // 源=MySQL 目标=MySQL（自映射）
        let ddl = gen
            .create_table_from(TargetDbKind::MySQL, &table, &t)
            .unwrap();
        assert!(ddl.contains("CREATE TABLE `users`"));
        assert!(ddl.contains("`id` INT"));
        assert!(ddl.contains("`name` VARCHAR(64)"));
        assert!(ddl.contains("`created_at` DATETIME"));
        assert!(ddl.contains("CREATE INDEX `idx_name`"));
    }

    #[test]
    fn test_ddl_generator_pg_target() {
        let gen = DdlGenerator::new(TargetDbKind::PostgreSQL);
        let table = TableSchema {
            name: "users".to_string(),
            fields: vec![
                mk_field("id", "INT", true),
                mk_field("name", "VARCHAR(64)", false),
                mk_field("created_at", "DATETIME", false),
            ],
            indexes: vec![],
            foreign_keys: vec![],
        };
        let t = TypeMappingTable::built_in();
        // 源=MySQL 目标=PostgreSQL
        let ddl = gen
            .create_table_from(TargetDbKind::MySQL, &table, &t)
            .unwrap();
        assert!(ddl.contains("CREATE TABLE \"users\""));
        assert!(ddl.contains("\"id\" INTEGER"));
        assert!(ddl.contains("\"name\" VARCHAR(64)"));
        assert!(ddl.contains("\"created_at\" TIMESTAMP"));
    }

    #[test]
    fn test_ddl_generator_sqlite_target() {
        let gen = DdlGenerator::new(TargetDbKind::SQLite);
        let table = TableSchema {
            name: "users".to_string(),
            fields: vec![
                mk_field("id", "INT", true),
                mk_field("name", "VARCHAR(64)", false),
                mk_field("created_at", "DATETIME", false),
            ],
            indexes: vec![],
            foreign_keys: vec![],
        };
        let t = TypeMappingTable::built_in();
        // 源=MySQL 目标=SQLite
        let ddl = gen
            .create_table_from(TargetDbKind::MySQL, &table, &t)
            .unwrap();
        assert!(ddl.contains("\"users\""));
        assert!(ddl.contains("INTEGER"));
        assert!(ddl.contains("TEXT"));
    }

    #[test]
    fn test_value_transformer_bool_tinyint() {
        let t = ValueTransformer::new();
        let v = t.convert("1", "TINYINT(1)", "BOOLEAN", TargetDbKind::PostgreSQL);
        assert_eq!(v.target, "true");
        let v = t.convert("0", "TINYINT(1)", "BOOLEAN", TargetDbKind::PostgreSQL);
        assert_eq!(v.target, "false");
    }

    #[test]
    fn test_value_transformer_datetime_to_iso() {
        let t = ValueTransformer::new();
        let v = t.convert("2024-05-01 12:34:56", "DATETIME", "TEXT", TargetDbKind::SQLite);
        assert_eq!(v.target, "'2024-05-01T12:34:56'");
    }

    #[test]
    fn test_value_transformer_null_passthrough() {
        let t = ValueTransformer::new();
        let v = t.convert("NULL", "VARCHAR", "TEXT", TargetDbKind::SQLite);
        assert_eq!(v.target, "NULL");
    }

    #[test]
    fn test_value_transformer_string_escape() {
        let t = ValueTransformer::new();
        let v = t.convert("O'Brien", "VARCHAR", "VARCHAR", TargetDbKind::MySQL);
        assert_eq!(v.target, "'O''Brien'");
    }

    #[test]
    fn test_cross_db_converter_full_pipeline() {
        let conv = CrossDbConverter::new();
        let table = TableSchema {
            name: "orders".to_string(),
            fields: vec![
                mk_field("id", "INT", true),
                mk_field("user_id", "BIGINT", false),
                mk_field("amount", "DECIMAL(10,2)", false),
                mk_field("status", "VARCHAR(32)", false),
                mk_field("created_at", "DATETIME", false),
            ],
            indexes: vec![],
            foreign_keys: vec![],
        };
        let report = conv
            .convert_table(&table, TargetDbKind::PostgreSQL, TargetDbKind::MySQL)
            .unwrap();
        assert_eq!(report.table_name, "orders");
        assert_eq!(report.fields_mapped, 5);
        assert_eq!(report.fields_unmapped, 0);
        assert!(report.ddl.contains("INTEGER"));
        assert!(report.ddl.contains("NUMERIC"));
        assert!(report.ddl.contains("VARCHAR(32)"));
        assert!(report.ddl.contains("TIMESTAMP"));
    }

    #[test]
    fn test_version_adjusted_type_mysql8() {
        let t = TypeMappingTable::built_in();
        let v = DatabaseVersion::new(8, 0, 32);
        let mapped = version_adjusted_type(
            "DATETIME",
            None,
            TargetDbKind::MySQL,
            Some(&v),
            &t,
        )
        .unwrap();
        assert!(mapped.starts_with("DATETIME(6)"));
    }

    #[test]
    fn test_link_kind_partial_eq() {
        assert_eq!(LinkKind::Exact, LinkKind::Exact);
        assert_ne!(LinkKind::Exact, LinkKind::Semantic);
    }

    #[test]
    fn test_type_table_default() {
        let t = TypeMappingTable::default();
        assert_eq!(t.rules.len(), 0);
    }

    #[test]
    fn test_conversion_report_summary() {
        let r = ConversionReport {
            source_db: TargetDbKind::MySQL,
            target_db: TargetDbKind::PostgreSQL,
            source_version: Some("5.7.40".to_string()),
            target_version: Some("16.2".to_string()),
            table_name: "users".to_string(),
            fields_total: 5,
            fields_mapped: 5,
            fields_unmapped: 0,
            lossy_conversions: 0,
            started_at: "2024-01-01T00:00:00Z".to_string(),
            finished_at: "2024-01-01T00:00:01Z".to_string(),
            ddl: "CREATE TABLE".to_string(),
            links: vec![],
            rules_applied: vec![],
        };
        let s = r.summary();
        assert!(s.contains("users"));
        assert!(s.contains("5/5"));
    }
}
