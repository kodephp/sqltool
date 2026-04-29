/// 数据库安全防御模块
/// 提供SQL注入防护、字段验证、敏感数据加密等功能

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// SQL注入检测器
pub struct SqlInjectionDetector {
    /// 危险关键字列表
    dangerous_keywords: HashSet<String>,
    /// 危险函数列表
    dangerous_functions: HashSet<String>,
    /// 严格模式
    strict_mode: bool,
}

impl SqlInjectionDetector {
    pub fn new() -> Self {
        let mut dangerous_keywords = HashSet::new();
        dangerous_keywords.insert("OR".to_string());
        dangerous_keywords.insert("AND".to_string());
        dangerous_keywords.insert("UNION".to_string());
        dangerous_keywords.insert("SELECT".to_string());
        dangerous_keywords.insert("INSERT".to_string());
        dangerous_keywords.insert("UPDATE".to_string());
        dangerous_keywords.insert("DELETE".to_string());
        dangerous_keywords.insert("DROP".to_string());
        dangerous_keywords.insert("TRUNCATE".to_string());
        dangerous_keywords.insert("EXEC".to_string());
        dangerous_keywords.insert("EXECUTE".to_string());
        dangerous_keywords.insert("SCRIPT".to_string());
        dangerous_keywords.insert("ALTER".to_string());
        dangerous_keywords.insert("CREATE".to_string());
        dangerous_keywords.insert("GRANT".to_string());
        dangerous_keywords.insert("REVOKE".to_string());

        let mut dangerous_functions = HashSet::new();
        dangerous_functions.insert("SLEEP".to_string());
        dangerous_functions.insert("BENCHMARK".to_string());
        dangerous_functions.insert("LOAD_FILE".to_string());
        dangerous_functions.insert("INTO OUTFILE".to_string());
        dangerous_functions.insert("INTO DUMPFILE".to_string());

        Self {
            dangerous_keywords,
            dangerous_functions,
            strict_mode: false,
        }
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    /// 检测SQL注入
    pub fn detect(&self, input: &str) -> Option<InjectionReport> {
        let upper = input.to_uppercase();
        let mut findings = Vec::new();
        let mut risk_level = RiskLevel::Low;

        // 检测注释攻击
        if upper.contains("--") || upper.contains("/*") || upper.contains("*/") || upper.contains("#") {
            findings.push(Finding {
                category: FindingCategory::CommentAttack,
                description: "检测到SQL注释符号".to_string(),
                position: input.find("--").or_else(|| input.find("/*")),
            });
            risk_level = RiskLevel::High;
        }

        // 检测字符串截断
        if input.contains("'") || input.contains("\"") {
            findings.push(Finding {
                category: FindingCategory::StringTruncation,
                description: "检测到可能的字符串截断".to_string(),
                position: input.find("'").or_else(|| input.find("\"")),
            });
            risk_level = RiskLevel::High;
        }

        // 检测堆叠查询
        if input.contains(";") {
            findings.push(Finding {
                category: FindingCategory::StackedQuery,
                description: "检测到分号，可能存在堆叠查询".to_string(),
                position: input.find(";"),
            });
            risk_level = RiskLevel::Critical;
        }

        // 检测UNION注入
        if upper.contains("UNION") && upper.contains("SELECT") {
            findings.push(Finding {
                category: FindingCategory::UnionInjection,
                description: "检测到UNION SELECT注入模式".to_string(),
                position: upper.find("UNION"),
            });
            risk_level = RiskLevel::Critical;
        }

        // 检测时间盲注
        for func in &self.dangerous_functions {
            if upper.contains(func) {
                findings.push(Finding {
                    category: FindingCategory::TimeBasedInjection,
                    description: format!("检测到危险函数: {}", func),
                    position: upper.find(func),
                });
                risk_level = RiskLevel::Critical;
            }
        }

        // 严格模式下检测更多模式
        if self.strict_mode {
            for keyword in &self.dangerous_keywords {
                if upper.contains(keyword) {
                    findings.push(Finding {
                        category: FindingCategory::SuspiciousKeyword,
                        description: format!("检测到可疑关键字: {}", keyword),
                        position: upper.find(keyword),
                    });
                    if risk_level < RiskLevel::Medium {
                        risk_level = RiskLevel::Medium;
                    }
                }
            }
        }

        if findings.is_empty() {
            None
        } else {
            Some(InjectionReport {
                input: input.to_string(),
                risk_level,
                findings,
                sanitized_input: self.sanitize(input),
            })
        }
    }

    /// 清理输入
    pub fn sanitize(&self, input: &str) -> String {
        let mut result = input.to_string();
        
        // 转义单引号
        result = result.replace("'", "''");
        
        // 移除注释
        result = result.replace("--", "");
        result = result.replace("/*", "");
        result = result.replace("*/", "");
        
        // 移除分号
        result = result.replace(";", "");
        
        result
    }

    /// 参数化查询验证
    pub fn validate_parameter(&self, param: &serde_json::Value) -> Result<()> {
        match param {
            serde_json::Value::String(s) => {
                if let Some(report) = self.detect(s) {
                    if report.risk_level >= RiskLevel::High {
                        return Err(anyhow!(
                            "SQL注入风险检测: {} - {}",
                            report.risk_level,
                            report.findings.iter()
                                .map(|f| f.description.clone())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl Default for SqlInjectionDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionReport {
    pub input: String,
    pub risk_level: RiskLevel,
    pub findings: Vec<Finding>,
    pub sanitized_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
            RiskLevel::Critical => write!(f, "Critical"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub category: FindingCategory,
    pub description: String,
    pub position: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingCategory {
    CommentAttack,
    StringTruncation,
    StackedQuery,
    UnionInjection,
    TimeBasedInjection,
    SuspiciousKeyword,
    BooleanBasedInjection,
    ErrorBasedInjection,
}

/// 字段安全验证器
pub struct FieldSecurityValidator {
    /// 敏感字段列表
    sensitive_fields: HashSet<String>,
    /// 最大字段长度
    max_field_length: usize,
}

impl FieldSecurityValidator {
    pub fn new() -> Self {
        let sensitive_fields = [
            "password", "passwd", "pwd", "secret", "token", "api_key",
            "apikey", "private_key", "credit_card", "ssn", "social_security",
        ].iter().map(|s| s.to_string()).collect();

        Self {
            sensitive_fields,
            max_field_length: 64,
        }
    }

    pub fn validate_field_name(&self, name: &str) -> Result<()> {
        // 检查长度
        if name.len() > self.max_field_length {
            return Err(anyhow!(
                "字段名 '{}' 长度 {} 超过最大限制 {}",
                name, name.len(), self.max_field_length
            ));
        }

        // 检查只允许字母数字和下划线
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow!(
                "字段名 '{}' 包含非法字符，只允许字母、数字和下划线",
                name
            ));
        }

        // 检查不能以数字开头
        if name.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
            return Err(anyhow!("字段名 '{}' 不能以数字开头", name));
        }

        Ok(())
    }

    pub fn is_sensitive_field(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.sensitive_fields.iter().any(|s| lower.contains(s))
    }

    pub fn mask_sensitive_value(&self, field_name: &str, value: &str) -> String {
        if self.is_sensitive_field(field_name) {
            if value.len() <= 4 {
                "****".to_string()
            } else {
                format!("{}****", &value[..2])
            }
        } else {
            value.to_string()
        }
    }
}

impl Default for FieldSecurityValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// 简单加密处理器（用于演示，生产环境请使用专业加密库）
pub struct SimpleEncryptor {
    key: Vec<u8>,
}

impl SimpleEncryptor {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.bytes().collect(),
        }
    }

    pub fn encrypt(&self, data: &str) -> String {
        let bytes = data.bytes();
        let encrypted: Vec<u8> = bytes
            .enumerate()
            .map(|(i, b)| b.wrapping_add(self.key[i % self.key.len()]))
            .collect();
        base64_encode(&encrypted)
    }

    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        let bytes = base64_decode(encrypted)?;
        let decrypted: Vec<u8> = bytes
            .iter()
            .enumerate()
            .map(|(i, b)| b.wrapping_sub(self.key[i % self.key.len()]))
            .collect();
        String::from_utf8(decrypted).map_err(|e| anyhow!("解密失败: {}", e))
    }
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in input.chunks(3) {
        let b = match chunk.len() {
            1 => [chunk[0], 0, 0],
            2 => [chunk[0], chunk[1], 0],
            _ => [chunk[0], chunk[1], chunk[2]],
        };
        result.push(CHARS[(b[0] >> 2) as usize] as char);
        result.push(CHARS[((b[0] & 0x03) << 4 | b[1] >> 4) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((b[1] & 0x0F) << 2 | b[2] >> 6) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(b[2] & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    const DECODE: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62, -1, -1, -1, 63,
        52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1,
        -1,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14,
        15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1,
        -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
        41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
    ];
    let input = input.trim_end_matches('=');
    let mut result = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;

    for c in input.chars() {
        if c as usize >= 128 {
            return Err(anyhow!("Invalid base64 character"));
        }
        let value = DECODE[c as usize];
        if value < 0 {
            return Err(anyhow!("Invalid base64 character"));
        }
        buffer = (buffer << 6) | (value as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }

    Ok(result)
}

/// 安全的SQL构建器
pub struct SafeSqlBuilder {
    table: String,
    detector: SqlInjectionDetector,
    validator: FieldSecurityValidator,
}

impl SafeSqlBuilder {
    pub fn new(table: &str) -> Result<Self> {
        let validator = FieldSecurityValidator::new();
        validator.validate_field_name(table)?;

        Ok(Self {
            table: table.to_string(),
            detector: SqlInjectionDetector::new(),
            validator,
        })
    }

    pub fn safe_where(&self, field: &str, operator: &str, value: &serde_json::Value) -> Result<String> {
        self.validator.validate_field_name(field)?;
        
        if let Some(report) = self.detector.detect(field) {
            if report.risk_level >= RiskLevel::High {
                return Err(anyhow!("字段名包含注入风险: {}", report.risk_level));
            }
        }

        let safe_value = match value {
            serde_json::Value::String(s) => {
                if let Some(report) = self.detector.detect(s) {
                    if report.risk_level >= RiskLevel::High {
                        return Err(anyhow!("值包含注入风险: {}", report.risk_level));
                    }
                }
                format!("'{}'", s.replace('\'', "''"))
            }
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => if *b { "TRUE".to_string() } else { "FALSE".to_string() },
            serde_json::Value::Null => "NULL".to_string(),
            _ => return Err(anyhow!("不支持的值类型")),
        };

        let safe_operator = match operator {
            "=" | "!=" | "<>" | ">" | "<" | ">=" | "<=" => operator,
            "LIKE" | "like" => "LIKE",
            "IN" | "in" => "IN",
            _ => return Err(anyhow!("不安全的操作符: {}", operator)),
        };

        Ok(format!("{} {} {}", field, safe_operator, safe_value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_injection_detection() {
        let detector = SqlInjectionDetector::new();

        // 正常输入
        assert!(detector.detect("Alice").is_none());
        assert!(detector.detect("user@example.com").is_none());

        // 注释攻击
        let report = detector.detect("admin'--").unwrap();
        assert_eq!(report.risk_level, RiskLevel::High);

        // UNION注入
        let report = detector.detect("' UNION SELECT * FROM users --").unwrap();
        assert_eq!(report.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_sanitize() {
        let detector = SqlInjectionDetector::new();
        let sanitized = detector.sanitize("admin'; DROP TABLE users; --");
        assert!(!sanitized.contains(";"));
        assert!(!sanitized.contains("--"));
    }

    #[test]
    fn test_field_validation() {
        let validator = FieldSecurityValidator::new();

        assert!(validator.validate_field_name("user_name").is_ok());
        assert!(validator.validate_field_name("123abc").is_err());
        assert!(validator.validate_field_name("name;drop").is_err());

        assert!(validator.is_sensitive_field("password"));
        assert!(validator.is_sensitive_field("api_key"));
        assert!(!validator.is_sensitive_field("username"));
    }

    #[test]
    fn test_sensitive_masking() {
        let validator = FieldSecurityValidator::new();
        
        let masked = validator.mask_sensitive_value("password", "mypassword123");
        assert!(masked.contains("****"));

        let normal = validator.mask_sensitive_value("username", "alice");
        assert_eq!(normal, "alice");
    }

    #[test]
    fn test_encryption() {
        let encryptor = SimpleEncryptor::new("mysecretkey");
        let original = "Hello, World!";
        
        let encrypted = encryptor.encrypt(original);
        assert_ne!(encrypted, original);
        
        let decrypted = encryptor.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_safe_sql_builder() {
        let builder = SafeSqlBuilder::new("users").unwrap();
        
        let condition = builder.safe_where(
            "name",
            "=",
            &serde_json::json!("Alice")
        ).unwrap();
        assert_eq!(condition, "name = 'Alice'");

        // 应该拒绝注入尝试
        let result = builder.safe_where(
            "name'; DROP TABLE users; --",
            "=",
            &serde_json::json!("test")
        );
        assert!(result.is_err());
    }
}
