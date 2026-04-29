/// 智能字段匹配模块 - 处理不同数据库结构差异

use crate::models::{Field, FieldMapping, TableSchema};
use crate::utils::string::similarity;
use std::collections::{HashMap, HashSet};

/// 匹配策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStrategy {
    /// 精确匹配字段名
    Exact,
    /// 相似度匹配（基于编辑距离）
    Similarity,
    /// 类型兼容匹配
    TypeCompatible,
    /// 智能匹配（综合多种策略）
    Smart,
}

/// 匹配配置
#[derive(Debug, Clone)]
pub struct FieldMatcherConfig {
    /// 匹配策略
    pub strategy: MatchStrategy,
    /// 相似度阈值 (0.0 - 1.0)
    pub similarity_threshold: f64,
    /// 是否考虑类型兼容性
    pub consider_type_compatibility: bool,
    /// 是否考虑字段顺序
    pub consider_field_order: bool,
    /// 忽略的字段前缀
    pub ignore_prefixes: Vec<String>,
    /// 忽略的字段后缀
    pub ignore_suffixes: Vec<String>,
}

impl Default for FieldMatcherConfig {
    fn default() -> Self {
        Self {
            strategy: MatchStrategy::Smart,
            similarity_threshold: 0.6,
            consider_type_compatibility: true,
            consider_field_order: true,
            ignore_prefixes: vec!["tbl_".to_string(), "tab_".to_string()],
            ignore_suffixes: vec!["_id".to_string(), "_name".to_string()],
        }
    }
}

/// 匹配结果
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub source_field: String,
    pub target_field: String,
    pub confidence: f64,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchType {
    /// 精确匹配
    Exact,
    /// 相似度匹配
    Similarity,
    /// 类型转换匹配
    TypeCast,
    /// 智能推断匹配
    Inferred,
    /// 未匹配
    Unmatched,
}

/// 智能字段匹配器
pub struct SmartFieldMatcher {
    config: FieldMatcherConfig,
    type_mappings: HashMap<(String, String), String>,
}

impl SmartFieldMatcher {
    pub fn new(config: FieldMatcherConfig) -> Self {
        let mut type_mappings = HashMap::new();
        
        // MySQL 到 PostgreSQL 类型映射
        let mysql_to_pg: Vec<((String, String), String)> = vec![
            (("INT".to_string(), "INTEGER".to_string()), "INTEGER".to_string()),
            (("BIGINT".to_string(), "BIGINT".to_string()), "BIGINT".to_string()),
            (("VARCHAR".to_string(), "VARCHAR".to_string()), "VARCHAR".to_string()),
            (("TEXT".to_string(), "TEXT".to_string()), "TEXT".to_string()),
            (("DATETIME".to_string(), "TIMESTAMP".to_string()), "TIMESTAMP".to_string()),
            (("TIMESTAMP".to_string(), "TIMESTAMP".to_string()), "TIMESTAMP".to_string()),
            (("BLOB".to_string(), "BYTEA".to_string()), "BYTEA".to_string()),
            (("TINYINT".to_string(), "SMALLINT".to_string()), "SMALLINT".to_string()),
            (("FLOAT".to_string(), "REAL".to_string()), "REAL".to_string()),
            (("DOUBLE".to_string(), "DOUBLE PRECISION".to_string()), "DOUBLE PRECISION".to_string()),
            (("DECIMAL".to_string(), "NUMERIC".to_string()), "NUMERIC".to_string()),
            (("ENUM".to_string(), "VARCHAR".to_string()), "VARCHAR".to_string()),
        ];
        
        for ((from, _), to) in mysql_to_pg {
            type_mappings.insert((from, "POSTGRES".to_string()), to);
        }
        
        // MySQL 到 MySQL (版本兼容)
        let mysql_compat: Vec<((String, String), String)> = vec![
            (("INT".to_string(), "INT".to_string()), "INT".to_string()),
            (("BIGINT".to_string(), "BIGINT".to_string()), "BIGINT".to_string()),
            (("VARCHAR".to_string(), "VARCHAR".to_string()), "VARCHAR".to_string()),
            (("TEXT".to_string(), "TEXT".to_string()), "TEXT".to_string()),
            (("TINYINT".to_string(), "TINYINT".to_string()), "TINYINT".to_string()),
        ];
        
        for ((from, _), to) in mysql_compat {
            type_mappings.insert((from, "MYSQL".to_string()), to);
        }
        
        Self {
            config,
            type_mappings,
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(FieldMatcherConfig::default())
    }

    /// 执行智能字段匹配
    pub fn match_fields(
        &self,
        source_schema: &TableSchema,
        target_schema: &TableSchema,
    ) -> Vec<MatchResult> {
        let mut results = Vec::new();
        let mut matched_targets: HashSet<String> = HashSet::new();
        
        // 按顺序处理源字段
        for (order, source_field) in source_schema.fields.iter().enumerate() {
            let result = self.find_best_match(
                source_field,
                target_schema,
                &matched_targets,
                order,
            );
            
            if result.match_type != MatchType::Unmatched {
                matched_targets.insert(result.target_field.clone());
            }
            
            results.push(result);
        }
        
        results
    }

    /// 查找最佳匹配
    fn find_best_match(
        &self,
        source_field: &Field,
        target_schema: &TableSchema,
        matched_targets: &HashSet<String>,
        order: usize,
    ) -> MatchResult {
        let mut best_match: Option<MatchResult> = None;
        
        for target_field in &target_schema.fields {
            // 跳过已匹配的字段
            if matched_targets.contains(&target_field.name) {
                continue;
            }
            
            // 尝试不同策略
            if let Some(result) = self.try_match(source_field, target_field, order) {
                match &best_match {
                    None => best_match = Some(result),
                    Some(current) if result.confidence > current.confidence => {
                        best_match = Some(result);
                    }
                    _ => {}
                }
            }
        }
        
        best_match.unwrap_or_else(|| MatchResult {
            source_field: source_field.name.clone(),
            target_field: String::new(),
            confidence: 0.0,
            match_type: MatchType::Unmatched,
        })
    }

    /// 尝试匹配
    fn try_match(&self, source: &Field, target: &Field, _order: usize) -> Option<MatchResult> {
        // 策略1: 精确匹配
        if source.name.to_lowercase() == target.name.to_lowercase() {
            return Some(MatchResult {
                source_field: source.name.clone(),
                target_field: target.name.clone(),
                confidence: 1.0,
                match_type: MatchType::Exact,
            });
        }
        
        // 策略2: 清理前缀后缀后匹配
        let cleaned_source = self.clean_field_name(&source.name);
        let cleaned_target = self.clean_field_name(&target.name);
        
        if cleaned_source == cleaned_target {
            return Some(MatchResult {
                source_field: source.name.clone(),
                target_field: target.name.clone(),
                confidence: 0.95,
                match_type: MatchType::Exact,
            });
        }
        
        // 策略3: 相似度匹配
        let sim = similarity(&cleaned_source, &cleaned_target);
        if sim >= self.config.similarity_threshold {
            return Some(MatchResult {
                source_field: source.name.clone(),
                target_field: target.name.clone(),
                confidence: sim,
                match_type: MatchType::Similarity,
            });
        }
        
        // 策略5: 类型兼容匹配
        if self.config.consider_type_compatibility {
            if self.are_types_compatible(&source.data_type, &target.data_type) {
                // 类型兼容但名称不同，给予较低置信度
                return Some(MatchResult {
                    source_field: source.name.clone(),
                    target_field: target.name.clone(),
                    confidence: 0.5,
                    match_type: MatchType::TypeCast,
                });
            }
        }
        
        None
    }

    /// 清理字段名（移除前缀后缀）
    fn clean_field_name(&self, name: &str) -> String {
        let mut cleaned = name.to_lowercase();
        
        // 移除前缀
        for prefix in &self.config.ignore_prefixes {
            if cleaned.starts_with(&prefix.to_lowercase()) {
                cleaned = cleaned[prefix.len()..].to_string();
            }
        }
        
        // 移除后缀
        for suffix in &self.config.ignore_suffixes {
            if cleaned.ends_with(&suffix.to_lowercase()) {
                cleaned = cleaned[..cleaned.len() - suffix.len()].to_string();
            }
        }
        
        cleaned.trim().to_string()
    }

    /// 检查类型是否兼容
    fn are_types_compatible(&self, source: &str, target: &str) -> bool {
        let s = source.to_uppercase();
        let t = target.to_uppercase();
        
        // 完全相同
        if s == t {
            return true;
        }
        
        // 数值类型
        let numeric_types = ["INT", "INTEGER", "BIGINT", "SMALLINT", "TINYINT", "FLOAT", "DOUBLE", "DECIMAL", "NUMERIC"];
        if numeric_types.iter().any(|&t| s.contains(t)) && numeric_types.iter().any(|&t| t.contains(&s)) {
            return true;
        }
        
        // 字符串类型
        let string_types = ["VARCHAR", "CHAR", "TEXT", "STRING"];
        if string_types.iter().any(|&t| s.contains(t)) && string_types.iter().any(|&t| t.contains(&s)) {
            return true;
        }
        
        // 时间类型
        let time_types = ["DATETIME", "TIMESTAMP", "DATE", "TIME"];
        if time_types.iter().any(|&t| s.contains(t)) && time_types.iter().any(|&t| t.contains(&s)) {
            return true;
        }
        
        false
    }

    /// 获取类型转换建议
    pub fn get_type_cast(&self, source_type: &str, target_db: &str) -> Option<String> {
        self.type_mappings
            .get(&(source_type.to_uppercase(), target_db.to_uppercase()))
            .cloned()
    }

    /// 生成字段映射
    pub fn generate_mappings(&self, results: &[MatchResult]) -> Vec<FieldMapping> {
        results
            .iter()
            .filter(|r| r.match_type != MatchType::Unmatched)
            .map(|r| FieldMapping {
                source_table: String::new(),
                source_field: r.source_field.clone(),
                target_table: String::new(),
                target_field: r.target_field.clone(),
            })
            .collect()
    }
}

/// 目标数据库架构信息
#[derive(Debug, Clone)]
pub struct TargetSchemaInfo {
    pub db_type: String,
    pub db_version: Option<String>,
    pub features: Vec<String>,
}

impl TargetSchemaInfo {
    pub fn new(db_type: &str) -> Self {
        Self {
            db_type: db_type.to_string(),
            db_version: None,
            features: Vec::new(),
        }
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.db_version = Some(version.to_string());
        self
    }

    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    pub fn is_key_value_store(&self) -> bool {
        matches!(
            self.db_type.to_lowercase().as_str(),
            "redis" | "mongodb" | "cassandra" | "dynamodb" | "taodb"
        )
    }

    pub fn is_time_series_db(&self) -> bool {
        matches!(
            self.db_type.to_lowercase().as_str(),
            "influxdb" | "timescaledb" | "prometheus" | "kdb+" | "questdb"
        )
    }

    pub fn supports_sql(&self) -> bool {
        !matches!(
            self.db_type.to_lowercase().as_str(),
            "redis" | "mongodb" | "cassandra" | "dynamodb"
        )
    }
}

/// 转换建议生成器
pub struct ConversionSuggestionGenerator {
    matcher: SmartFieldMatcher,
}

impl ConversionSuggestionGenerator {
    pub fn new(matcher: SmartFieldMatcher) -> Self {
        Self { matcher }
    }

    /// 生成转换建议
    pub fn generate_suggestions(
        &self,
        source_schema: &TableSchema,
        target_schema: &TableSchema,
        target_info: &TargetSchemaInfo,
    ) -> ConversionSuggestions {
        let mut suggestions = Vec::new();
        
        // 1. 字段匹配建议
        let match_results = self.matcher.match_fields(source_schema, target_schema);
        
        // 2. 类型转换建议
        for result in &match_results {
            if result.match_type == MatchType::TypeCast {
                suggestions.push(ConversionSuggestion {
                    suggestion_type: SuggestionType::TypeConversion,
                    field: result.source_field.clone(),
                    message: format!(
                        "字段 '{}' 类型可能需要转换，建议检查目标类型是否兼容",
                        result.source_field
                    ),
                    priority: Priority::Medium,
                });
            }
        }
        
        // 3. 目标数据库特定建议
        if target_info.is_key_value_store() {
            suggestions.push(ConversionSuggestion {
                suggestion_type: SuggestionType::StructureChange,
                field: String::new(),
                message: "目标数据库是 Key-Value 类型，建议将多个字段合并为 JSON 或使用 ID 作为 Key".to_string(),
                priority: Priority::High,
            });
        }
        
        if target_info.is_time_series_db() {
            suggestions.push(ConversionSuggestion {
                suggestion_type: SuggestionType::StructureChange,
                field: String::new(),
                message: "目标数据库是时序数据库，建议添加时间戳字段并优化分区策略".to_string(),
                priority: Priority::High,
            });
        }
        
        // 4. 缺失字段建议
        let matched_fields: HashSet<_> = match_results
            .iter()
            .filter(|r| r.match_type != MatchType::Unmatched)
            .map(|r| r.source_field.clone())
            .collect();
        
        for field in &source_schema.fields {
            if !matched_fields.contains(&field.name) {
                suggestions.push(ConversionSuggestion {
                    suggestion_type: SuggestionType::MissingField,
                    field: field.name.clone(),
                    message: format!("字段 '{}' 在目标数据库中没有找到匹配，可能需要手动映射", field.name),
                    priority: Priority::High,
                });
            }
        }
        
        ConversionSuggestions { suggestions }
    }
}

/// 转换建议
#[derive(Debug, Clone)]
pub struct ConversionSuggestion {
    pub suggestion_type: SuggestionType,
    pub field: String,
    pub message: String,
    pub priority: Priority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionType {
    TypeConversion,
    StructureChange,
    MissingField,
    IndexOptimization,
    PerformanceOptimization,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
}

/// 转换建议集合
#[derive(Debug, Clone, Default)]
pub struct ConversionSuggestions {
    pub suggestions: Vec<ConversionSuggestion>,
}

impl ConversionSuggestions {
    pub fn has_high_priority(&self) -> bool {
        self.suggestions.iter().any(|s| s.priority == Priority::High)
    }

    pub fn format(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("共有 {} 条转换建议:\n", self.suggestions.len()));
        
        for suggestion in &self.suggestions {
            let priority_str = match suggestion.priority {
                Priority::High => "[高优先级]",
                Priority::Medium => "[中优先级]",
                Priority::Low => "[低优先级]",
            };
            
            output.push_str(&format!(
                "{} {} - {}: {}\n",
                priority_str,
                format!("{:?}", suggestion.suggestion_type),
                suggestion.field,
                suggestion.message
            ));
        }
        
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let matcher = SmartFieldMatcher::with_default_config();
        let config = FieldMatcherConfig::default();
        let matcher = SmartFieldMatcher::new(config);
        
        let source_schema = TableSchema {
            name: "users".to_string(),
            fields: vec![
                Field {
                    name: "id".to_string(),
                    data_type: "INT".to_string(),
                    length: None,
                    nullable: false,
                    default_value: None,
                    primary_key: true,
                    auto_increment: true,
                },
                Field {
                    name: "name".to_string(),
                    data_type: "VARCHAR".to_string(),
                    length: Some(100),
                    nullable: false,
                    default_value: None,
                    primary_key: false,
                    auto_increment: false,
                },
            ],
            foreign_keys: vec![],
            indexes: vec![],
        };
        
        let target_schema = TableSchema {
            name: "users_copy".to_string(),
            fields: vec![
                Field {
                    name: "id".to_string(),
                    data_type: "INTEGER".to_string(),
                    length: None,
                    nullable: false,
                    default_value: None,
                    primary_key: true,
                    auto_increment: true,
                },
                Field {
                    name: "name".to_string(),
                    data_type: "VARCHAR".to_string(),
                    length: Some(100),
                    nullable: false,
                    default_value: None,
                    primary_key: false,
                    auto_increment: false,
                },
            ],
            foreign_keys: vec![],
            indexes: vec![],
        };
        
        let results = matcher.match_fields(&source_schema, &target_schema);
        
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].match_type, MatchType::Exact);
        assert_eq!(results[1].match_type, MatchType::Exact);
        
        println!("匹配结果: {:?}", results);
    }

    #[test]
    fn test_similarity_match() {
        let mut config = FieldMatcherConfig::default();
        config.similarity_threshold = 0.5;
        let matcher = SmartFieldMatcher::new(config);
        
        let source_schema = TableSchema {
            name: "users".to_string(),
            fields: vec![
                Field {
                    name: "user_name".to_string(),
                    data_type: "VARCHAR".to_string(),
                    length: Some(100),
                    nullable: false,
                    default_value: None,
                    primary_key: false,
                    auto_increment: false,
                },
            ],
            foreign_keys: vec![],
            indexes: vec![],
        };
        
        let target_schema = TableSchema {
            name: "users_copy".to_string(),
            fields: vec![
                Field {
                    name: "username".to_string(),
                    data_type: "VARCHAR".to_string(),
                    length: Some(100),
                    nullable: false,
                    default_value: None,
                    primary_key: false,
                    auto_increment: false,
                },
            ],
            foreign_keys: vec![],
            indexes: vec![],
        };
        
        let results = matcher.match_fields(&source_schema, &target_schema);

        assert!(!results.is_empty());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].match_type, MatchType::Similarity);
        assert!(results[0].confidence >= 0.5);

        println!("相似度匹配结果: {:?}", results);
    }

    #[test]
    fn test_type_compatibility() {
        let config = FieldMatcherConfig::default();
        let matcher = SmartFieldMatcher::new(config);
        
        let source_schema = TableSchema {
            name: "data".to_string(),
            fields: vec![
                Field {
                    name: "created_at".to_string(),
                    data_type: "DATETIME".to_string(),
                    length: None,
                    nullable: true,
                    default_value: None,
                    primary_key: false,
                    auto_increment: false,
                },
            ],
            foreign_keys: vec![],
            indexes: vec![],
        };
        
        let target_schema = TableSchema {
            name: "data_copy".to_string(),
            fields: vec![
                Field {
                    name: "created_time".to_string(),
                    data_type: "TIMESTAMP".to_string(),
                    length: None,
                    nullable: true,
                    default_value: None,
                    primary_key: false,
                    auto_increment: false,
                },
            ],
            foreign_keys: vec![],
            indexes: vec![],
        };
        
        let results = matcher.match_fields(&source_schema, &target_schema);
        
        assert_eq!(results.len(), 1);
        // 类型兼容但不精确匹配
        assert!(results[0].confidence < 1.0);
        
        println!("类型兼容性匹配结果: {:?}", results);
    }
}
