use sqltool::{SmartFieldMatcher, TableSchema, Field};

#[test]
fn test_smart_field_matching() {
    // 创建源表结构
    let source_schema = TableSchema {
        name: "users".to_string(),
        fields: vec![
            Field {
                name: "id".to_string(),
                data_type: "int".to_string(),
                length: Some(11),
                nullable: false,
                default_value: None,
                primary_key: true,
                auto_increment: true,
            },
            Field {
                name: "name".to_string(),
                data_type: "varchar".to_string(),
                length: Some(255),
                nullable: false,
                default_value: None,
                primary_key: false,
                auto_increment: false,
            },
            Field {
                name: "email".to_string(),
                data_type: "varchar".to_string(),
                length: Some(255),
                nullable: false,
                default_value: None,
                primary_key: false,
                auto_increment: false,
            },
        ],
        indexes: vec![],
        foreign_keys: vec![],
    };

    // 创建目标表结构
    let target_schema = TableSchema {
        name: "customers".to_string(),
        fields: vec![
            Field {
                name: "id".to_string(),
                data_type: "int".to_string(),
                length: Some(11),
                nullable: false,
                default_value: None,
                primary_key: true,
                auto_increment: true,
            },
            Field {
                name: "full_name".to_string(),
                data_type: "varchar".to_string(),
                length: Some(255),
                nullable: false,
                default_value: None,
                primary_key: false,
                auto_increment: false,
            },
            Field {
                name: "email_address".to_string(),
                data_type: "varchar".to_string(),
                length: Some(255),
                nullable: false,
                default_value: None,
                primary_key: false,
                auto_increment: false,
            },
        ],
        indexes: vec![],
        foreign_keys: vec![],
    };

    // 创建智能字段匹配器
    let matcher = SmartFieldMatcher::with_default_config();

    // 执行匹配
    let mappings = matcher.match_fields(&source_schema, &target_schema);

    // 验证匹配结果
    assert_eq!(mappings.len(), 3);
    assert!(mappings.iter().any(|m| m.source_field == "id" && m.target_field == "id"));
    assert!(mappings.iter().any(|m| m.source_field == "name" && m.target_field == "full_name"));
    assert!(mappings.iter().any(|m| m.source_field == "email" && m.target_field == "email_address"));
}
