use serde::{Deserialize, Serialize};

/// 字段结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Field {
    pub name: String,
    pub data_type: String,
    pub length: Option<usize>,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub primary_key: bool,
    pub auto_increment: bool,
}

/// 表结构
#[derive(Debug, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub fields: Vec<Field>,
    pub indexes: Vec<Index>,
    pub foreign_keys: Vec<ForeignKey>,
}

/// 索引结构
#[derive(Debug, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    pub fields: Vec<String>,
    pub unique: bool,
}

/// 外键结构
#[derive(Debug, Serialize, Deserialize)]
pub struct ForeignKey {
    pub name: String,
    pub fields: Vec<String>,
    pub reference_table: String,
    pub reference_fields: Vec<String>,
}

/// 字段映射
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FieldMapping {
    pub source_table: String,
    pub source_field: String,
    pub target_table: String,
    pub target_field: String,
}

/// 表映射
#[derive(Debug, Serialize, Deserialize)]
pub struct TableMapping {
    pub source_table: String,
    pub target_table: String,
    pub field_mappings: Vec<FieldMapping>,
}
