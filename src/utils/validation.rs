/// 数据验证和转换模块

use crate::models::{Field, FieldMapping};
use crate::utils::error::{SqlToolError, SqlResult};

/// 数据验证器
pub struct DataValidator {
    strict_mode: bool,
}

impl DataValidator {
    pub fn new(strict_mode: bool) -> Self {
        Self { strict_mode }
    }

    /// 验证字段值是否符合目标字段的类型和约束
    pub fn validate_value(&self, value: &serde_json::Value, field: &Field) -> SqlResult<()> {
        match value {
            serde_json::Value::Null => {
                if !field.nullable {
                    return Err(SqlToolError::validation_error(&format!(
                        "Field '{}' does not allow NULL values",
                        field.name
                    )));
                }
            }
            serde_json::Value::Bool(_) => {
                if !self.is_boolean_type(&field.data_type) {
                    return Err(SqlToolError::validation_error(&format!(
                        "Field '{}' expects {} but received boolean",
                        field.name, field.data_type
                    )));
                }
            }
            serde_json::Value::Number(num) => {
                if !self.is_numeric_type(&field.data_type) {
                    return Err(SqlToolError::validation_error(&format!(
                        "Field '{}' expects {} but received number",
                        field.name, field.data_type
                    )));
                }
                if let Some(max_len) = field.length {
                    let num_str = num.to_string();
                    if num_str.len() > max_len {
                        return Err(SqlToolError::validation_error(&format!(
                            "Field '{}' value length {} exceeds maximum {}",
                            field.name, num_str.len(), max_len
                        )));
                    }
                }
            }
            serde_json::Value::String(s) => {
                if !self.is_string_type(&field.data_type) {
                    return Err(SqlToolError::validation_error(&format!(
                        "Field '{}' expects {} but received string",
                        field.name, field.data_type
                    )));
                }
                if let Some(max_len) = field.length {
                    if s.len() > max_len {
                        return Err(SqlToolError::validation_error(&format!(
                            "Field '{}' value length {} exceeds maximum {}",
                            field.name, s.len(), max_len
                        )));
                    }
                }
            }
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                return Err(SqlToolError::validation_error(&format!(
                    "Field '{}' does not support complex JSON values",
                    field.name
                )));
            }
        }
        Ok(())
    }

    fn is_boolean_type(&self, data_type: &str) -> bool {
        let dt = data_type.to_lowercase();
        dt == "boolean" || dt == "bool" || dt == "tinyint" || dt == "bit"
    }

    fn is_numeric_type(&self, data_type: &str) -> bool {
        let dt = data_type.to_lowercase();
        dt.contains("int") || dt.contains("float") || dt.contains("double") || 
        dt.contains("decimal") || dt.contains("numeric") || dt.contains("real")
    }

    fn is_string_type(&self, data_type: &str) -> bool {
        let dt = data_type.to_lowercase();
        dt.contains("char") || dt.contains("text") || dt.contains("string") ||
        dt.contains("varchar") || dt.contains("enum")
    }

    /// 验证字段映射是否有效
    pub fn validate_mapping(&self, mapping: &FieldMapping) -> SqlResult<()> {
        if mapping.source_field.trim().is_empty() {
            return Err(SqlToolError::validation_error("Source field name cannot be empty"));
        }
        if mapping.target_field.trim().is_empty() {
            return Err(SqlToolError::validation_error("Target field name cannot be empty"));
        }
        if mapping.source_table.trim().is_empty() {
            return Err(SqlToolError::validation_error("Source table name cannot be empty"));
        }
        if mapping.target_table.trim().is_empty() {
            return Err(SqlToolError::validation_error("Target table name cannot be empty"));
        }
        Ok(())
    }
}

/// 数据转换器
pub struct DataTransformer {
    pub trim_strings: bool,
    pub null_to_default: bool,
    pub type_coercion: bool,
}

impl Default for DataTransformer {
    fn default() -> Self {
        Self {
            trim_strings: true,
            null_to_default: false,
            type_coercion: true,
        }
    }
}

impl DataTransformer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 转换值以适应目标字段
    pub fn transform_value(&self, value: serde_json::Value, field: &Field) -> SqlResult<serde_json::Value> {
        let transformed = match value {
            serde_json::Value::String(s) => {
                let s = if self.trim_strings { s.trim().to_string() } else { s };
                self.convert_string_to_type(&s, field)?
            }
            serde_json::Value::Null => {
                if self.null_to_default {
                    if let Some(ref default) = field.default_value {
                        serde_json::Value::String(default.clone())
                    } else {
                        serde_json::Value::Null
                    }
                } else {
                    serde_json::Value::Null
                }
            }
            _ => value,
        };
        Ok(transformed)
    }

    fn convert_string_to_type(&self, s: &str, field: &Field) -> SqlResult<serde_json::Value> {
        if !self.type_coercion {
            return Ok(serde_json::Value::String(s.to_string()));
        }

        let dt = field.data_type.to_lowercase();
        
        if dt.contains("int") {
            match s.parse::<i64>() {
                Ok(n) => return Ok(serde_json::Value::Number(n.into())),
                Err(_) => {
                    if self.strict_mode_validation() {
                        return Err(SqlToolError::data_error(&format!(
                            "Cannot convert '{}' to integer for field '{}'",
                            s, field.name
                        )));
                    }
                }
            }
        } else if dt.contains("float") || dt.contains("double") || dt.contains("decimal") || dt.contains("real") {
            match s.parse::<f64>() {
                Ok(n) => return Ok(serde_json::Number::from_f64(n).map(|n| serde_json::Value::Number(n)).unwrap_or(serde_json::Value::Null)),
                Err(_) => {
                    if self.strict_mode_validation() {
                        return Err(SqlToolError::data_error(&format!(
                            "Cannot convert '{}' to float for field '{}'",
                            s, field.name
                        )));
                    }
                }
            }
        } else if dt == "boolean" || dt == "bool" {
            let lower = s.to_lowercase();
            if lower == "true" || lower == "1" || lower == "yes" {
                return Ok(serde_json::Value::Bool(true));
            } else if lower == "false" || lower == "0" || lower == "no" {
                return Ok(serde_json::Value::Bool(false));
            }
        }

        Ok(serde_json::Value::String(s.to_string()))
    }

    fn strict_mode_validation(&self) -> bool {
        false
    }

    /// 批量转换值
    pub fn transform_values(
        &self,
        values: Vec<serde_json::Value>,
        fields: &[Field],
    ) -> SqlResult<Vec<serde_json::Value>> {
        if values.len() != fields.len() {
            return Err(SqlToolError::data_error(&format!(
                "Value count {} does not match field count {}",
                values.len(),
                fields.len()
            )));
        }

        let mut transformed = Vec::with_capacity(values.len());
        for (value, field) in values.iter().zip(fields.iter()) {
            transformed.push(self.transform_value(value.clone(), field)?);
        }
        Ok(transformed)
    }
}

/// 数据过滤器
pub struct DataFilter {
    pub skip_nulls: bool,
    pub skip_duplicates: bool,
    pub unique_fields: Vec<String>,
}

impl Default for DataFilter {
    fn default() -> Self {
        Self {
            skip_nulls: false,
            skip_duplicates: false,
            unique_fields: Vec::new(),
        }
    }
}

impl DataFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 过滤重复记录
    pub fn filter_duplicates(&self, records: &mut Vec<Vec<serde_json::Value>>, fields: &[String]) -> usize {
        if !self.skip_duplicates || fields.is_empty() {
            return 0;
        }

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut removed = 0;

        records.retain(|record| {
            let key: String = record.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("|");
            
            if seen.contains(&key) {
                removed += 1;
                false
            } else {
                seen.insert(key);
                true
            }
        });

        removed
    }

    /// 过滤NULL值
    pub fn filter_nulls(&self, records: &mut Vec<Vec<serde_json::Value>>) -> usize {
        if !self.skip_nulls {
            return 0;
        }

        let initial_len = records.len();
        records.retain(|record| !record.iter().any(|v| v.is_null()));
        initial_len - records.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_validator() {
        let validator = DataValidator::new(false);
        let field = Field {
            name: "test".to_string(),
            data_type: "VARCHAR".to_string(),
            length: Some(10),
            nullable: false,
            default_value: None,
            primary_key: false,
            auto_increment: false,
        };

        let result = validator.validate_value(&serde_json::Value::String("test".to_string()), &field);
        assert!(result.is_ok());

        let result = validator.validate_value(&serde_json::Value::Null, &field);
        assert!(result.is_err());
    }

    #[test]
    fn test_data_transformer() {
        let transformer = DataTransformer::new();
        let field = Field {
            name: "age".to_string(),
            data_type: "INTEGER".to_string(),
            length: None,
            nullable: true,
            default_value: None,
            primary_key: false,
            auto_increment: false,
        };

        let result = transformer.transform_value(serde_json::Value::String("25".to_string()), &field);
        assert!(result.is_ok());
        if let Ok(serde_json::Value::Number(n)) = result {
            assert_eq!(n.as_i64().unwrap(), 25);
        }
    }
}