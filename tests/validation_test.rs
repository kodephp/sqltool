use sqltool::{DataValidator, DataTransformer, DataFilter};
use sqltool::utils::{BatchConfig, BatchProcessor, ProgressTracker};

#[tokio::test]
async fn test_data_validator_string_field() {
    let validator = DataValidator::new(false);
    let field = sqltool::Field {
        name: "test".to_string(),
        data_type: "VARCHAR".to_string(),
        length: Some(10),
        nullable: true,
        default_value: None,
        primary_key: false,
        auto_increment: false,
    };

    let result = validator.validate_value(&serde_json::Value::String("test".to_string()), &field);
    assert!(result.is_ok());

    let result = validator.validate_value(&serde_json::Value::Null, &field);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_data_validator_non_nullable_field() {
    let validator = DataValidator::new(false);
    let field = sqltool::Field {
        name: "test".to_string(),
        data_type: "INTEGER".to_string(),
        length: None,
        nullable: false,
        default_value: None,
        primary_key: false,
        auto_increment: false,
    };

    let result = validator.validate_value(&serde_json::Value::Null, &field);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_data_validator_length_exceeded() {
    let validator = DataValidator::new(false);
    let field = sqltool::Field {
        name: "test".to_string(),
        data_type: "VARCHAR".to_string(),
        length: Some(5),
        nullable: true,
        default_value: None,
        primary_key: false,
        auto_increment: false,
    };

    let result = validator.validate_value(&serde_json::Value::String("longer than 5".to_string()), &field);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_data_transformer_integer_conversion() {
    let transformer = DataTransformer::new();
    let field = sqltool::Field {
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

#[tokio::test]
async fn test_data_transformer_float_conversion() {
    let transformer = DataTransformer::new();
    let field = sqltool::Field {
        name: "price".to_string(),
        data_type: "FLOAT".to_string(),
        length: None,
        nullable: true,
        default_value: None,
        primary_key: false,
        auto_increment: false,
    };

    let result = transformer.transform_value(serde_json::Value::String("19.99".to_string()), &field);
    assert!(result.is_ok());
    if let Ok(serde_json::Value::Number(n)) = result {
        assert_eq!(n.as_f64().unwrap(), 19.99);
    }
}

#[tokio::test]
async fn test_data_transformer_boolean_conversion() {
    let transformer = DataTransformer::new();
    let field = sqltool::Field {
        name: "active".to_string(),
        data_type: "BOOLEAN".to_string(),
        length: None,
        nullable: true,
        default_value: None,
        primary_key: false,
        auto_increment: false,
    };

    let result = transformer.transform_value(serde_json::Value::String("true".to_string()), &field);
    assert!(result.is_ok());
    if let Ok(serde_json::Value::Bool(b)) = result {
        assert!(b);
    }

    let result = transformer.transform_value(serde_json::Value::String("1".to_string()), &field);
    assert!(result.is_ok());
    if let Ok(serde_json::Value::Bool(b)) = result {
        assert!(b);
    }
}

#[tokio::test]
async fn test_data_filter_duplicates() {
    let mut filter = DataFilter::new();
    filter.skip_duplicates = true;
    
    let mut records = vec![
        vec![serde_json::Value::String("a".to_string())],
        vec![serde_json::Value::String("b".to_string())],
        vec![serde_json::Value::String("a".to_string())],
    ];
    
    let fields = vec!["field1".to_string()];
    let removed = filter.filter_duplicates(&mut records, &fields);
    
    assert_eq!(removed, 1);
    assert_eq!(records.len(), 2);
}

#[tokio::test]
async fn test_batch_config() {
    let config = BatchConfig::new(1000)
        .with_parallel_batches(8)
        .with_commit_interval(500);
    
    assert_eq!(config.batch_size, 1000);
    assert_eq!(config.parallel_batches, 8);
    assert_eq!(config.commit_interval, 500);
}

#[tokio::test]
async fn test_progress_tracker() {
    let tracker = ProgressTracker::new(100, 10);
    
    tracker.increment(10).await;
    let progress = tracker.get_progress().await;
    
    assert_eq!(progress.total, 100);
    assert_eq!(progress.processed, 10);
    assert!((progress.percentage - 10.0).abs() < 0.01);
}
