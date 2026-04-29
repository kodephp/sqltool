/// 数据对比模块
/// 用于对比两个数据库或表之间的结构和数据差异

use crate::databases::DatabaseConnection;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Map;

/// 数据对比配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCompareConfig {
    /// 对比模式
    pub compare_mode: CompareMode,
    /// 忽略的字段列表
    pub ignore_fields: Vec<String>,
    /// 比较字段
    pub compare_fields: Vec<String>,
    /// 主键字段
    pub primary_key: String,
    /// 忽略大小写
    pub ignore_case: bool,
    /// 忽略空白字符
    pub ignore_whitespace: bool,
    /// 数值容差(用于浮点数比较)
    pub numeric_tolerance: f64,
    /// 比较时间戳(忽略毫秒)
    pub ignore_timestamp_ms: bool,
    /// 最大差异数量限制
    pub max_diff_count: usize,
}

impl Default for DataCompareConfig {
    fn default() -> Self {
        Self {
            compare_mode: CompareMode::Full,
            ignore_fields: vec![
                "updated_at".to_string(),
                "created_at".to_string(),
                "modified_at".to_string(),
            ],
            compare_fields: Vec::new(),
            primary_key: "id".to_string(),
            ignore_case: false,
            ignore_whitespace: true,
            numeric_tolerance: 0.0001,
            ignore_timestamp_ms: true,
            max_diff_count: 10000,
        }
    }
}

/// 对比模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompareMode {
    /// 完全对比
    Full,
    /// 仅对比结构
    SchemaOnly,
    /// 仅对比数据
    DataOnly,
    /// 快速对比(抽样)
    Quick,
    /// 仅对比差异
    DiffOnly,
}

/// 对比结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareResult {
    /// 是否完全一致
    pub is_identical: bool,
    /// 源数据库/表
    pub source: String,
    /// 目标数据库/表
    pub target: String,
    /// 结构差异
    pub schema_diffs: Vec<SchemaDiff>,
    /// 数据差异
    pub data_diffs: Vec<DataDiff>,
    /// 缺失的记录(源有目标没有)
    pub missing_in_target: Vec<RowDiff>,
    /// 缺失的记录(目标有源没有)
    pub missing_in_source: Vec<RowDiff>,
    /// 统计信息
    pub stats: CompareStats,
    /// 对比耗时(毫秒)
    pub duration_ms: u64,
    /// 错误信息
    pub errors: Vec<String>,
}

/// 结构差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDiff {
    /// 差异类型
    pub diff_type: SchemaDiffType,
    /// 对象类型(表、列、索引等)
    pub object_type: String,
    /// 对象名称
    pub object_name: String,
    /// 字段名称(如果是列差异)
    pub field_name: Option<String>,
    /// 源定义
    pub source_definition: Option<String>,
    /// 目标定义
    pub target_definition: Option<String>,
    /// 差异描述
    pub description: String,
}

/// 结构差异类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaDiffType {
    /// 表不存在
    TableMissingInTarget,
    /// 表不存在于源
    TableMissingInSource,
    /// 列类型不匹配
    ColumnTypeMismatch,
    /// 列长度不匹配
    ColumnLengthMismatch,
    /// 列 nullable 不匹配
    ColumnNullableMismatch,
    /// 列默认值不匹配
    ColumnDefaultMismatch,
    /// 列不存在于源
    ColumnMissingInSource,
    /// 列不存在于目标
    ColumnMissingInTarget,
    /// 索引差异
    IndexDifference,
    /// 外键差异
    ForeignKeyDifference,
    /// 约束差异
    ConstraintDifference,
}

/// 数据差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDiff {
    /// 差异类型
    pub diff_type: DataDiffType,
    /// 表名
    pub table_name: String,
    /// 主键值
    pub primary_key_value: String,
    /// 字段名称
    pub field_name: String,
    /// 源值
    pub source_value: Option<String>,
    /// 目标值
    pub target_value: Option<String>,
    /// 差异描述
    pub description: String,
}

/// 数据差异类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataDiffType {
    /// 值不同
    ValueMismatch,
    /// 源有目标没有
    MissingInTarget,
    /// 目标有源没有
    MissingInSource,
    /// 类型不匹配
    TypeMismatch,
    /// 空值差异
    NullMismatch,
}

/// 行差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowDiff {
    /// 表名
    pub table_name: String,
    /// 主键值
    pub primary_key_value: String,
    /// 完整行数据
    pub row_data: Map<String, serde_json::Value>,
}

/// 对比统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareStats {
    /// 源表行数
    pub source_rows: u64,
    /// 目标表行数
    pub target_rows: u64,
    /// 匹配行数
    pub matched_rows: u64,
    /// 差异行数
    pub diff_rows: u64,
    /// 缺失行数(源有目标没有)
    pub missing_in_target_count: u64,
    /// 缺失行数(目标有源没有)
    pub missing_in_source_count: u64,
    /// 源表大小(字节)
    pub source_size_bytes: u64,
    /// 目标表大小(字节)
    pub target_size_bytes: u64,
    /// 匹配百分比
    pub match_percentage: f64,
}

/// 数据对比器
pub struct DataComparer {
    source_connection: Box<dyn DatabaseConnection>,
    target_connection: Box<dyn DatabaseConnection>,
    config: DataCompareConfig,
}

impl DataComparer {
    /// 创建新的对比器
    pub fn new(
        source_connection: Box<dyn DatabaseConnection>,
        target_connection: Box<dyn DatabaseConnection>,
        config: DataCompareConfig,
    ) -> Self {
        Self {
            source_connection,
            target_connection,
            config,
        }
    }

    /// 执行表对比
    pub async fn compare_table(&mut self, table_name: &str) -> Result<CompareResult> {
        let start = std::time::Instant::now();
        let source_name = "source".to_string();
        let target_name = "target".to_string();

        let mut schema_diffs = Vec::new();
        let mut data_diffs = Vec::new();
        let mut missing_in_target = Vec::new();
        let mut missing_in_source = Vec::new();
        let mut errors = Vec::new();

        if let Err(e) = self.compare_table_schema(table_name, &mut schema_diffs).await {
            errors.push(format!("结构对比失败: {}", e));
        }

        let (source_rows, target_rows, matched, diff_count, missing_t, missing_s) =
            if matches!(self.config.compare_mode, CompareMode::SchemaOnly) {
                (0, 0, 0, 0, 0, 0)
            } else {
                match self.compare_table_data(table_name, &mut data_diffs, &mut missing_in_target, &mut missing_in_source).await {
                    Ok(stats) => (stats.0, stats.1, stats.2, stats.3, stats.4, stats.5),
                    Err(e) => {
                        errors.push(format!("数据对比失败: {}", e));
                        (0, 0, 0, 0, 0, 0)
                    }
                }
            };

        let total_rows = source_rows.max(target_rows);
        let match_percentage = if total_rows > 0 {
            (matched as f64 / total_rows as f64) * 100.0
        } else {
            100.0
        };

        Ok(CompareResult {
            is_identical: schema_diffs.is_empty() && data_diffs.is_empty() && missing_in_target.is_empty() && missing_in_source.is_empty(),
            source: format!("{}.{}", source_name, table_name),
            target: format!("{}.{}", target_name, table_name),
            schema_diffs,
            data_diffs,
            missing_in_target,
            missing_in_source,
            stats: CompareStats {
                source_rows,
                target_rows,
                matched_rows: matched,
                diff_rows: diff_count,
                missing_in_target_count: missing_t,
                missing_in_source_count: missing_s,
                source_size_bytes: 0,
                target_size_bytes: 0,
                match_percentage,
            },
            duration_ms: start.elapsed().as_millis() as u64,
            errors,
        })
    }

    /// 比较表结构
    async fn compare_table_schema(&self, table_name: &str, diffs: &mut Vec<SchemaDiff>) -> Result<()> {
        let source_schema = self.source_connection.get_table_schema(table_name).await?;
        let target_schema = self.target_connection.get_table_schema(table_name).await?;

        for source_field in &source_schema.fields {
            let column = &source_field.name;
            let source_def = format!("{}:{} nullable={}", source_field.data_type, source_field.length.map(|l| l.to_string()).unwrap_or_default(), source_field.nullable);

            let target_field = target_schema.fields.iter().find(|f| f.name == *column);

            if let Some(target_f) = target_field {
                let target_def = format!("{}:{} nullable={}", target_f.data_type, target_f.length.map(|l| l.to_string()).unwrap_or_default(), target_f.nullable);
                if source_def != target_def {
                    diffs.push(SchemaDiff {
                        diff_type: SchemaDiffType::ColumnTypeMismatch,
                        object_type: "column".to_string(),
                        object_name: table_name.to_string(),
                        field_name: Some(column.clone()),
                        source_definition: Some(source_def),
                        target_definition: Some(target_def),
                        description: format!("列 {} 类型不匹配", column),
                    });
                }
            } else {
                diffs.push(SchemaDiff {
                    diff_type: SchemaDiffType::ColumnMissingInTarget,
                    object_type: "column".to_string(),
                    object_name: table_name.to_string(),
                    field_name: Some(column.clone()),
                    source_definition: Some(source_def),
                    target_definition: None,
                    description: format!("列 {} 存在于源但不存在于目标", column),
                });
            }
        }

        for target_field in &target_schema.fields {
            let column = &target_field.name;
            let source_exists = source_schema.fields.iter().any(|f| f.name == *column);
            if !source_exists {
                let target_def = format!("{}:{} nullable={}", target_field.data_type, target_field.length.map(|l| l.to_string()).unwrap_or_default(), target_field.nullable);
                diffs.push(SchemaDiff {
                    diff_type: SchemaDiffType::ColumnMissingInSource,
                    object_type: "column".to_string(),
                    object_name: table_name.to_string(),
                    field_name: Some(column.clone()),
                    source_definition: None,
                    target_definition: Some(target_def),
                    description: format!("列 {} 不存在于源但存在于目标", column),
                });
            }
        }

        Ok(())
    }

    /// 比较表数据
    async fn compare_table_data(
        &mut self,
        table_name: &str,
        data_diffs: &mut Vec<DataDiff>,
        missing_in_target: &mut Vec<RowDiff>,
        missing_in_source: &mut Vec<RowDiff>,
    ) -> Result<(u64, u64, u64, u64, u64, u64)> {
        let source_sql = format!("SELECT * FROM {}", table_name);
        let target_sql = format!("SELECT * FROM {}", table_name);

        let source_rows = self.source_connection.query(&source_sql).await?;
        let target_rows = self.target_connection.query(&target_sql).await?;

        let source_count = source_rows.len() as u64;
        let target_count = target_rows.len() as u64;

        let mut source_map: Map<String, serde_json::Value> = Map::new();
        let mut target_map: Map<String, serde_json::Value> = Map::new();

        for row in &source_rows {
            if let serde_json::Value::Object(obj) = row {
                let pk_value = obj.get(&self.config.primary_key)
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                source_map.insert(pk_value, serde_json::Value::Object(obj.clone()));
            }
        }

        for row in &target_rows {
            if let serde_json::Value::Object(obj) = row {
                let pk_value = obj.get(&self.config.primary_key)
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                target_map.insert(pk_value, serde_json::Value::Object(obj.clone()));
            }
        }

        let mut matched = 0u64;
        let mut diff_count = 0u64;

        for (pk, source_val) in &source_map {
            if let Some(target_val) = target_map.get(pk) {
                matched += 1;

                if let (serde_json::Value::Object(source_obj), serde_json::Value::Object(target_obj)) = (source_val, target_val) {
                    for (field, source_field_val) in source_obj {
                        if self.config.ignore_fields.contains(field) {
                            continue;
                        }

                        if let Some(target_field_val) = target_obj.get(field) {
                            if !self.values_equal(source_field_val, target_field_val) {
                                if data_diffs.len() < self.config.max_diff_count {
                                    data_diffs.push(DataDiff {
                                        diff_type: DataDiffType::ValueMismatch,
                                        table_name: table_name.to_string(),
                                        primary_key_value: pk.clone(),
                                        field_name: field.clone(),
                                        source_value: Some(source_field_val.to_string()),
                                        target_value: Some(target_field_val.to_string()),
                                        description: format!("字段 {} 值不匹配: {} vs {}", field, source_field_val, target_field_val),
                                    });
                                }
                                diff_count += 1;
                            }
                        }
                    }
                }
            } else {
                if let serde_json::Value::Object(obj) = source_val {
                    missing_in_target.push(RowDiff {
                        table_name: table_name.to_string(),
                        primary_key_value: pk.clone(),
                        row_data: obj.clone(),
                    });
                }
            }
        }

        for (pk, target_val) in &target_map {
            if !source_map.contains_key(pk) {
                if let serde_json::Value::Object(obj) = target_val {
                    missing_in_source.push(RowDiff {
                        table_name: table_name.to_string(),
                        primary_key_value: pk.clone(),
                        row_data: obj.clone(),
                    });
                }
            }
        }

        Ok((source_count, target_count, matched, diff_count, missing_in_target.len() as u64, missing_in_source.len() as u64))
    }

    /// 比较两个值是否相等
    fn values_equal(&self, a: &serde_json::Value, b: &serde_json::Value) -> bool {
        match (a, b) {
            (serde_json::Value::Null, serde_json::Value::Null) => true,
            (serde_json::Value::Null, _) | (_, serde_json::Value::Null) => false,
            (serde_json::Value::Number(a_num), serde_json::Value::Number(b_num)) => {
                if let (Some(a_f), Some(b_f)) = (a_num.as_f64(), b_num.as_f64()) {
                    (a_f - b_f).abs() < self.config.numeric_tolerance
                } else {
                    a_num == b_num
                }
            }
            (serde_json::Value::String(a_str), serde_json::Value::String(b_str)) => {
                if self.config.ignore_case {
                    a_str.to_lowercase() == b_str.to_lowercase()
                } else if self.config.ignore_whitespace {
                    a_str.trim() == b_str.trim()
                } else {
                    a_str == b_str
                }
            }
            _ => a == b,
        }
    }

    /// 生成同步SQL
    pub fn generate_sync_sql(&self, result: &CompareResult) -> Vec<String> {
        let mut sqls = Vec::new();

        for diff in &result.data_diffs {
            if diff.diff_type == DataDiffType::ValueMismatch {
                sqls.push(format!(
                    "UPDATE {} SET {} = {} WHERE {} = '{}';",
                    diff.table_name,
                    diff.field_name,
                    diff.target_value.as_deref().unwrap_or("NULL"),
                    self.config.primary_key,
                    diff.primary_key_value
                ));
            }
        }

        for row in &result.missing_in_target {
            let primary_key = &self.config.primary_key;
            let set_clauses: Vec<String> = row.row_data.iter()
                .filter(|(k, _)| k.as_str() != primary_key.as_str() && !self.config.ignore_fields.iter().any(|f| f.as_str() == k.as_str()))
                .map(|(k, v)| format!("{} = {}", k, self.value_to_sql(v)))
                .collect();

            if !set_clauses.is_empty() {
                sqls.push(format!(
                    "INSERT INTO {} ({}) VALUES ({});",
                    row.table_name,
                    set_clauses.iter().map(|s| s.split(" = ").next().unwrap_or("")).collect::<Vec<_>>().join(", "),
                    set_clauses.iter().map(|s| s.split(" = ").nth(1).unwrap_or("")).collect::<Vec<_>>().join(", ")
                ));
            }
        }

        sqls
    }

    fn value_to_sql(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "NULL".to_string(),
            serde_json::Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            _ => format!("'{}'", value.to_string().replace('\'', "''")),
        }
    }

    /// 生成差异报告
    pub fn generate_diff_report(&self, result: &CompareResult) -> String {
        let mut report = String::new();

        report.push_str(&format!("=== 数据对比报告 ===\n"));
        report.push_str(&format!("源: {}\n", result.source));
        report.push_str(&format!("目标: {}\n", result.target));
        report.push_str(&format!("是否一致: {}\n", if result.is_identical { "是" } else { "否" }));
        report.push_str(&format!("对比耗时: {}ms\n\n", result.duration_ms));

        report.push_str(&format!("--- 统计信息 ---\n"));
        report.push_str(&format!("源表行数: {}\n", result.stats.source_rows));
        report.push_str(&format!("目标表行数: {}\n", result.stats.target_rows));
        report.push_str(&format!("匹配行数: {}\n", result.stats.matched_rows));
        report.push_str(&format!("差异行数: {}\n", result.stats.diff_rows));
        report.push_str(&format!("目标缺失: {}\n", result.stats.missing_in_target_count));
        report.push_str(&format!("源缺失: {}\n", result.stats.missing_in_source_count));
        report.push_str(&format!("匹配率: {:.2}%\n\n", result.stats.match_percentage));

        if !result.schema_diffs.is_empty() {
            report.push_str(&format!("--- 结构差异 ({} 项) ---\n", result.schema_diffs.len()));
            for diff in &result.schema_diffs {
                report.push_str(&format!("  - {}\n", diff.description));
            }
            report.push('\n');
        }

        if !result.data_diffs.is_empty() {
            let display_count = result.data_diffs.len().min(100);
            report.push_str(&format!("--- 数据差异 (前 {} 项) ---\n", display_count));
            for diff in result.data_diffs.iter().take(100) {
                report.push_str(&format!("  - {}: {} vs {}\n", diff.field_name, diff.source_value.as_deref().unwrap_or("NULL"), diff.target_value.as_deref().unwrap_or("NULL")));
            }
            report.push('\n');
        }

        if !result.errors.is_empty() {
            report.push_str(&format!("--- 错误 ---\n"));
            for error in &result.errors {
                report.push_str(&format!("  - {}\n", error));
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_compare_config_default() {
        let config = DataCompareConfig::default();
        assert!(matches!(config.compare_mode, CompareMode::Full));
        assert_eq!(config.primary_key, "id");
        assert!(config.ignore_whitespace);
        assert!(!config.ignore_case);
    }

    #[test]
    fn test_data_diff_type_serialization() {
        let diff_type = DataDiffType::ValueMismatch;
        let json = serde_json::to_string(&diff_type).unwrap();
        assert!(json.contains("ValueMismatch"));
    }

    #[test]
    fn test_compare_stats() {
        let stats = CompareStats {
            source_rows: 1000,
            target_rows: 998,
            matched_rows: 995,
            diff_rows: 3,
            missing_in_target_count: 2,
            missing_in_source_count: 5,
            source_size_bytes: 1024000,
            target_size_bytes: 1023488,
            match_percentage: 99.5,
        };

        assert_eq!(stats.matched_rows, 995);
        assert_eq!(stats.match_percentage, 99.5);
    }

    #[test]
    fn test_value_to_sql_conversion() {
        assert_eq!(value_to_sql_static(&serde_json::json!(null)), "NULL");
        assert_eq!(value_to_sql_static(&serde_json::json!(true)), "TRUE");
        assert_eq!(value_to_sql_static(&serde_json::json!(false)), "FALSE");
        assert_eq!(value_to_sql_static(&serde_json::json!(123)), "123");
        assert_eq!(value_to_sql_static(&serde_json::json!("test")), "'test'");
    }

    #[test]
    fn test_values_equal_with_tolerance() {
        let tolerance = 0.0001;
        assert!(values_equal_static(&serde_json::json!(1.0), &serde_json::json!(1.00001), tolerance));
        assert!(!values_equal_static(&serde_json::json!(1.0), &serde_json::json!(1.001), tolerance));
    }
}

fn value_to_sql_static(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        _ => format!("'{}'", value.to_string().replace('\'', "''")),
    }
}

fn values_equal_static(a: &serde_json::Value, b: &serde_json::Value, tolerance: f64) -> bool {
    match (a, b) {
        (serde_json::Value::Null, serde_json::Value::Null) => true,
        (serde_json::Value::Null, _) | (_, serde_json::Value::Null) => false,
        (serde_json::Value::Number(a_num), serde_json::Value::Number(b_num)) => {
            if let (Some(a_f), Some(b_f)) = (a_num.as_f64(), b_num.as_f64()) {
                (a_f - b_f).abs() < tolerance
            } else {
                a_num == b_num
            }
        }
        _ => a == b,
    }
}
