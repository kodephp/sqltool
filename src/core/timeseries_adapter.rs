/// 时序数据库适配器 - 处理 InfluxDB、TimescaleDB、Taodb 等时序数据库

use crate::models::FieldMapping;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// 时间序列数据类型
#[derive(Debug, Clone)]
pub enum TimeSeriesDataType {
    /// 浮点值
    Float,
    /// 整数值
    Integer,
    /// 无符号整数值
    UInteger,
    /// 布尔值
    Boolean,
    /// 字符串
    String,
    /// 字节数组
    Bytes,
}

/// 时序数据库配置
#[derive(Debug, Clone)]
pub struct TimeSeriesConfig {
    /// 时间戳字段
    pub timestamp_field: String,
    /// 指标字段列表
    pub metric_fields: Vec<String>,
    /// 标签字段列表
    pub tag_fields: Vec<String>,
    /// 保留策略
    pub retention_policy: Option<String>,
    /// 数据库名称
    pub database: String,
    /// 测量（表）名称
    pub measurement: String,
    /// 分区策略
    pub partition_strategy: PartitionStrategy,
}

impl Default for TimeSeriesConfig {
    fn default() -> Self {
        Self {
            timestamp_field: "timestamp".to_string(),
            metric_fields: Vec::new(),
            tag_fields: Vec::new(),
            retention_policy: None,
            database: "default".to_string(),
            measurement: "data".to_string(),
            partition_strategy: PartitionStrategy::ByTime {
                interval: TimeInterval::Hours(1),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PartitionStrategy {
    /// 按时间分区
    ByTime { interval: TimeInterval },
    /// 按标签值分区
    ByTag { tag: String },
    /// 按数值范围分区
    ByValue { size: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInterval {
    Minutes(u32),
    Hours(u32),
    Days(u32),
}

impl TimeInterval {
    pub fn as_seconds(&self) -> u64 {
        match self {
            TimeInterval::Minutes(m) => (*m as u64) * 60,
            TimeInterval::Hours(h) => (*h as u64) * 3600,
            TimeInterval::Days(d) => (*d as u64) * 86400,
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            TimeInterval::Minutes(m) => format!("{}m", m),
            TimeInterval::Hours(h) => format!("{}h", h),
            TimeInterval::Days(d) => format!("{}d", d),
        }
    }
}

/// 时序数据库统一接口
pub trait TimeSeriesConverter {
    /// 转换数据
    fn convert(&self, data: &HashMap<String, Value>, mappings: &[FieldMapping]) -> Result<TimeSeriesPoint>;
    
    /// 批量转换
    fn convert_batch(&self, data_list: &[HashMap<String, Value>], mappings: &[FieldMapping]) -> Result<Vec<TimeSeriesPoint>>;
}

/// InfluxDB 转换器
pub struct InfluxDbConverter {
    config: TimeSeriesConfig,
}

impl InfluxDbConverter {
    pub fn new(config: TimeSeriesConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(TimeSeriesConfig::default())
    }

    /// 转换为 InfluxDB 行协议
    pub fn convert_to_line_protocol(
        &self,
        data: &HashMap<String, Value>,
        mappings: &[FieldMapping],
    ) -> Result<String> {
        let point = self.convert(data, mappings)?;
        Ok(self.point_to_line_protocol(&point))
    }

    /// 将点转换为行协议
    fn point_to_line_protocol(&self, point: &TimeSeriesPoint) -> String {
        let mut output = format!("{},", self.config.measurement);
        
        // 添加标签
        let tags: Vec<String> = point.tags.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        output.push_str(&tags.join(","));
        output.push(' ');
        
        // 添加指标
        let fields: Vec<String> = point.fields.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        output.push_str(&fields.join(","));
        output.push(' ');
        
        // 添加时间戳
        output.push_str(&point.timestamp.to_string());
        
        output
    }
}

impl TimeSeriesConverter for InfluxDbConverter {
    fn convert(&self, data: &HashMap<String, Value>, _mappings: &[FieldMapping]) -> Result<TimeSeriesPoint> {
        let mut point = TimeSeriesPoint::default();
        
        // 提取时间戳
        point.timestamp = data.get(&self.config.timestamp_field)
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        
        // 提取标签（字符串字段作为标签）
        for tag_field in &self.config.tag_fields {
            if let Some(value) = data.get(tag_field) {
                if let Some(s) = value.as_str() {
                    point.tags.insert(tag_field.clone(), s.to_string());
                }
            }
        }
        
        // 提取指标（数值字段作为指标）
        for metric_field in &self.config.metric_fields {
            if let Some(value) = data.get(metric_field) {
                let metric_value = match value {
                    Value::Number(n) => {
                        if n.is_f64() {
                            MetricValue::Float(n.as_f64().unwrap())
                        } else if n.is_i64() {
                            MetricValue::Integer(n.as_i64().unwrap())
                        } else {
                            MetricValue::Float(n.to_string().parse().unwrap_or(0.0))
                        }
                    }
                    Value::Bool(b) => MetricValue::Boolean(*b),
                    Value::String(s) => MetricValue::String(s.clone()),
                    _ => continue,
                };
                point.fields.insert(metric_field.clone(), metric_value);
            }
        }
        
        Ok(point)
    }

    fn convert_batch(&self, data_list: &[HashMap<String, Value>], mappings: &[FieldMapping]) -> Result<Vec<TimeSeriesPoint>> {
        let mut points = Vec::new();
        for data in data_list {
            let point = self.convert(data, mappings)?;
            points.push(point);
        }
        Ok(points)
    }
}

/// 时序数据点
#[derive(Debug, Clone, Default)]
pub struct TimeSeriesPoint {
    /// 测量名称
    pub measurement: String,
    /// 标签
    pub tags: HashMap<String, String>,
    /// 指标字段
    pub fields: HashMap<String, MetricValue>,
    /// 时间戳（纳秒）
    pub timestamp: i64,
}

impl TimeSeriesPoint {
    pub fn new(measurement: &str) -> Self {
        Self {
            measurement: measurement.to_string(),
            ..Default::default()
        }
    }

    pub fn with_tag(mut self, key: &str, value: &str) -> Self {
        self.tags.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_field(mut self, key: &str, value: MetricValue) -> Self {
        self.fields.insert(key.to_string(), value);
        self
    }

    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }
}

/// 指标值
#[derive(Debug, Clone)]
pub enum MetricValue {
    Float(f64),
    Integer(i64),
    UInteger(u64),
    Boolean(bool),
    String(String),
}

impl std::fmt::Display for MetricValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricValue::Float(v) => write!(f, "{}", v),
            MetricValue::Integer(v) => write!(f, "{}i", v),
            MetricValue::UInteger(v) => write!(f, "{}u", v),
            MetricValue::Boolean(v) => write!(f, "{}", v),
            MetricValue::String(v) => write!(f, "\"{}\"", v),
        }
    }
}

impl MetricValue {
    pub fn as_f64(&self) -> f64 {
        match self {
            MetricValue::Float(v) => *v,
            MetricValue::Integer(v) => *v as f64,
            MetricValue::UInteger(v) => *v as f64,
            MetricValue::Boolean(v) => if *v { 1.0 } else { 0.0 },
            MetricValue::String(v) => v.parse().unwrap_or(0.0),
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            MetricValue::Float(v) => format!("{}", v),
            MetricValue::Integer(v) => format!("{}i", v),
            MetricValue::UInteger(v) => format!("{}u", v),
            MetricValue::Boolean(v) => format!("{}", v),
            MetricValue::String(v) => format!("\"{}\"", v),
        }
    }
}

/// TimescaleDB 转换器
pub struct TimescaleDbConverter {
    config: TimeSeriesConfig,
}

impl TimescaleDbConverter {
    pub fn new(config: TimeSeriesConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(TimeSeriesConfig::default())
    }

    /// 生成创建超表的 SQL
    pub fn generate_hypertable_sql(&self) -> String {
        let partition_interval = match self.config.partition_strategy {
            PartitionStrategy::ByTime { interval } => interval.as_string(),
            _ => "1 day".to_string(),
        };
        
        format!(
            "SELECT create_hypertable('{}.{}', '{}', chunk_time_interval => INTERVAL '{}');",
            self.config.database,
            self.config.measurement,
            self.config.timestamp_field,
            partition_interval
        )
    }
}

impl TimeSeriesConverter for TimescaleDbConverter {
    fn convert(&self, data: &HashMap<String, Value>, _mappings: &[FieldMapping]) -> Result<TimeSeriesPoint> {
        let mut point = TimeSeriesPoint::new(&self.config.measurement);
        
        // 提取时间戳
        point.timestamp = data.get(&self.config.timestamp_field)
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        
        // 提取标签
        for tag_field in &self.config.tag_fields {
            if let Some(value) = data.get(tag_field) {
                if let Some(s) = value.as_str() {
                    point.tags.insert(tag_field.clone(), s.to_string());
                }
            }
        }
        
        // 提取指标
        for metric_field in &self.config.metric_fields {
            if let Some(value) = data.get(metric_field) {
                let metric_value = match value {
                    Value::Number(n) => {
                        if n.is_f64() {
                            MetricValue::Float(n.as_f64().unwrap())
                        } else {
                            MetricValue::Integer(n.as_i64().unwrap_or(0))
                        }
                    }
                    Value::Bool(b) => MetricValue::Boolean(*b),
                    Value::String(s) => MetricValue::String(s.clone()),
                    _ => continue,
                };
                point.fields.insert(metric_field.clone(), metric_value);
            }
        }
        
        Ok(point)
    }

    fn convert_batch(&self, data_list: &[HashMap<String, Value>], mappings: &[FieldMapping]) -> Result<Vec<TimeSeriesPoint>> {
        let mut points = Vec::new();
        for data in data_list {
            let point = self.convert(data, mappings)?;
            points.push(point);
        }
        Ok(points)
    }
}

/// Taodb 转换器
pub struct TaodbConverter {
    config: TimeSeriesConfig,
}

impl TaodbConverter {
    pub fn new(config: TimeSeriesConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(TimeSeriesConfig::default())
    }

    /// 转换为 Taodb 格式
    pub fn convert_to_taodb(
        &self,
        data: &HashMap<String, Value>,
        _mappings: &[FieldMapping],
    ) -> Result<TaodbRecord> {
        let mut record = TaodbRecord::default();
        
        // 设置时间戳
        record.timestamp = data.get(&self.config.timestamp_field)
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        
        // 设置标签
        for tag_field in &self.config.tag_fields {
            if let Some(value) = data.get(tag_field) {
                if let Some(s) = value.as_str() {
                    record.tags.insert(tag_field.clone(), s.to_string());
                }
            }
        }
        
        // 设置指标
        for metric_field in &self.config.metric_fields {
            if let Some(value) = data.get(metric_field) {
                if let Some(n) = value.as_f64() {
                    record.metrics.insert(metric_field.clone(), n);
                }
            }
        }
        
        Ok(record)
    }

    /// 生成 Taodb 插入语句
    pub fn generate_insert_sql(&self, record: &TaodbRecord) -> String {
        let tags: Vec<String> = record.tags.iter()
            .map(|(k, v)| format!("tag {}='{}'", k, v))
            .collect();
        
        let metrics: Vec<String> = record.metrics.iter()
            .map(|(k, v)| format!("field {}={}", k, v))
            .collect();
        
        format!(
            "INSERT INTO {} ({}) TAGS ({}) VALUES ({}) {}",
            self.config.measurement,
            metrics.join(", "),
            tags.join(", "),
            self.config.timestamp_field,
            record.timestamp
        )
    }
}

impl TimeSeriesConverter for TaodbConverter {
    fn convert(&self, data: &HashMap<String, Value>, _mappings: &[FieldMapping]) -> Result<TimeSeriesPoint> {
        let mut point = TimeSeriesPoint::new(&self.config.measurement);
        
        point.timestamp = data.get(&self.config.timestamp_field)
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        
        for tag_field in &self.config.tag_fields {
            if let Some(value) = data.get(tag_field) {
                if let Some(s) = value.as_str() {
                    point.tags.insert(tag_field.clone(), s.to_string());
                }
            }
        }
        
        for metric_field in &self.config.metric_fields {
            if let Some(value) = data.get(metric_field) {
                let metric_value = match value {
                    Value::Number(n) => {
                        if n.is_f64() {
                            MetricValue::Float(n.as_f64().unwrap())
                        } else {
                            MetricValue::Integer(n.as_i64().unwrap_or(0))
                        }
                    }
                    Value::Bool(b) => MetricValue::Boolean(*b),
                    Value::String(s) => MetricValue::String(s.clone()),
                    _ => continue,
                };
                point.fields.insert(metric_field.clone(), metric_value);
            }
        }
        
        Ok(point)
    }

    fn convert_batch(&self, data_list: &[HashMap<String, Value>], mappings: &[FieldMapping]) -> Result<Vec<TimeSeriesPoint>> {
        let mut points = Vec::new();
        for data in data_list {
            let point = self.convert(data, mappings)?;
            points.push(point);
        }
        Ok(points)
    }
}

/// Taodb 记录
#[derive(Debug, Clone, Default)]
pub struct TaodbRecord {
    pub timestamp: i64,
    pub tags: HashMap<String, String>,
    pub metrics: HashMap<String, f64>,
}

/// 时序数据批量写入器
pub struct TimeSeriesBatchWriter {
    converters: Vec<Box<dyn TimeSeriesConverter>>,
    batch_size: usize,
}

impl TimeSeriesBatchWriter {
    pub fn new(batch_size: usize) -> Self {
        Self {
            converters: Vec::new(),
            batch_size,
        }
    }

    pub fn add_converter<C: TimeSeriesConverter + 'static>(&mut self, converter: C) {
        self.converters.push(Box::new(converter));
    }

    /// 批量写入
    pub fn write_batch(
        &self,
        data_list: &[HashMap<String, Value>],
        mappings: &[FieldMapping],
    ) -> Result<BatchWriteResult> {
        let mut total_points = 0;
        let mut batches = Vec::new();
        
        for converter in &self.converters {
            let points = converter.convert_batch(data_list, mappings)?;
            total_points += points.len();
            batches.push(points);
        }
        
        Ok(BatchWriteResult {
            total_points,
            batch_count: batches.len(),
            status: WriteStatus::Success,
        })
    }
}

/// 批量写入结果
#[derive(Debug, Clone)]
pub struct BatchWriteResult {
    pub total_points: usize,
    pub batch_count: usize,
    pub status: WriteStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteStatus {
    Success,
    PartialSuccess,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_influxdb_converter() {
        let config = TimeSeriesConfig {
            timestamp_field: "time".to_string(),
            metric_fields: vec!["temperature".to_string(), "humidity".to_string()],
            tag_fields: vec!["location".to_string(), "sensor_id".to_string()],
            ..Default::default()
        };

        let converter = InfluxDbConverter::new(config);

        let mut data = HashMap::new();
        data.insert("time".to_string(), Value::Number(1640000000000000000i64.into()));
        data.insert("temperature".to_string(), Value::Number(serde_json::Number::from_f64(25.5).unwrap()));
        data.insert("humidity".to_string(), Value::Number(serde_json::Number::from_f64(60.0).unwrap()));
        data.insert("location".to_string(), Value::String("Beijing".to_string()));
        data.insert("sensor_id".to_string(), Value::String("sensor_001".to_string()));

        let point = converter.convert(&data, &[]).unwrap();

        assert_eq!(point.timestamp, 1640000000000000000i64);
        assert!(point.tags.contains_key("location"));
        assert!(point.fields.contains_key("temperature"));

        let line_protocol = converter.point_to_line_protocol(&point);
        println!("Line Protocol: {}", line_protocol);
    }

    #[test]
    fn test_timescale_hypertable_sql() {
        let config = TimeSeriesConfig {
            database: "metrics".to_string(),
            measurement: "sensor_data".to_string(),
            timestamp_field: "timestamp".to_string(),
            partition_strategy: PartitionStrategy::ByTime { interval: TimeInterval::Hours(1) },
            ..Default::default()
        };

        let converter = TimescaleDbConverter::new(config);
        let sql = converter.generate_hypertable_sql();

        assert!(sql.contains("create_hypertable"));
        assert!(sql.contains("sensor_data"));

        println!("Hypertable SQL: {}", sql);
    }

    #[test]
    fn test_taodb_converter() {
        let config = TimeSeriesConfig {
            timestamp_field: "ts".to_string(),
            metric_fields: vec!["cpu_usage".to_string()],
            tag_fields: vec!["host".to_string()],
            ..Default::default()
        };

        let converter = TaodbConverter::new(config);

        let mut data = HashMap::new();
        data.insert("ts".to_string(), Value::Number(1640000000000i64.into()));
        data.insert("cpu_usage".to_string(), Value::Number(serde_json::Number::from_f64(85.5).unwrap()));
        data.insert("host".to_string(), Value::String("server01".to_string()));

        let record = converter.convert_to_taodb(&data, &[]).unwrap();

        assert_eq!(record.timestamp, 1640000000000i64);
        assert!(record.metrics.contains_key("cpu_usage"));

        let sql = converter.generate_insert_sql(&record);
        println!("Taodb SQL: {}", sql);
    }
}
