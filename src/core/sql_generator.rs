/// SQL 生成器模块 - 将字段映射转换为 SQL 语句

use crate::models::{FieldMapping, TableSchema};
use anyhow::Result;

/// SQL 生成器配置
#[derive(Debug, Clone)]
pub struct SqlGeneratorConfig {
    /// 是否使用参数化查询
    pub use_parameters: bool,
    /// 参数占位符风格 (?, $1, :name)
    pub placeholder_style: PlaceholderStyle,
    /// 是否生成批量插入语句
    pub batch_insert: bool,
    /// 批量插入的大小
    pub batch_size: usize,
}

impl Default for SqlGeneratorConfig {
    fn default() -> Self {
        Self {
            use_parameters: true,
            placeholder_style: PlaceholderStyle::QuestionMark,
            batch_insert: false,
            batch_size: 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderStyle {
    QuestionMark,      // ? (SQLite, MySQL)
    DollarNumber,      // $1, $2 (PostgreSQL)
    AtName,           // @name (SQL Server)
    ColonName,        // :name (Oracle)
}

impl PlaceholderStyle {
    pub fn for_database(db_type: &str) -> Self {
        match db_type.to_lowercase().as_str() {
            "postgresql" | "postgres" => PlaceholderStyle::DollarNumber,
            "mysql" | "mariadb" => PlaceholderStyle::QuestionMark,
            "sqlite" => PlaceholderStyle::QuestionMark,
            "sqlserver" | "mssql" => PlaceholderStyle::AtName,
            "oracle" => PlaceholderStyle::ColonName,
            _ => PlaceholderStyle::QuestionMark,
        }
    }
}

/// SQL 语句类型
#[derive(Debug, Clone)]
pub enum SqlStatement {
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    CreateTable(CreateTableStatement),
    DropTable(DropTableStatement),
    AlterTable(AlterTableStatement),
}

#[derive(Debug, Clone)]
pub struct SelectStatement {
    pub table: String,
    pub fields: Vec<String>,
    pub where_clause: Option<String>,
    pub order_by: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct InsertStatement {
    pub table: String,
    pub fields: Vec<String>,
    pub values: Vec<Vec<serde_json::Value>>,
    pub returning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateStatement {
    pub table: String,
    pub set_clauses: Vec<SetClause>,
    pub where_clause: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SetClause {
    pub field: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct DeleteStatement {
    pub table: String,
    pub where_clause: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateTableStatement {
    pub table: String,
    pub fields: Vec<FieldDefinition>,
    pub primary_key: Option<Vec<String>>,
    pub foreign_keys: Vec<ForeignKeyDefinition>,
}

#[derive(Debug, Clone)]
pub struct FieldDefinition {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub auto_increment: bool,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyDefinition {
    pub fields: Vec<String>,
    pub reference_table: String,
    pub reference_fields: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DropTableStatement {
    pub table: String,
    pub if_exists: bool,
    pub cascade: bool,
}

#[derive(Debug, Clone)]
pub struct AlterTableStatement {
    pub table: String,
    pub actions: Vec<AlterAction>,
}

#[derive(Debug, Clone)]
pub enum AlterAction {
    AddColumn(FieldDefinition),
    DropColumn(String),
    ModifyColumn(FieldDefinition),
    RenameColumn { old: String, new: String },
}

/// SQL 生成器
pub struct SqlGenerator {
    config: SqlGeneratorConfig,
}

impl SqlGenerator {
    pub fn new(config: SqlGeneratorConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(SqlGeneratorConfig::default())
    }

    /// 根据字段映射生成 INSERT 语句
    pub fn generate_insert(&self, mapping: &FieldMapping, _values: &[serde_json::Value]) -> Result<String> {
        let _placeholder = self.get_placeholder(0);
        let field_count = mapping.target_field.split(',').count();
        
        let fields = mapping.target_field.split(',').map(|s| s.trim()).collect::<Vec<_>>().join(", ");
        let placeholders = (0..field_count)
            .map(|i| self.get_placeholder(i))
            .collect::<Vec<_>>()
            .join(", ");

        Ok(format!(
            "INSERT INTO {} ({}) VALUES ({})",
            mapping.target_table, fields, placeholders
        ))
    }

    /// 根据字段映射生成批量 INSERT 语句
    pub fn generate_batch_insert(&self, mapping: &FieldMapping, rows: &[Vec<serde_json::Value>]) -> Result<Vec<String>> {
        let mut statements = Vec::new();
        
        for chunk in rows.chunks(self.config.batch_size) {
            let field_count = mapping.target_field.split(',').count();
            let fields = mapping.target_field.split(',').map(|s| s.trim()).collect::<Vec<_>>().join(", ");
            
            let mut values_clauses = Vec::new();
            for (row_idx, _row) in chunk.iter().enumerate() {
                let placeholders = (0..field_count)
                    .map(|i| self.get_placeholder(row_idx * field_count + i))
                    .collect::<Vec<_>>()
                    .join(", ");
                values_clauses.push(format!("({})", placeholders));
            }

            statements.push(format!(
                "INSERT INTO {} ({}) VALUES {}",
                mapping.target_table, fields, values_clauses.join(", ")
            ));
        }

        Ok(statements)
    }

    /// 根据字段映射生成 UPDATE 语句
    pub fn generate_update(&self, mapping: &FieldMapping, _values: &[serde_json::Value], where_clause: &str) -> Result<String> {
        let target_fields: Vec<&str> = mapping.target_field.split(',').map(|s| s.trim()).collect();
        let mut set_clauses = Vec::new();
        
        for (i, field) in target_fields.iter().enumerate() {
            let placeholder = self.get_placeholder(i);
            set_clauses.push(format!("{} = {}", field, placeholder));
        }

        Ok(format!(
            "UPDATE {} SET {} WHERE {}",
            mapping.target_table,
            set_clauses.join(", "),
            where_clause
        ))
    }

    /// 根据字段映射生成 SELECT 语句
    pub fn generate_select(&self, mapping: &FieldMapping, where_clause: Option<&str>, limit: Option<usize>) -> Result<String> {
        let fields = mapping.target_field.split(',').map(|s| s.trim()).collect::<Vec<_>>().join(", ");
        
        let mut sql = format!("SELECT {} FROM {}", fields, mapping.target_table);
        
        if let Some(where_clause) = where_clause {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }
        
        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        Ok(sql)
    }

    /// 根据字段映射生成 DELETE 语句
    pub fn generate_delete(&self, mapping: &FieldMapping, where_clause: Option<&str>) -> Result<String> {
        let mut sql = format!("DELETE FROM {}", mapping.target_table);
        
        if let Some(where_clause) = where_clause {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }

        Ok(sql)
    }

    /// 根据表结构生成 CREATE TABLE 语句
    pub fn generate_create_table(&self, schema: &TableSchema) -> Result<String> {
        let mut field_defs = Vec::new();
        
        for field in &schema.fields {
            let mut def = format!("{} {}", field.name, self.convert_data_type(&field.data_type));
            
            if !field.nullable {
                def.push_str(" NOT NULL");
            }
            
            if let Some(ref default) = field.default_value {
                def.push_str(&format!(" DEFAULT {}", default));
            }
            
            if field.auto_increment {
                def.push_str(" AUTOINCREMENT");
            }
            
            field_defs.push(def);
        }
        
        // 添加主键
        let primary_keys: Vec<&str> = schema.fields.iter()
            .filter(|f| f.primary_key)
            .map(|f| f.name.as_str())
            .collect();
        
        if !primary_keys.is_empty() {
            field_defs.push(format!("PRIMARY KEY ({})", primary_keys.join(", ")));
        }
        
        // 添加外键
        for fk in &schema.foreign_keys {
            field_defs.push(format!(
                "FOREIGN KEY ({}) REFERENCES {}({})",
                fk.fields.join(", "),
                fk.reference_table,
                fk.reference_fields.join(", ")
            ));
        }

        Ok(format!(
            "CREATE TABLE {} ({})",
            schema.name,
            field_defs.join(", ")
        ))
    }

    /// 将多个字段映射转换为单个 INSERT 语句
    pub fn generate_insert_from_mappings(
        &self,
        mappings: &[FieldMapping],
        source_table: &str,
        target_table: &str,
    ) -> Result<String> {
        let _field_parts: Vec<&str> = Vec::new();
        let _value_parts: Vec<String> = Vec::new();
        
        // 按源表分组
        let mut table_fields: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
        for mapping in mappings {
            table_fields
                .entry(&mapping.source_table)
                .or_insert_with(Vec::new)
                .push(&mapping.source_field);
        }
        
        // 生成 SELECT 子句
        let mut select_parts = Vec::new();
        for (table, fields) in &table_fields {
            for field in fields {
                select_parts.push(format!("{}.{}", table, field));
            }
        }
        
        let select_clause = select_parts.join(", ");
        
        // 生成目标字段列表
        let target_fields = mappings
            .iter()
            .map(|m| m.target_field.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        
        // 生成占位符
        let _placeholders = mappings
            .iter()
            .enumerate()
            .map(|(i, _)| self.get_placeholder(i))
            .collect::<Vec<_>>()
            .join(", ");

        Ok(format!(
            "INSERT INTO {} ({}) SELECT {} FROM {} WHERE {}",
            target_table,
            target_fields,
            select_clause,
            source_table,
            "1=1" // 占位条件
        ))
    }

    /// 获取参数占位符
    fn get_placeholder(&self, index: usize) -> String {
        match self.config.placeholder_style {
            PlaceholderStyle::QuestionMark => "?".to_string(),
            PlaceholderStyle::DollarNumber => format!("${}", index + 1),
            PlaceholderStyle::AtName => format!("@p{}", index + 1),
            PlaceholderStyle::ColonName => format!(":{}", index + 1),
        }
    }

    /// 转换数据类型到目标数据库
    fn convert_data_type(&self, data_type: &str) -> String {
        // 这里可以实现数据类型转换逻辑
        data_type.to_string()
    }

    /// 根据字段映射生成完整的转移 SQL 脚本
    pub fn generate_transfer_script(
        &self,
        mappings: &[FieldMapping],
        _source_db_type: &str,
        _target_db_type: &str,
    ) -> Result<Vec<String>> {
        let mut statements = Vec::new();
        
        // 为每个源表生成 SELECT 和 INSERT 语句
        let mut table_mappings: std::collections::HashMap<String, Vec<&FieldMapping>> = std::collections::HashMap::new();
        for mapping in mappings {
            table_mappings
                .entry(mapping.source_table.clone())
                .or_insert_with(Vec::new)
                .push(mapping);
        }
        
        for (source_table, table_mappings) in &table_mappings {
            let target_table = table_mappings.first().map(|m| m.target_table.as_str()).unwrap_or(source_table);
            
            // 生成 SELECT 语句
            let select_fields = table_mappings
                .iter()
                .map(|m| m.source_field.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            
            statements.push(format!("SELECT {} FROM {}", select_fields, source_table));
            
            // 生成 INSERT 语句
            let insert_fields = table_mappings
                .iter()
                .map(|m| m.target_field.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            
            let placeholders = table_mappings
                .iter()
                .enumerate()
                .map(|(i, _)| self.get_placeholder(i))
                .collect::<Vec<_>>()
                .join(", ");
            
            statements.push(format!(
                "INSERT INTO {} ({}) VALUES ({})",
                target_table, insert_fields, placeholders
            ));
        }
        
        Ok(statements)
    }
}

/// 字段连线映射器 - 处理可视化连线的数据转换
pub struct FieldConnectionMapper {
    generator: SqlGenerator,
}

impl FieldConnectionMapper {
    pub fn new() -> Self {
        Self {
            generator: SqlGenerator::with_defaults(),
        }
    }

    /// 根据连线关系生成 INSERT 语句
    pub fn map_connection_to_insert(
        &self,
        source_field: &str,
        target_field: &str,
        value: &serde_json::Value,
    ) -> Result<String> {
        let mapping = FieldMapping {
            source_table: "source".to_string(),
            source_field: source_field.to_string(),
            target_table: "target".to_string(),
            target_field: target_field.to_string(),
        };
        
        self.generator.generate_insert(&mapping, &[value.clone()])
    }

    /// 根据连线关系生成 UPDATE 语句
    pub fn map_connection_to_update(
        &self,
        source_field: &str,
        target_field: &str,
        value: &serde_json::Value,
        where_field: &str,
        where_value: &serde_json::Value,
    ) -> Result<String> {
        let mapping = FieldMapping {
            source_table: "source".to_string(),
            source_field: source_field.to_string(),
            target_table: "target".to_string(),
            target_field: target_field.to_string(),
        };
        
        let where_clause = format!("{} = {}", where_field, where_value);
        self.generator.generate_update(&mapping, &[value.clone()], &where_clause)
    }

    /// 根据连线关系批量生成 INSERT 语句
    pub fn map_connections_to_batch_insert(
        &self,
        connections: &[(String, String, serde_json::Value)],
        target_table: &str,
    ) -> Result<String> {
        if connections.is_empty() {
            return Ok(String::new());
        }
        
        let target_fields = connections
            .iter()
            .map(|(_, target_field, _)| target_field.clone())
            .collect::<Vec<_>>()
            .join(", ");
        
        let placeholders = connections
            .iter()
            .enumerate()
            .map(|(i, _)| self.generator.get_placeholder(i))
            .collect::<Vec<_>>()
            .join(", ");
        
        Ok(format!(
            "INSERT INTO {} ({}) VALUES ({})",
            target_table, target_fields, placeholders
        ))
    }

    /// 将字段连线转换为 SQL SET 子句
    pub fn map_connection_to_set_clause(
        &self,
        _source_field: &str,
        target_field: &str,
        value: &serde_json::Value,
    ) -> String {
        format!("{} = {}", target_field, value)
    }

    /// 生成完整的字段连线 SQL 脚本
    pub fn generate_connection_script(
        &self,
        connections: &[(String, String, serde_json::Value)], // (source_field, target_field, value)
        target_table: &str,
        operation: &str,
    ) -> Result<String> {
        match operation.to_uppercase().as_str() {
            "INSERT" => self.map_connections_to_batch_insert(connections, target_table),
            "UPDATE" => {
                if connections.is_empty() {
                    return Ok(String::new());
                }
                
                let set_clauses = connections
                    .iter()
                    .map(|(_source, target, value)| {
                        format!("{} = {}", target, value)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                
                Ok(format!("UPDATE {} SET {}", target_table, set_clauses))
            }
            _ => Err(anyhow::anyhow!("Unsupported operation: {}", operation)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_statement_generation() {
        let generator = SqlGenerator::with_defaults();
        
        let mapping = FieldMapping {
            source_table: "users".to_string(),
            source_field: "name".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "name".to_string(),
        };
        
        let sql = generator.generate_insert(&mapping, &[serde_json::json!("Alice")]).unwrap();
        assert!(sql.contains("INSERT INTO users_copy"));
        assert!(sql.contains("name"));
        
        println!("Generated INSERT: {}", sql);
    }

    #[test]
    fn test_select_statement_generation() {
        let generator = SqlGenerator::with_defaults();
        
        let mapping = FieldMapping {
            source_table: "users".to_string(),
            source_field: "id, name, email".to_string(),
            target_table: "users_copy".to_string(),
            target_field: "id, name, email".to_string(),
        };
        
        let sql = generator.generate_select(&mapping, Some("id > 100"), Some(10)).unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM users"));
        assert!(sql.contains("WHERE id > 100"));
        assert!(sql.contains("LIMIT 10"));
        
        println!("Generated SELECT: {}", sql);
    }

    #[test]
    fn test_placeholder_styles() {
        let pg_style = PlaceholderStyle::for_database("postgresql");
        assert_eq!(pg_style, PlaceholderStyle::DollarNumber);
        
        let mysql_style = PlaceholderStyle::for_database("mysql");
        assert_eq!(mysql_style, PlaceholderStyle::QuestionMark);
        
        let sqlite_style = PlaceholderStyle::for_database("sqlite");
        assert_eq!(sqlite_style, PlaceholderStyle::QuestionMark);
        
        println!("Placeholder styles test passed");
    }

    #[test]
    fn test_field_connection_mapper() {
        let mapper = FieldConnectionMapper::new();
        
        let connections = vec![
            ("id".to_string(), "user_id".to_string(), serde_json::json!(1)),
            ("name".to_string(), "user_name".to_string(), serde_json::json!("Alice")),
            ("email".to_string(), "user_email".to_string(), serde_json::json!("alice@example.com")),
        ];
        
        let sql = mapper.generate_connection_script(&connections, "users", "INSERT").unwrap();
        assert!(sql.contains("INSERT INTO users"));
        assert!(sql.contains("user_id, user_name, user_email"));
        
        println!("Generated connection script: {}", sql);
    }
}
