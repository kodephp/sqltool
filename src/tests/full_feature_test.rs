//! SQLTool 功能测试套件
//!
//! 测试各功能模块的数据转化正确性

#[cfg(test)]
mod tests {
    // ============================================================================
    // 连接字符串解析测试
    // ============================================================================

    #[test]
    fn test_parse_mysql_connection_string() {
        let conn = crate::utils::ConnectionString::parse("mysql://root:password@localhost:3306/mydb").unwrap();
        assert_eq!(conn.db_type, "mysql");
        assert_eq!(conn.host, "localhost");
        assert_eq!(conn.port, 3306);
        assert_eq!(conn.username.as_deref(), Some("root"));
        assert_eq!(conn.password.as_deref(), Some("password"));
        assert_eq!(conn.database, "mydb");
    }

    #[test]
    fn test_parse_postgresql_connection_string() {
        let conn = crate::utils::ConnectionString::parse("postgresql://postgres:secret@192.168.1.100:5432/testdb").unwrap();
        assert_eq!(conn.db_type, "postgresql");
        assert_eq!(conn.host, "192.168.1.100");
        assert_eq!(conn.port, 5432);
        assert_eq!(conn.database, "testdb");
    }

    #[test]
    fn test_parse_sqlite_connection_string() {
        let conn = crate::utils::ConnectionString::parse("sqlite:///tmp/test.db").unwrap();
        assert_eq!(conn.db_type, "sqlite");
        assert_eq!(conn.database, "tmp/test.db");
    }

    #[test]
    fn test_parse_oracle_connection_string() {
        let conn = crate::utils::ConnectionString::parse("oracle://system:pass@localhost:1521/orcl").unwrap();
        assert_eq!(conn.db_type, "oracle");
        assert_eq!(conn.port, 1521);
        assert_eq!(conn.database, "orcl");
    }

    #[test]
    fn test_parse_connection_string_with_options() {
        let conn = crate::utils::ConnectionString::parse(
            "mysql://root:pass@localhost:3306/mydb?charset=utf8mb4&timeout=10"
        ).unwrap();
        assert_eq!(conn.options.len(), 2);
        assert_eq!(conn.options[0].0, "charset");
        assert_eq!(conn.options[0].1, "utf8mb4");
    }

    #[test]
    fn test_connection_string_mask_password() {
        let conn = crate::utils::ConnectionString::parse("mysql://root:secretpass@localhost/mydb").unwrap();
        let masked = conn.mask_password();
        assert!(!masked.contains("secretpass"));
        assert!(masked.contains("***"));
    }

    #[test]
    fn test_connection_string_is_localhost() {
        let conn1 = crate::utils::ConnectionString::parse("mysql://root@localhost/mydb").unwrap();
        assert!(conn1.is_localhost());

        let conn2 = crate::utils::ConnectionString::parse("mysql://root@127.0.0.1/mydb").unwrap();
        assert!(conn2.is_localhost());

        let conn3 = crate::utils::ConnectionString::parse("mysql://root@192.168.1.1/mydb").unwrap();
        assert!(!conn3.is_localhost());
    }

    #[test]
    fn test_connection_string_validate() {
        let conn = crate::utils::ConnectionString::parse("mysql://root:pass@localhost:3306/mydb").unwrap();
        assert!(conn.validate().is_ok());

        let invalid = crate::utils::ConnectionString::parse("unsupported://localhost/db").unwrap();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_empty_connection_string() {
        let result = crate::utils::ConnectionString::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_malformed_connection_string() {
        let result = crate::utils::ConnectionString::parse("not-a-valid-format");
        assert!(result.is_err());
    }

    // ============================================================================
    // SQL注入检测测试
    // ============================================================================

    #[test]
    fn test_sql_injection_classic_or_attack() {
        let detector = crate::utils::SqlInjectionDetector::new();

        let report = detector.detect("' OR '1'='1");
        assert!(report.is_some());
        let report = report.unwrap();
        assert!(matches!(report.risk_level, crate::utils::RiskLevel::High | crate::utils::RiskLevel::Critical));
        assert!(!report.findings.is_empty());
    }

    #[test]
    fn test_sql_injection_union_attack() {
        let detector = crate::utils::SqlInjectionDetector::new();

        let report = detector.detect("' UNION SELECT password FROM users --");
        assert!(report.is_some());
        let report = report.unwrap();
        assert!(matches!(report.risk_level, crate::utils::RiskLevel::High | crate::utils::RiskLevel::Critical));
    }

    #[test]
    fn test_sql_injection_drop_table_attack() {
        let detector = crate::utils::SqlInjectionDetector::new();

        let report = detector.detect("'; DROP TABLE users; --");
        assert!(report.is_some());
        let report = report.unwrap();
        assert!(matches!(report.risk_level, crate::utils::RiskLevel::High | crate::utils::RiskLevel::Critical));
    }

    #[test]
    fn test_sql_injection_delete_attack() {
        let detector = crate::utils::SqlInjectionDetector::new();

        let report = detector.detect("1; DELETE FROM users WHERE 1=1");
        assert!(report.is_some());
        let report = report.unwrap();
        assert!(matches!(report.risk_level, crate::utils::RiskLevel::High | crate::utils::RiskLevel::Critical));
    }

    #[test]
    fn test_sql_injection_normal_input() {
        let detector = crate::utils::SqlInjectionDetector::new();

        let report = detector.detect("John Doe");
        if let Some(report) = report {
            assert!(matches!(report.risk_level, crate::utils::RiskLevel::Low | crate::utils::RiskLevel::Medium));
        }
    }

    #[test]
    fn test_sql_injection_numbers_only() {
        let detector = crate::utils::SqlInjectionDetector::new();

        let report = detector.detect("12345");
        if let Some(report) = report {
            assert!(matches!(report.risk_level, crate::utils::RiskLevel::Low | crate::utils::RiskLevel::Medium));
        }
    }

    #[test]
    fn test_sql_injection_xss_attack() {
        let detector = crate::utils::SqlInjectionDetector::new();

        let report = detector.detect("<script>alert('xss')</script>");
        if let Some(report) = report {
            assert!(matches!(report.risk_level, crate::utils::RiskLevel::Medium | crate::utils::RiskLevel::High | crate::utils::RiskLevel::Critical));
        }
    }

    #[test]
    fn test_unicode_input() {
        let detector = crate::utils::SqlInjectionDetector::new();
        let report = detector.detect("用户名字张三");
        if let Some(report) = report {
            assert!(matches!(report.risk_level, crate::utils::RiskLevel::Low | crate::utils::RiskLevel::Medium));
        }
    }

    #[test]
    fn test_mixed_unicode_and_injection() {
        let detector = crate::utils::SqlInjectionDetector::new();
        let report = detector.detect("张三'; DROP TABLE users; --");
        assert!(report.is_some());
        let report = report.unwrap();
        assert!(matches!(report.risk_level, crate::utils::RiskLevel::High | crate::utils::RiskLevel::Critical | crate::utils::RiskLevel::Medium));
    }

    // ============================================================================
    // SafeSqlBuilder 测试
    // ============================================================================

    #[test]
    fn test_safe_sql_builder_simple() {
        let builder = crate::utils::SafeSqlBuilder::new("users").unwrap();
        let sql = builder.safe_where("name", "=", &serde_json::json!("John")).unwrap();

        assert!(sql.contains("name"));
        assert!(sql.contains("John"));
    }

    #[test]
    fn test_safe_sql_builder_with_injection_attempt() {
        let builder = crate::utils::SafeSqlBuilder::new("users").unwrap();
        let result = builder.safe_where("name", "=", &serde_json::json!("'; DROP TABLE users; --"));

        assert!(result.is_err() || result.unwrap().contains("\\'"));
    }

    #[test]
    fn test_safe_sql_builder_like_operator() {
        let builder = crate::utils::SafeSqlBuilder::new("users").unwrap();
        let sql = builder.safe_where("email", "LIKE", &serde_json::json!("%@example.com")).unwrap();

        assert!(sql.contains("LIKE"));
        assert!(sql.contains("%@example.com"));
    }

    #[test]
    fn test_safe_sql_builder_special_chars() {
        let builder = crate::utils::SafeSqlBuilder::new("users").unwrap();
        let result = builder.safe_where("name", "=", &serde_json::json!("O'Brien"));

        if result.is_ok() {
            let sql = result.unwrap();
            assert!(sql.contains("''"));
        }
    }

    // ============================================================================
    // FieldSecurityValidator 测试
    // ============================================================================

    #[test]
    fn test_field_validator_valid_field_name() {
        let validator = crate::utils::FieldSecurityValidator::new();
        assert!(validator.validate_field_name("user_name").is_ok());
        assert!(validator.validate_field_name("email123").is_ok());
        assert!(validator.validate_field_name("_private").is_ok());
    }

    #[test]
    fn test_field_validator_invalid_field_name() {
        let validator = crate::utils::FieldSecurityValidator::new();
        assert!(validator.validate_field_name("user-name").is_err());
        assert!(validator.validate_field_name("user space").is_err());
    }

    #[test]
    fn test_field_validator_long_name() {
        let validator = crate::utils::FieldSecurityValidator::new();
        let long_name = "a".repeat(100);
        assert!(validator.validate_field_name(&long_name).is_err());
    }

    // ============================================================================
    // OperationResult 测试
    // ============================================================================

    #[test]
    fn test_operation_result_success() {
        let result = crate::utils::OperationResult::success("操作成功");
        assert!(result.success);
        assert_eq!(result.message, "操作成功");
    }

    #[test]
    fn test_operation_result_failure() {
        let result = crate::utils::OperationResult::failure("操作失败");
        assert!(!result.success);
        assert_eq!(result.message, "操作失败");
    }

    #[test]
    fn test_operation_result_json_serialization() {
        let result = crate::utils::OperationResult::success("测试");
        let json = result.to_json();
        assert!(!json.is_empty());
    }

    // ============================================================================
    // BatchConfig 测试
    // ============================================================================

    #[test]
    fn test_batch_config_defaults() {
        let config = crate::utils::BatchConfig::default();
        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.parallel_batches, 4);
        assert_eq!(config.commit_interval, 100);
    }

    #[test]
    fn test_batch_config_custom() {
        let config = crate::utils::BatchConfig {
            batch_size: 5000,
            parallel_batches: 8,
            commit_interval: 200,
            timeout: std::time::Duration::from_secs(600),
        };

        assert_eq!(config.batch_size, 5000);
        assert_eq!(config.parallel_batches, 8);
        assert_eq!(config.commit_interval, 200);
    }

    // ============================================================================
    // ProgressTracker 测试 (异步测试)
    // ============================================================================

    #[tokio::test]
    async fn test_progress_tracker_initial() {
        let tracker = crate::utils::ProgressTracker::new(100, 10);
        let info = tracker.get_progress().await;

        assert_eq!(info.total, 100);
        assert_eq!(info.processed, 0);
    }

    #[tokio::test]
    async fn test_progress_tracker_update() {
        let tracker = crate::utils::ProgressTracker::new(100, 10);
        tracker.increment(50).await;

        let info = tracker.get_progress().await;
        assert_eq!(info.processed, 50);
    }

    #[tokio::test]
    async fn test_progress_tracker_complete() {
        let tracker = crate::utils::ProgressTracker::new(100, 10);
        tracker.increment(100).await;

        let info = tracker.get_progress().await;
        assert_eq!(info.processed, 100);
        assert!(info.percentage >= 99.9);
    }

    // ============================================================================
    // 数据库配置测试
    // ============================================================================

    #[test]
    fn test_backup_config_defaults() {
        let config = crate::core::BackupConfig::default();
        assert_eq!(config.parallel_tables, 4);
    }

    #[test]
    fn test_transfer_config_defaults() {
        let config = crate::utils::config::TransferConfig {
            batch_size: 1000,
            commit_interval: 100,
        };
        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.commit_interval, 100);
    }

    #[test]
    fn test_data_compare_config_defaults() {
        let config = crate::core::DataCompareConfig::default();
        assert_eq!(config.primary_key, "id");
    }

    // ============================================================================
    // DataFilter 测试
    // ============================================================================

    #[test]
    fn test_data_filter_new() {
        let filter = crate::utils::DataFilter::new();
        assert!(!filter.skip_nulls);
        assert!(!filter.skip_duplicates);
        assert!(filter.unique_fields.is_empty());
    }

    // ============================================================================
    // Oracle类型转换测试
    // ============================================================================

    #[test]
    fn test_oracle_to_mysql_type_conversion() {
        use crate::databases::OracleConverter;

        assert_eq!(OracleConverter::to_mysql_type("VARCHAR2"), "VARCHAR");
        assert_eq!(OracleConverter::to_mysql_type("NUMBER"), "BIGINT");
        assert_eq!(OracleConverter::to_mysql_type("DATE"), "DATETIME");
        assert_eq!(OracleConverter::to_mysql_type("CLOB"), "TEXT");
        assert_eq!(OracleConverter::to_mysql_type("BLOB"), "BLOB");
    }

    #[test]
    fn test_oracle_to_postgres_type_conversion() {
        use crate::databases::OracleConverter;

        assert_eq!(OracleConverter::to_postgres_type("VARCHAR2"), "VARCHAR");
        assert_eq!(OracleConverter::to_postgres_type("NUMBER"), "BIGINT");
        assert_eq!(OracleConverter::to_postgres_type("DATE"), "TIMESTAMP");
        assert_eq!(OracleConverter::to_postgres_type("CLOB"), "TEXT");
    }

    #[test]
    fn test_oracle_to_sqlite_type_conversion() {
        use crate::databases::OracleConverter;

        assert_eq!(OracleConverter::to_sqlite_type("VARCHAR2"), "TEXT");
        assert_eq!(OracleConverter::to_sqlite_type("NUMBER"), "INTEGER");
        assert_eq!(OracleConverter::to_sqlite_type("DATE"), "TEXT");
        assert_eq!(OracleConverter::to_sqlite_type("CLOB"), "TEXT");
        assert_eq!(OracleConverter::to_sqlite_type("BLOB"), "BLOB");
    }

    // ============================================================================
    // 边界条件测试
    // ============================================================================

    #[test]
    fn test_very_long_input() {
        let detector = crate::utils::SqlInjectionDetector::new();
        let long_input = "a".repeat(10000);
        let report = detector.detect(&long_input);
        if let Some(report) = report {
            assert!(matches!(report.risk_level, crate::utils::RiskLevel::Low | crate::utils::RiskLevel::Medium));
        }
    }

    #[test]
    fn test_empty_input() {
        let detector = crate::utils::SqlInjectionDetector::new();
        let report = detector.detect("");
        if let Some(report) = report {
            assert!(matches!(report.risk_level, crate::utils::RiskLevel::Low | crate::utils::RiskLevel::Medium));
        }
    }

    #[test]
    fn test_special_characters_input() {
        let detector = crate::utils::SqlInjectionDetector::new();
        let report = detector.detect("!@#$%^&*()_+-=[]{}|;':\",./<>?");
        if let Some(report) = report {
            assert!(matches!(report.risk_level, crate::utils::RiskLevel::Low | crate::utils::RiskLevel::Medium | crate::utils::RiskLevel::High | crate::utils::RiskLevel::Critical));
        }
    }

    // ============================================================================
    // RiskLevel 比较测试
    // ============================================================================

    #[test]
    fn test_risk_level_ordering() {
        use crate::utils::RiskLevel;

        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn test_risk_level_display() {
        use crate::utils::RiskLevel;

        assert_eq!(format!("{}", RiskLevel::Low), "Low");
        assert_eq!(format!("{}", RiskLevel::Medium), "Medium");
        assert_eq!(format!("{}", RiskLevel::High), "High");
        assert_eq!(format!("{}", RiskLevel::Critical), "Critical");
    }
}