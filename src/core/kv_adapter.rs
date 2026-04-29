/// Key-Value 存储适配器 - 处理 Redis、MongoDB 等非关系型数据库

use crate::models::FieldMapping;
use anyhow::Result;
use serde_json::{Map, Value};

/// Key-Value 存储数据类型
#[derive(Debug, Clone)]
pub enum KvDataType {
    /// 字符串
    String,
    /// 哈希
    Hash,
    /// 列表
    List,
    /// 集合
    Set,
    /// 有序集合
    SortedSet,
    /// JSON
    Json,
}

/// Key-Value 存储转换配置
#[derive(Debug, Clone)]
pub struct KvConversionConfig {
    /// 主键字段（将作为 Redis Key 的基础）
    pub primary_key_field: String,
    /// Key 前缀
    pub key_prefix: String,
    /// Key 格式模板
    pub key_template: String,
    /// 过期时间（秒），0 表示不过期
    pub ttl_seconds: u64,
    /// JSON 序列化模式
    pub json_mode: JsonMode,
    /// 字段合并策略
    pub field_merge_strategy: FieldMergeStrategy,
}

impl Default for KvConversionConfig {
    fn default() -> Self {
        Self {
            primary_key_field: "id".to_string(),
            key_prefix: "db:".to_string(),
            key_template: "{prefix}{pk_value}".to_string(),
            ttl_seconds: 0,
            json_mode: JsonMode::Flatten,
            field_merge_strategy: FieldMergeStrategy::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonMode {
    /// 扁平化：所有字段平铺
    Flatten,
    /// 嵌套：保留原有结构
    Nested,
    /// 混合：指定字段嵌套
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMergeStrategy {
    /// JSON：所有字段合并为一个 JSON
    Json,
    /// 哈希：使用 Redis Hash 存储
    Hash,
    /// 单独键：每个字段单独作为键
    Separate,
}

/// Redis 转换器
pub struct RedisConverter {
    config: KvConversionConfig,
}

impl RedisConverter {
    pub fn new(config: KvConversionConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(KvConversionConfig::default())
    }

    /// 将关系型数据转换为 Redis 格式
    pub fn convert_to_redis(
        &self,
        table_name: &str,
        data: &Map<String, Value>,
        mappings: &[FieldMapping],
    ) -> Result<Vec<RedisCommand>> {
        let mut commands = Vec::new();
        
        // 获取主键值
        let pk_value = self.get_field_value(data, &self.config.primary_key_field)?;
        
        // 生成 Key
        let key = self.generate_key(table_name, &pk_value)?;
        
        // 根据字段合并策略生成命令
        match self.config.field_merge_strategy {
            FieldMergeStrategy::Json => {
                let json_value = self.convert_to_json(data, mappings)?;
                commands.push(RedisCommand::Set {
                    key: key.clone(),
                    value: json_value,
                    ttl: self.config.ttl_seconds,
                });
            }
            FieldMergeStrategy::Hash => {
                let hash_fields = self.convert_to_hash(data, mappings)?;
                commands.push(RedisCommand::HSet {
                    key: key.clone(),
                    fields: hash_fields,
                });
                if self.config.ttl_seconds > 0 {
                    commands.push(RedisCommand::Expire {
                        key: key,
                        seconds: self.config.ttl_seconds,
                    });
                }
            }
            FieldMergeStrategy::Separate => {
                for mapping in mappings {
                    if let Some(value) = data.get(&mapping.source_field) {
                        let field_key = format!("{}:{}", key, mapping.target_field);
                        commands.push(RedisCommand::Set {
                            key: field_key,
                            value: value.to_string(),
                            ttl: self.config.ttl_seconds,
                        });
                    }
                }
            }
        }
        
        Ok(commands)
    }

    /// 转换为 JSON 字符串
    fn convert_to_json(&self, data: &Map<String, Value>, mappings: &[FieldMapping]) -> Result<String> {
        let mut target_data = Map::new();
        
        for mapping in mappings {
            if let Some(value) = data.get(&mapping.source_field) {
                target_data.insert(mapping.target_field.clone(), value.clone());
            }
        }
        
        Ok(serde_json::to_string(&target_data)?)
    }

    /// 转换为 Redis Hash
    fn convert_to_hash(&self, data: &Map<String, Value>, mappings: &[FieldMapping]) -> Result<Vec<(String, String)>> {
        let mut fields = Vec::new();
        
        for mapping in mappings {
            if let Some(value) = data.get(&mapping.source_field) {
                fields.push((mapping.target_field.clone(), value.to_string()));
            }
        }
        
        Ok(fields)
    }

    /// 获取字段值
    fn get_field_value(&self, data: &Map<String, Value>, field_name: &str) -> Result<String> {
        data.get(field_name)
            .map(|v| v.to_string())
            .ok_or_else(|| anyhow::anyhow!("Field '{}' not found", field_name))
    }

    /// 生成 Key
    fn generate_key(&self, table_name: &str, pk_value: &str) -> Result<String> {
        let key = self.config.key_template
            .replace("{prefix}", &self.config.key_prefix)
            .replace("{table}", table_name)
            .replace("{pk}", &self.config.primary_key_field)
            .replace("{pk_value}", pk_value);
        
        Ok(key)
    }

    /// 生成 Redis 命令列表
    pub fn generate_commands(&self, table_name: &str, data_list: &[Map<String, Value>], mappings: &[FieldMapping]) -> Result<Vec<RedisCommand>> {
        let mut all_commands = Vec::new();
        
        for data in data_list {
            let commands = self.convert_to_redis(table_name, data, mappings)?;
            all_commands.extend(commands);
        }
        
        Ok(all_commands)
    }
}

/// Redis 命令
#[derive(Debug, Clone)]
pub enum RedisCommand {
    Set { key: String, value: String, ttl: u64 },
    HSet { key: String, fields: Vec<(String, String)> },
    Expire { key: String, seconds: u64 },
    Del { key: String },
}

impl RedisCommand {
    /// 转换为 Redis 协议格式
    pub fn to_redis_proto(&self) -> String {
        match self {
            RedisCommand::Set { key, value, .. } => {
                format!("*3\r\n$3\r\nSET\r\n${}\r\n{}\r\n${}\r\n{}", 
                    key.len(), key, value.len(), value)
            }
            RedisCommand::HSet { key, fields } => {
                let mut cmd = format!("*3\r\n$4\r\nHSET\r\n${}\r\n{}\r\n", key.len(), key);
                for (field, value) in fields {
                    cmd.push_str(&format!("${}\r\n{}\r\n${}\r\n{}\r\n", 
                        field.len(), field, value.len(), value));
                }
                cmd
            }
            RedisCommand::Expire { key, seconds } => {
                format!("*3\r\n$6\r\nEXPIRE\r\n${}\r\n{}\r\n${}\r\n{}\r\n",
                    key.len(), key, seconds.to_string().len(), seconds)
            }
            RedisCommand::Del { key } => {
                format!("*2\r\n$3\r\nDEL\r\n${}\r\n{}\r\n",
                    key.len(), key)
            }
        }
    }
}

/// MongoDB 转换器
pub struct MongoDbConverter {
    config: KvConversionConfig,
}

impl MongoDbConverter {
    pub fn new(config: KvConversionConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(KvConversionConfig::default())
    }

    /// 将关系型数据转换为 MongoDB 文档
    pub fn convert_to_mongo(
        &self,
        collection_name: &str,
        data: &Map<String, Value>,
        mappings: &[FieldMapping],
    ) -> Result<MongoOperation> {
        let mut document = Map::new();
        
        for mapping in mappings {
            if let Some(value) = data.get(&mapping.source_field) {
                document.insert(mapping.target_field.clone(), value.clone());
            }
        }
        
        Ok(MongoOperation::InsertOne {
            collection: collection_name.to_string(),
            document,
        })
    }

    /// 生成批量插入操作
    pub fn convert_batch(
        &self,
        collection_name: &str,
        data_list: &[Map<String, Value>],
        mappings: &[FieldMapping],
    ) -> Result<Vec<MongoOperation>> {
        let mut operations = Vec::new();
        
        for data in data_list {
            let op = self.convert_to_mongo(collection_name, data, mappings)?;
            operations.push(op);
        }
        
        Ok(operations)
    }
}

/// MongoDB 操作
#[derive(Debug, Clone)]
pub enum MongoOperation {
    InsertOne { collection: String, document: Map<String, Value> },
    InsertMany { collection: String, documents: Vec<Map<String, Value>> },
    UpdateOne { collection: String, filter: Map<String, Value>, update: Map<String, Value> },
    UpdateMany { collection: String, filter: Map<String, Value>, update: Map<String, Value> },
    DeleteOne { collection: String, filter: Map<String, Value> },
    DeleteMany { collection: String, filter: Map<String, Value> },
}

/// Key-Value 存储统一接口
pub trait KvStoreConverter {
    fn convert(&self, table_name: &str, data: &Map<String, Value>, mappings: &[FieldMapping]) -> Result<KvConversionResult>;
}

/// 转换结果
#[derive(Debug, Clone)]
pub struct KvConversionResult {
    pub key: String,
    pub value: Value,
    pub ttl_seconds: Option<u64>,
    pub metadata: Option<Map<String, Value>>,
}

impl KvConversionResult {
    pub fn new(key: String, value: Value) -> Self {
        Self {
            key,
            value,
            ttl_seconds: None,
            metadata: None,
        }
    }

    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl_seconds = Some(ttl);
        self
    }

    pub fn with_metadata(mut self, metadata: Map<String, Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// 转换为 Redis 命令
    pub fn to_redis_command(&self) -> RedisCommand {
        RedisCommand::Set {
            key: self.key.clone(),
            value: serde_json::to_string(&self.value).unwrap_or_default(),
            ttl: self.ttl_seconds.unwrap_or(0),
        }
    }

    /// 转换为 MongoDB 文档
    pub fn to_mongo_document(&self) -> Map<String, Value> {
        let mut doc = Map::new();
        doc.insert("_id".to_string(), Value::String(self.key.clone()));
        doc.insert("data".to_string(), self.value.clone());
        
        if let Some(ref metadata) = self.metadata {
            for (k, v) in metadata {
                doc.insert(k.clone(), v.clone());
            }
        }
        
        doc
    }
}

/// 批量转换器
pub struct BatchKvConverter {
    converters: Vec<Box<dyn KvStoreConverter>>,
}

impl BatchKvConverter {
    pub fn new() -> Self {
        Self {
            converters: Vec::new(),
        }
    }

    pub fn add_converter<C: KvStoreConverter + 'static>(&mut self, converter: C) {
        self.converters.push(Box::new(converter));
    }

    /// 批量转换数据
    pub fn convert_batch(
        &self,
        table_name: &str,
        data_list: &[Map<String, Value>],
        mappings: &[FieldMapping],
    ) -> Result<Vec<KvConversionResult>> {
        let mut results = Vec::new();
        
        for converter in &self.converters {
            for data in data_list {
                let result = converter.convert(table_name, data, mappings)?;
                results.push(result);
            }
        }
        
        Ok(results)
    }
}

impl Default for BatchKvConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_converter_basic() {
        let converter = RedisConverter::with_default_config();
        
        let mut data = Map::new();
        data.insert("id".to_string(), Value::Number(1.into()));
        data.insert("name".to_string(), Value::String("Alice".to_string()));
        data.insert("email".to_string(), Value::String("alice@example.com".to_string()));
        
        let mappings = vec![
            FieldMapping {
                source_table: "users".to_string(),
                source_field: "id".to_string(),
                target_table: "users".to_string(),
                target_field: "id".to_string(),
            },
            FieldMapping {
                source_table: "users".to_string(),
                source_field: "name".to_string(),
                target_table: "users".to_string(),
                target_field: "name".to_string(),
            },
        ];
        
        let commands = converter.convert_to_redis("users", &data, &mappings).unwrap();
        
        assert!(!commands.is_empty());
        println!("生成的 Redis 命令: {:?}", commands);
    }

    #[test]
    fn test_key_generation() {
        let config = KvConversionConfig {
            primary_key_field: "id".to_string(),
            key_prefix: "user:".to_string(),
            key_template: "{prefix}{pk_value}".to_string(),
            ttl_seconds: 3600,
            json_mode: JsonMode::Flatten,
            field_merge_strategy: FieldMergeStrategy::Json,
        };
        
        let converter = RedisConverter::new(config);
        
        let mut data = Map::new();
        data.insert("id".to_string(), Value::Number(123.into()));
        
        let key = converter.generate_key("users", "123").unwrap();
        assert_eq!(key, "user:123");
        
        println!("生成的 Key: {}", key);
    }

    #[test]
    fn test_mongodb_converter() {
        let converter = MongoDbConverter::with_default_config();
        
        let mut data = Map::new();
        data.insert("id".to_string(), Value::Number(1.into()));
        data.insert("name".to_string(), Value::String("Bob".to_string()));
        
        let mappings = vec![
            FieldMapping {
                source_table: "users".to_string(),
                source_field: "id".to_string(),
                target_table: "users".to_string(),
                target_field: "_id".to_string(),
            },
        ];
        
        let op = converter.convert_to_mongo("users", &data, &mappings).unwrap();

        match &op {
            MongoOperation::InsertOne { collection, document } => {
                assert_eq!(collection, "users");
                assert!(document.contains_key("_id"));
            }
            _ => panic!("Expected InsertOne"),
        }

        println!("生成的 MongoDB 操作: {:?}", op);
    }
}
