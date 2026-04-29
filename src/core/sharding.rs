/// 分库分表处理模块

use crate::models::FieldMapping;
use anyhow::Result;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// 分库分表策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardingStrategy {
    /// 按字段值哈希
    Hash,
    /// 按字段值范围
    Range,
    /// 按字段值列表
    List,
    /// 按时间
    Time,
    /// 按日期
    Date,
}

/// 分片键配置
#[derive(Debug, Clone)]
pub struct ShardingConfig {
    /// 分片键字段名
    pub sharding_key: String,
    /// 分片策略
    pub strategy: ShardingStrategy,
    /// 分片数量
    pub shard_count: usize,
    /// 分片参数
    pub shard_param: ShardParam,
    /// 目标表模板
    pub target_table_template: String,
}

impl ShardingConfig {
    pub fn new(sharding_key: &str, strategy: ShardingStrategy, shard_count: usize) -> Self {
        Self {
            sharding_key: sharding_key.to_string(),
            strategy,
            shard_count,
            shard_param: ShardParam::None,
            target_table_template: "{table}_{shard}".to_string(),
        }
    }

    pub fn with_param(mut self, param: ShardParam) -> Self {
        self.shard_param = param;
        self
    }

    pub fn with_table_template(mut self, template: &str) -> Self {
        self.target_table_template = template.to_string();
        self
    }
}

#[derive(Debug, Clone)]
pub enum ShardParam {
    /// 无参数
    None,
    /// 哈希参数
    HashParam { modulus: usize },
    /// 范围参数
    RangeParam { start: i64, end: i64 },
    /// 列表参数
    ListParam { values: Vec<String> },
    /// 时间格式
    TimeFormat { format: String },
}

/// 分片信息
#[derive(Debug, Clone)]
pub struct ShardInfo {
    /// 分片索引
    pub shard_index: usize,
    /// 目标数据库
    pub target_db: Option<String>,
    /// 目标表
    pub target_table: String,
    /// 记录数量
    pub record_count: usize,
}

/// 分库分表管理器
pub struct ShardingManager {
    config: ShardingConfig,
}

impl ShardingManager {
    pub fn new(config: ShardingConfig) -> Self {
        Self { config }
    }

    /// 计算分片索引
    pub fn calculate_shard(&self, value: &Value) -> Result<usize> {
        match &self.config.shard_param {
            ShardParam::None => {
                match value {
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Ok((i % self.config.shard_count as i64).abs() as usize)
                        } else if let Some(u) = n.as_u64() {
                            Ok((u % self.config.shard_count as u64) as usize)
                        } else {
                            Ok(0)
                        }
                    }
                    Value::String(s) => {
                        let hash = self.hash_string(s);
                        Ok(hash % self.config.shard_count)
                    }
                    _ => Ok(0),
                }
            }
            ShardParam::HashParam { modulus } => {
                let hash = match value {
                    Value::Number(n) => n.as_u64().unwrap_or(0) as usize,
                    Value::String(s) => self.hash_string(s),
                    _ => 0,
                };
                Ok(hash % modulus)
            }
            ShardParam::RangeParam { start, end } => {
                let range = (*end - *start) as usize;
                let range_per_shard = (range + self.config.shard_count - 1) / self.config.shard_count;
                let index = match value {
                    Value::Number(n) => {
                        let v = n.as_i64().unwrap_or(*start);
                        let offset = (v - start) as usize;
                        (offset / range_per_shard).min(self.config.shard_count - 1)
                    }
                    _ => 0,
                };
                Ok(index)
            }
            ShardParam::ListParam { values } => {
                let s = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => String::new(),
                };
                Ok(values.iter().position(|v| v == &s).unwrap_or(0))
            }
            ShardParam::TimeFormat { .. } => {
                // 时间分片由 calculate_shard_by_time 处理
                Ok(0)
            }
        }
    }

    /// 根据时间计算分片
    pub fn calculate_shard_by_time(&self, value: &Value) -> Result<usize> {
        let timestamp = match value {
            Value::Number(n) => n.as_i64().unwrap_or(0),
            Value::String(s) => {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.timestamp())
                    .unwrap_or_else(|_| {
                        // 尝试解析其他时间格式
                        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                            .map(|dt| dt.and_utc().timestamp())
                            .unwrap_or(0)
                    })
            }
            _ => 0,
        };

        let seconds = timestamp;
        let interval = match &self.config.shard_param {
            ShardParam::TimeFormat { format: _ } => 3600, // 默认1小时
            _ => 3600,
        };

        Ok(((seconds / interval) as usize) % self.config.shard_count)
    }

    /// 生成分片目标表名
    pub fn generate_target_table(&self, base_table: &str, shard_index: usize) -> String {
        self.config.target_table_template
            .replace("{table}", base_table)
            .replace("{shard}", &shard_index.to_string())
    }

    /// 计算数据应该路由到哪个分片
    pub fn route(&self, data: &Map<String, Value>) -> Result<ShardInfo> {
        let shard_key_value = data.get(&self.config.sharding_key)
            .ok_or_else(|| anyhow::anyhow!("Sharding key '{}' not found", self.config.sharding_key))?;

        let shard_index = match self.config.strategy {
            ShardingStrategy::Hash | ShardingStrategy::List => {
                self.calculate_shard(shard_key_value)?
            }
            ShardingStrategy::Range => {
                self.calculate_shard(shard_key_value)?
            }
            ShardingStrategy::Time | ShardingStrategy::Date => {
                self.calculate_shard_by_time(shard_key_value)?
            }
        };

        let target_table = self.generate_target_table("", shard_index);

        Ok(ShardInfo {
            shard_index,
            target_db: None,
            target_table,
            record_count: 0,
        })
    }

    /// 对数据进行分片
    pub fn shard_data(
        &self,
        _base_table: &str,
        data_list: &[Map<String, Value>],
    ) -> Result<HashMap<usize, Vec<(Map<String, Value>, usize)>>> {
        let mut shards: HashMap<usize, Vec<(Map<String, Value>, usize)>> = HashMap::new();

        for (idx, data) in data_list.iter().enumerate() {
            let shard_info = self.route(data)?;
            shards
                .entry(shard_info.shard_index)
                .or_insert_with(Vec::new)
                .push((data.clone(), idx));
        }

        Ok(shards)
    }

    /// 生成字段映射
    pub fn generate_shard_mappings(
        &self,
        source_table: &str,
        target_table: &str,
        source_field: &str,
        target_field: &str,
    ) -> FieldMapping {
        FieldMapping {
            source_table: source_table.to_string(),
            source_field: source_field.to_string(),
            target_table: target_table.to_string(),
            target_field: target_field.to_string(),
        }
    }

    /// 哈希字符串
    fn hash_string(&self, s: &str) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        (hasher.finish() as usize) % self.config.shard_count
    }
}

/// 分片迁移计划
#[derive(Debug, Clone)]
pub struct ShardMigrationPlan {
    pub shards: Vec<ShardMigrationTask>,
    pub total_records: usize,
}

#[derive(Debug, Clone)]
pub struct ShardMigrationTask {
    pub shard_index: usize,
    pub source_table: String,
    pub target_table: String,
    pub target_db: Option<String>,
    pub record_count: usize,
    pub mappings: Vec<FieldMapping>,
}

impl ShardMigrationPlan {
    pub fn format(&self) -> String {
        let mut output = format!("分片迁移计划（总计 {} 条记录）:\n\n", self.total_records);
        
        for task in &self.shards {
            output.push_str(&format!(
                "分片 {}: {} -> {} ({} 条记录)\n",
                task.shard_index,
                task.source_table,
                task.target_table,
                task.record_count
            ));
        }
        
        output
    }
}

/// 分片路由
pub struct ShardRouter {
    managers: Vec<ShardingManager>,
}

impl ShardRouter {
    pub fn new() -> Self {
        Self {
            managers: Vec::new(),
        }
    }

    pub fn add_manager(&mut self, manager: ShardingManager) {
        self.managers.push(manager);
    }

    /// 路由数据到正确的分片
    pub fn route(&self, data: &Map<String, Value>) -> Result<Vec<ShardInfo>> {
        let mut results = Vec::new();
        
        for manager in &self.managers {
            let shard_info = manager.route(data)?;
            results.push(shard_info);
        }
        
        Ok(results)
    }
}

impl Default for ShardRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局分片配置管理器
pub struct GlobalShardingManager {
    table_configs: HashMap<String, ShardingConfig>,
}

impl GlobalShardingManager {
    pub fn new() -> Self {
        Self {
            table_configs: HashMap::new(),
        }
    }

    pub fn register_table(&mut self, table_name: &str, config: ShardingConfig) {
        self.table_configs.insert(table_name.to_string(), config);
    }

    pub fn get_config(&self, table_name: &str) -> Option<&ShardingConfig> {
        self.table_configs.get(table_name)
    }

    pub fn create_manager(&self, table_name: &str) -> Result<ShardingManager> {
        let config = self.table_configs.get(table_name)
            .ok_or_else(|| anyhow::anyhow!("No sharding config for table '{}'", table_name))?;
        Ok(ShardingManager::new(config.clone()))
    }
}

impl Default for GlobalShardingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_sharding() {
        let config = ShardingConfig::new("user_id", ShardingStrategy::Hash, 4);
        let manager = ShardingManager::new(config);

        let mut data = Map::new();
        data.insert("user_id".to_string(), Value::Number(1.into()));
        data.insert("name".to_string(), Value::String("Alice".to_string()));

        let shard = manager.route(&data).unwrap();
        assert!(shard.shard_index < 4);

        println!("Hash sharding result: {:?}", shard);
    }

    #[test]
    fn test_range_sharding() {
        let config = ShardingConfig::new(
            "age",
            ShardingStrategy::Range,
            4,
        ).with_param(ShardParam::RangeParam { start: 0, end: 100 });

        let manager = ShardingManager::new(config);

        let mut data = Map::new();
        data.insert("age".to_string(), Value::Number(25.into()));

        let shard = manager.route(&data).unwrap();
        assert_eq!(shard.shard_index, 1); // 25 在 25-50 范围内

        println!("Range sharding result: {:?}", shard);
    }

    #[test]
    fn test_list_sharding() {
        let config = ShardingConfig::new(
            "region",
            ShardingStrategy::List,
            3,
        ).with_param(ShardParam::ListParam {
            values: vec![
                "Beijing".to_string(),
                "Shanghai".to_string(),
                "Guangzhou".to_string(),
            ],
        });

        let manager = ShardingManager::new(config);

        let mut data = Map::new();
        data.insert("region".to_string(), Value::String("Shanghai".to_string()));

        let shard = manager.route(&data).unwrap();
        assert_eq!(shard.shard_index, 1);

        println!("List sharding result: {:?}", shard);
    }

    #[test]
    fn test_time_sharding() {
        let config = ShardingConfig::new(
            "timestamp",
            ShardingStrategy::Time,
            24, // 24小时分片
        ).with_param(ShardParam::TimeFormat {
            format: "%Y-%m-%d %H:00:00".to_string(),
        });

        let manager = ShardingManager::new(config);

        let mut data = Map::new();
        data.insert("timestamp".to_string(), Value::Number(1640003600i64.into())); // 2021-12-20 12:33:20

        let shard = manager.route(&data).unwrap();
        assert!(shard.shard_index < 24);

        println!("Time sharding result: {:?}", shard);
    }

    #[test]
    fn test_batch_sharding() {
        let config = ShardingConfig::new("id", ShardingStrategy::Hash, 3);
        let manager = ShardingManager::new(config);

        let data_list = vec![
            {
                let mut m = Map::new();
                m.insert("id".to_string(), Value::Number(1.into()));
                m
            },
            {
                let mut m = Map::new();
                m.insert("id".to_string(), Value::Number(2.into()));
                m
            },
            {
                let mut m = Map::new();
                m.insert("id".to_string(), Value::Number(3.into()));
                m
            },
        ];

        let shards = manager.shard_data("users", &data_list).unwrap();

        assert_eq!(shards.len(), 3); // 3个不同的分片

        for (shard_idx, records) in &shards {
            println!("Shard {}: {} records", shard_idx, records.len());
        }
    }
}
