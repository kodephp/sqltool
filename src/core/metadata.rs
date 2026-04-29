use crate::databases::DatabaseConnection;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    pub database_name: String,
    pub db_type: String,
    pub version: String,
    pub tables: Vec<TableMetadata>,
    pub views: Vec<ViewMetadata>,
    pub functions: Vec<FunctionMetadata>,
    pub stored_procedures: Vec<StoredProcedureMetadata>,
    pub triggers: Vec<TriggerMetadata>,
    pub indexes: Vec<IndexMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    pub name: String,
    pub schema: Option<String>,
    pub engine: Option<String>,
    pub row_count: u64,
    pub data_length: u64,
    pub index_length: u64,
    pub auto_increment: Option<u64>,
    pub collation: Option<String>,
    pub fields: Vec<FieldMetadata>,
    pub foreign_keys: Vec<ForeignKeyMetadata>,
    pub indexes: Vec<IndexMetadata>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMetadata {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
    pub default_value: Option<String>,
    pub max_length: Option<u64>,
    pub precision: Option<u32>,
    pub scale: Option<u32>,
    pub character_set: Option<String>,
    pub collation: Option<String>,
    pub extra: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyMetadata {
    pub name: String,
    pub field: String,
    pub referenced_table: String,
    pub referenced_field: String,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewMetadata {
    pub name: String,
    pub schema: Option<String>,
    pub definition: String,
    pub is_updatable: bool,
    pub check_option: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMetadata {
    pub name: String,
    pub schema: Option<String>,
    pub return_type: String,
    pub arguments: Vec<FunctionArgument>,
    pub language: String,
    pub definition: String,
    pub volatility: String,
    pub security_type: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionArgument {
    pub name: String,
    pub data_type: String,
    pub default_value: Option<String>,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProcedureMetadata {
    pub name: String,
    pub schema: Option<String>,
    pub arguments: Vec<FunctionArgument>,
    pub language: String,
    pub definition: String,
    pub sql_mode: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerMetadata {
    pub name: String,
    pub table_name: String,
    pub timing: String,
    pub event: String,
    pub definition: String,
    pub created: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub name: String,
    pub table_name: String,
    pub fields: Vec<String>,
    pub index_type: String,
    pub is_unique: bool,
    pub is_primary: bool,
    pub is_visible: bool,
    pub comment: Option<String>,
}

pub struct MetadataManager {
    connection: Box<dyn DatabaseConnection>,
    db_type: String,
}

impl MetadataManager {
    pub fn new(connection: Box<dyn DatabaseConnection>, db_type: &str) -> Self {
        Self {
            connection,
            db_type: db_type.to_string(),
        }
    }

    pub async fn get_database_metadata(&self) -> Result<DatabaseMetadata> {
        let tables = self.get_tables().await?;
        let views = self.get_views().await?;
        let functions = self.get_functions().await?;
        let stored_procedures = self.get_stored_procedures().await?;
        let triggers = self.get_triggers().await?;

        Ok(DatabaseMetadata {
            database_name: self.get_current_database_name().await?,
            db_type: self.db_type.clone(),
            version: self.get_database_version().await?,
            tables,
            views,
            functions,
            stored_procedures,
            triggers,
            indexes: Vec::new(),
        })
    }

    pub async fn get_tables(&self) -> Result<Vec<TableMetadata>> {
        match self.db_type.as_str() {
            "mysql" => self.get_mysql_tables().await,
            "pgsql" | "postgres" => self.get_postgres_tables().await,
            "sqlite" => self.get_sqlite_tables().await,
            _ => Err(anyhow!("Unsupported database type: {}", self.db_type)),
        }
    }

    async fn get_mysql_tables(&self) -> Result<Vec<TableMetadata>> {
        let sql = r#"
            SELECT
                TABLE_NAME,
                TABLE_SCHEMA,
                ENGINE,
                TABLE_ROWS,
                DATA_LENGTH,
                INDEX_LENGTH,
                AUTO_INCREMENT,
                TABLE_COLLATION,
                TABLE_COMMENT
            FROM information_schema.TABLES
            WHERE TABLE_TYPE = 'BASE TABLE'
            ORDER BY TABLE_NAME
        "#;

        let rows = self.connection.query(sql).await?;
        let mut tables = Vec::new();

        for row in rows {
            let name = self.get_string_value(&row, "TABLE_NAME")?;
            let name_clone = name.clone();
            let _table_sql = format!("SELECT * FROM {} LIMIT 0", name);
            let fields = self.get_mysql_table_fields(&name_clone).await?;

            tables.push(TableMetadata {
                name: name_clone.clone(),
                schema: self.get_string_value(&row, "TABLE_SCHEMA").ok(),
                engine: self.get_string_value(&row, "ENGINE").ok(),
                row_count: self.get_u64_value(&row, "TABLE_ROWS").unwrap_or(0),
                data_length: self.get_u64_value(&row, "DATA_LENGTH").unwrap_or(0),
                index_length: self.get_u64_value(&row, "INDEX_LENGTH").unwrap_or(0),
                auto_increment: self.get_u64_value(&row, "AUTO_INCREMENT").ok(),
                collation: self.get_string_value(&row, "TABLE_COLLATION").ok(),
                fields,
                foreign_keys: self.get_foreign_keys_internal("BASE TABLE", &name_clone).await.unwrap_or_default(),
                indexes: self.get_table_indexes_internal(&name_clone).await.unwrap_or_default(),
                comment: self.get_string_value(&row, "TABLE_COMMENT").ok(),
            });
        }

        Ok(tables)
    }

    async fn get_mysql_table_fields(&self, table_name: &str) -> Result<Vec<FieldMetadata>> {
        let sql = format!(
            r#"
            SELECT
                COLUMN_NAME,
                DATA_TYPE,
                COLUMN_TYPE,
                IS_NULLABLE,
                COLUMN_KEY,
                COLUMN_DEFAULT,
                CHARACTER_MAXIMUM_LENGTH,
                NUMERIC_PRECISION,
                NUMERIC_SCALE,
                CHARACTER_SET_NAME,
                COLLATION_NAME,
                EXTRA,
                COLUMN_COMMENT
            FROM information_schema.COLUMNS
            WHERE TABLE_NAME = '{}'
            ORDER BY ORDINAL_POSITION
            "#,
            table_name
        );

        let rows = self.connection.query(&sql).await?;
        let mut fields = Vec::new();

        for row in rows {
            let col_key = self.get_string_value(&row, "COLUMN_KEY").unwrap_or_default();
            let extra = self.get_string_value(&row, "EXTRA").unwrap_or_default();

            fields.push(FieldMetadata {
                name: self.get_string_value(&row, "COLUMN_NAME")?,
                data_type: self.get_string_value(&row, "DATA_TYPE")?,
                is_nullable: self.get_string_value(&row, "IS_NULLABLE").unwrap_or_default() == "YES",
                is_primary_key: col_key == "PRI",
                is_foreign_key: col_key == "MUL",
                default_value: self.get_string_value(&row, "COLUMN_DEFAULT").ok(),
                max_length: self.get_u64_value(&row, "CHARACTER_MAXIMUM_LENGTH").ok(),
                precision: self.get_u32_value(&row, "NUMERIC_PRECISION").ok(),
                scale: self.get_u32_value(&row, "NUMERIC_SCALE").ok(),
                character_set: self.get_string_value(&row, "CHARACTER_SET_NAME").ok(),
                collation: self.get_string_value(&row, "COLLATION_NAME").ok(),
                extra: if extra.is_empty() { None } else { Some(extra) },
                comment: self.get_string_value(&row, "COLUMN_COMMENT").ok(),
            });
        }

        Ok(fields)
    }

    async fn get_postgres_tables(&self) -> Result<Vec<TableMetadata>> {
        let sql = r#"
            SELECT
                t.table_name,
                t.table_schema,
                c.reltuples::bigint as row_count,
                pg_size_pretty(pg_total_relation_size(c.oid)) as total_size
            FROM information_schema.tables t
            JOIN pg_class c ON c.relname = t.table_name
            JOIN pg_namespace n ON n.nspname = t.table_schema
            WHERE t.table_type = 'BASE TABLE'
            AND t.table_schema NOT IN ('pg_catalog', 'information_schema')
            ORDER BY t.table_name
        "#;

        let rows = self.connection.query(sql).await?;
        let mut tables = Vec::new();

        for row in rows {
            let name = self.get_string_value(&row, "table_name")?;
            let name_clone = name.clone();
            let fields = self.get_postgres_table_fields(&name_clone).await?;

            tables.push(TableMetadata {
                name: name_clone.clone(),
                schema: self.get_string_value(&row, "table_schema").ok(),
                engine: None,
                row_count: self.get_u64_value(&row, "row_count").unwrap_or(0),
                data_length: 0,
                index_length: 0,
                auto_increment: None,
                collation: None,
                fields,
                foreign_keys: self.get_foreign_keys_internal("BASE TABLE", &name_clone).await.unwrap_or_default(),
                indexes: self.get_table_indexes_internal(&name_clone).await.unwrap_or_default(),
                comment: None,
            });
        }

        Ok(tables)
    }

    async fn get_postgres_table_fields(&self, table_name: &str) -> Result<Vec<FieldMetadata>> {
        let sql = format!(
            r#"
            SELECT
                c.column_name,
                c.data_type,
                c.is_nullable,
                c.column_default,
                c.character_maximum_length,
                c.numeric_precision,
                c.numeric_scale,
                c.collation_name,
                CASE WHEN pk.column_name IS NOT NULL THEN 'PRI' ELSE '' END as column_key
            FROM information_schema.columns c
            LEFT JOIN (
                SELECT ku.column_name
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage ku ON tc.constraint_name = ku.constraint_name
                WHERE tc.table_name = '{}' AND tc.constraint_type = 'PRIMARY KEY'
            ) pk ON c.column_name = pk.column_name
            WHERE c.table_name = '{}'
            ORDER BY c.ordinal_position
            "#,
            table_name, table_name
        );

        let rows = self.connection.query(&sql).await?;
        let mut fields = Vec::new();

        for row in rows {
            let col_key = self.get_string_value(&row, "column_key").unwrap_or_default();

            fields.push(FieldMetadata {
                name: self.get_string_value(&row, "column_name")?,
                data_type: self.get_string_value(&row, "data_type")?,
                is_nullable: self.get_string_value(&row, "is_nullable").unwrap_or_default() == "YES",
                is_primary_key: col_key == "PRI",
                is_foreign_key: false,
                default_value: self.get_string_value(&row, "column_default").ok(),
                max_length: self.get_u64_value(&row, "character_maximum_length").ok(),
                precision: self.get_u32_value(&row, "numeric_precision").ok(),
                scale: self.get_u32_value(&row, "numeric_scale").ok(),
                character_set: None,
                collation: self.get_string_value(&row, "collation_name").ok(),
                extra: None,
                comment: None,
            });
        }

        Ok(fields)
    }

    async fn get_sqlite_tables(&self) -> Result<Vec<TableMetadata>> {
        let sql = "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name";
        let rows = self.connection.query(sql).await?;
        let mut tables = Vec::new();

        for row in rows {
            let name = self.get_string_value(&row, "name")?;
            let name_clone = name.clone();
            let fields = self.get_sqlite_table_fields(&name_clone).await?;

            tables.push(TableMetadata {
                name: name_clone.clone(),
                schema: None,
                engine: Some("SQLite".to_string()),
                row_count: 0,
                data_length: 0,
                index_length: 0,
                auto_increment: None,
                collation: None,
                fields,
                foreign_keys: Vec::new(),
                indexes: self.get_table_indexes_internal(&name_clone).await.unwrap_or_default(),
                comment: None,
            });
        }

        Ok(tables)
    }

    async fn get_sqlite_table_fields(&self, table_name: &str) -> Result<Vec<FieldMetadata>> {
        let sql = format!("PRAGMA table_info({})", table_name);
        let rows = self.connection.query(&sql).await?;
        let mut fields = Vec::new();

        for row in rows {
            fields.push(FieldMetadata {
                name: self.get_string_value(&row, "name")?,
                data_type: self.get_string_value(&row, "type")?,
                is_nullable: self.get_i64_value(&row, "notnull").unwrap_or(1) == 0,
                is_primary_key: self.get_i64_value(&row, "pk").unwrap_or(0) > 0,
                is_foreign_key: false,
                default_value: self.get_string_value(&row, "dflt_value").ok(),
                max_length: None,
                precision: None,
                scale: None,
                character_set: None,
                collation: None,
                extra: None,
                comment: None,
            });
        }

        Ok(fields)
    }

    pub async fn get_views(&self) -> Result<Vec<ViewMetadata>> {
        match self.db_type.as_str() {
            "mysql" => self.get_mysql_views().await,
            "pgsql" | "postgres" => self.get_postgres_views().await,
            "sqlite" => self.get_sqlite_views().await,
            _ => Ok(Vec::new()),
        }
    }

    async fn get_mysql_views(&self) -> Result<Vec<ViewMetadata>> {
        let sql = r#"
            SELECT
                TABLE_NAME as view_name,
                VIEW_DEFINITION,
                CHECK_OPTION,
                IS_UPDATABLE,
                TABLE_COMMENT
            FROM information_schema.VIEWS
            WHERE TABLE_SCHEMA = DATABASE()
            ORDER BY TABLE_NAME
        "#;

        let rows = self.connection.query(sql).await?;
        let mut views = Vec::new();

        for row in rows {
            views.push(ViewMetadata {
                name: self.get_string_value(&row, "view_name")?,
                schema: None,
                definition: self.get_string_value(&row, "VIEW_DEFINITION")?,
                is_updatable: self.get_string_value(&row, "IS_UPDATABLE").unwrap_or_default() == "YES",
                check_option: self.get_string_value(&row, "CHECK_OPTION").ok(),
                comment: self.get_string_value(&row, "TABLE_COMMENT").ok(),
            });
        }

        Ok(views)
    }

    async fn get_postgres_views(&self) -> Result<Vec<ViewMetadata>> {
        let sql = r#"
            SELECT
                v.schemaname,
                v.viewname,
                pg_get_viewdef(v.viewname) as definition
            FROM pg_views v
            WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
            ORDER BY v.viewname
        "#;

        let rows = self.connection.query(sql).await?;
        let mut views = Vec::new();

        for row in rows {
            views.push(ViewMetadata {
                name: self.get_string_value(&row, "viewname")?,
                schema: self.get_string_value(&row, "schemaname").ok(),
                definition: self.get_string_value(&row, "definition")?,
                is_updatable: true,
                check_option: None,
                comment: None,
            });
        }

        Ok(views)
    }

    async fn get_sqlite_views(&self) -> Result<Vec<ViewMetadata>> {
        let sql = "SELECT name, sql FROM sqlite_master WHERE type='view' ORDER BY name";
        let rows = self.connection.query(sql).await?;
        let mut views = Vec::new();

        for row in rows {
            views.push(ViewMetadata {
                name: self.get_string_value(&row, "name")?,
                schema: None,
                definition: self.get_string_value(&row, "sql").unwrap_or_default(),
                is_updatable: false,
                check_option: None,
                comment: None,
            });
        }

        Ok(views)
    }

    pub async fn get_functions(&self) -> Result<Vec<FunctionMetadata>> {
        match self.db_type.as_str() {
            "mysql" => self.get_mysql_functions().await,
            "pgsql" | "postgres" => self.get_postgres_functions().await,
            _ => Ok(Vec::new()),
        }
    }

    async fn get_mysql_functions(&self) -> Result<Vec<FunctionMetadata>> {
        let sql = r#"
            SELECT
                ROUTINE_NAME,
                DATA_TYPE as return_type,
                ROUTINE_DEFINITION,
                SQL_DATA_ACCESS,
                SECURITY_TYPE,
                ROUTINE_COMMENT
            FROM information_schema.ROUTINES
            WHERE ROUTINE_TYPE = 'FUNCTION'
            AND ROUTINE_SCHEMA = DATABASE()
            ORDER BY ROUTINE_NAME
        "#;

        let rows = self.connection.query(sql).await?;
        let mut functions = Vec::new();

        for row in rows {
            functions.push(FunctionMetadata {
                name: self.get_string_value(&row, "ROUTINE_NAME")?,
                schema: None,
                return_type: self.get_string_value(&row, "return_type")?,
                arguments: Vec::new(),
                language: "SQL".to_string(),
                definition: self.get_string_value(&row, "ROUTINE_DEFINITION").unwrap_or_default(),
                volatility: self.get_string_value(&row, "SQL_DATA_ACCESS").unwrap_or_default(),
                security_type: self.get_string_value(&row, "SECURITY_TYPE").unwrap_or_default(),
                comment: self.get_string_value(&row, "ROUTINE_COMMENT").ok(),
            });
        }

        Ok(functions)
    }

    async fn get_postgres_functions(&self) -> Result<Vec<FunctionMetadata>> {
        let sql = r#"
            SELECT
                p.proname as name,
                n.nspname as schema,
                pg_get_function_result(p.oid) as return_type,
                pg_get_function_arguments(p.oid) as arguments,
                p.prosrc as definition,
                p.provolatile as volatility,
                p.secdef as security_type
            FROM pg_proc p
            JOIN pg_namespace n ON p.pronamespace = n.oid
            WHERE p.prokind = 'f'
            AND n.nspname NOT IN ('pg_catalog', 'information_schema')
            ORDER BY p.proname
        "#;

        let rows = self.connection.query(sql).await?;
        let mut functions = Vec::new();

        for row in rows {
            let args_str = self.get_string_value(&row, "arguments").unwrap_or_default();
            let args: Vec<FunctionArgument> = args_str
                .split(',')
                .filter_map(|arg| {
                    let parts: Vec<&str> = arg.trim().split_whitespace().collect();
                    if parts.is_empty() {
                        None
                    } else {
                        Some(FunctionArgument {
                            name: parts.get(1).unwrap_or(&parts[0]).to_string(),
                            data_type: parts.get(0).unwrap_or(&"").to_string(),
                            default_value: None,
                            mode: "IN".to_string(),
                        })
                    }
                })
                .collect();

            functions.push(FunctionMetadata {
                name: self.get_string_value(&row, "name")?,
                schema: self.get_string_value(&row, "schema").ok(),
                return_type: self.get_string_value(&row, "return_type")?,
                arguments: args,
                language: "SQL".to_string(),
                definition: self.get_string_value(&row, "definition").unwrap_or_default(),
                volatility: self.get_string_value(&row, "volatility").unwrap_or_default(),
                security_type: if self.get_i64_value(&row, "security_type").unwrap_or(0) == 1 {
                    "DEFINER".to_string()
                } else {
                    "INVOKER".to_string()
                },
                comment: None,
            });
        }

        Ok(functions)
    }

    pub async fn get_stored_procedures(&self) -> Result<Vec<StoredProcedureMetadata>> {
        match self.db_type.as_str() {
            "mysql" => self.get_mysql_stored_procedures().await,
            "pgsql" | "postgres" => self.get_postgres_stored_procedures().await,
            _ => Ok(Vec::new()),
        }
    }

    async fn get_mysql_stored_procedures(&self) -> Result<Vec<StoredProcedureMetadata>> {
        let sql = r#"
            SELECT
                ROUTINE_NAME,
                ROUTINE_DEFINITION,
                SQL_DATA_ACCESS,
                SQL_MODE,
                ROUTINE_COMMENT
            FROM information_schema.ROUTINES
            WHERE ROUTINE_TYPE = 'PROCEDURE'
            AND ROUTINE_SCHEMA = DATABASE()
            ORDER BY ROUTINE_NAME
        "#;

        let rows = self.connection.query(sql).await?;
        let mut procedures = Vec::new();

        for row in rows {
            procedures.push(StoredProcedureMetadata {
                name: self.get_string_value(&row, "ROUTINE_NAME")?,
                schema: None,
                arguments: Vec::new(),
                language: "SQL".to_string(),
                definition: self.get_string_value(&row, "ROUTINE_DEFINITION").unwrap_or_default(),
                sql_mode: self.get_string_value(&row, "SQL_MODE").unwrap_or_default(),
                comment: self.get_string_value(&row, "ROUTINE_COMMENT").ok(),
            });
        }

        Ok(procedures)
    }

    async fn get_postgres_stored_procedures(&self) -> Result<Vec<StoredProcedureMetadata>> {
        let sql = r#"
            SELECT
                p.proname as name,
                n.nspname as schema,
                pg_get_function_arguments(p.oid) as arguments,
                p.prosrc as definition
            FROM pg_proc p
            JOIN pg_namespace n ON p.pronamespace = n.oid
            WHERE p.prokind = 'p'
            AND n.nspname NOT IN ('pg_catalog', 'information_schema')
            ORDER BY p.proname
        "#;

        let rows = self.connection.query(sql).await?;
        let mut procedures = Vec::new();

        for row in rows {
            procedures.push(StoredProcedureMetadata {
                name: self.get_string_value(&row, "name")?,
                schema: self.get_string_value(&row, "schema").ok(),
                arguments: Vec::new(),
                language: "SQL".to_string(),
                definition: self.get_string_value(&row, "definition").unwrap_or_default(),
                sql_mode: String::new(),
                comment: None,
            });
        }

        Ok(procedures)
    }

    pub async fn get_triggers(&self) -> Result<Vec<TriggerMetadata>> {
        match self.db_type.as_str() {
            "mysql" => self.get_mysql_triggers().await,
            "pgsql" | "postgres" => self.get_postgres_triggers().await,
            "sqlite" => self.get_sqlite_triggers().await,
            _ => Ok(Vec::new()),
        }
    }

    async fn get_mysql_triggers(&self) -> Result<Vec<TriggerMetadata>> {
        let sql = r#"
            SELECT
                TRIGGER_NAME,
                EVENT_OBJECT_TABLE as table_name,
                ACTION_TIMING as timing,
                EVENT_MANIPULATION as event,
                ACTION_STATEMENT as definition,
                CREATED
            FROM information_schema.TRIGGERS
            WHERE TRIGGER_SCHEMA = DATABASE()
            ORDER BY TRIGGER_NAME
        "#;

        let rows = self.connection.query(sql).await?;
        let mut triggers = Vec::new();

        for row in rows {
            triggers.push(TriggerMetadata {
                name: self.get_string_value(&row, "TRIGGER_NAME")?,
                table_name: self.get_string_value(&row, "table_name")?,
                timing: self.get_string_value(&row, "timing")?,
                event: self.get_string_value(&row, "event")?,
                definition: self.get_string_value(&row, "definition")?,
                created: self.get_string_value(&row, "CREATED").ok(),
                comment: None,
            });
        }

        Ok(triggers)
    }

    async fn get_postgres_triggers(&self) -> Result<Vec<TriggerMetadata>> {
        let sql = r#"
            SELECT
                t.tgname as name,
                c.relname as table_name,
                CASE WHEN t.tgtype & 1 = 1 THEN 'AFTER' ELSE 'BEFORE' END as timing,
                CASE
                    WHEN t.tgtype & 2 = 2 THEN 'INSERT'
                    WHEN t.tgtype & 4 = 4 THEN 'DELETE'
                    WHEN t.tgtype & 8 = 8 THEN 'UPDATE'
                    ELSE 'UNKNOWN'
                END as event,
                pg_get_triggerdef(t.oid) as definition
            FROM pg_trigger t
            JOIN pg_class c ON t.tgrelid = c.oid
            JOIN pg_namespace n ON c.relnamespace = n.oid
            WHERE NOT t.tgisinternal
            AND n.nspname NOT IN ('pg_catalog', 'information_schema')
            ORDER BY t.tgname
        "#;

        let rows = self.connection.query(sql).await?;
        let mut triggers = Vec::new();

        for row in rows {
            triggers.push(TriggerMetadata {
                name: self.get_string_value(&row, "name")?,
                table_name: self.get_string_value(&row, "table_name")?,
                timing: self.get_string_value(&row, "timing")?,
                event: self.get_string_value(&row, "event")?,
                definition: self.get_string_value(&row, "definition").unwrap_or_default(),
                created: None,
                comment: None,
            });
        }

        Ok(triggers)
    }

    async fn get_sqlite_triggers(&self) -> Result<Vec<TriggerMetadata>> {
        let sql = "SELECT name, sql FROM sqlite_master WHERE type='trigger' ORDER BY name";
        let rows = self.connection.query(sql).await?;
        let mut triggers = Vec::new();

        for row in rows {
            let sql_def = self.get_string_value(&row, "sql").unwrap_or_default();
            triggers.push(TriggerMetadata {
                name: self.get_string_value(&row, "name")?,
                table_name: String::new(),
                timing: if sql_def.contains("BEFORE") { "BEFORE".to_string() } else { "AFTER".to_string() },
                event: if sql_def.contains("INSERT") { "INSERT".to_string() }
                    else if sql_def.contains("UPDATE") { "UPDATE".to_string() }
                    else if sql_def.contains("DELETE") { "DELETE".to_string() }
                    else { "UNKNOWN".to_string() },
                definition: sql_def,
                created: None,
                comment: None,
            });
        }

        Ok(triggers)
    }

    async fn get_foreign_keys_internal(&self, _table_type: &str, _table_name: &str) -> Result<Vec<ForeignKeyMetadata>> {
        Ok(Vec::new())
    }

    async fn get_table_indexes_internal(&self, _table_name: &str) -> Result<Vec<IndexMetadata>> {
        Ok(Vec::new())
    }

    pub async fn get_current_database_name(&self) -> Result<String> {
        let sql = match self.db_type.as_str() {
            "mysql" => "SELECT DATABASE() as db_name",
            "pgsql" | "postgres" => "SELECT current_database() as db_name",
            "sqlite" => "SELECT 'main' as db_name",
            _ => "SELECT '' as db_name",
        };

        let rows = self.connection.query(sql).await?;
        if let Some(row) = rows.first() {
            self.get_string_value(row, "db_name")
        } else {
            Ok(String::new())
        }
    }

    pub async fn get_database_version(&self) -> Result<String> {
        let sql = match self.db_type.as_str() {
            "mysql" => "SELECT VERSION() as version",
            "pgsql" | "postgres" => "SELECT version() as version",
            "sqlite" => "SELECT sqlite_version() as version",
            _ => "SELECT '' as version",
        };

        let rows = self.connection.query(sql).await?;
        if let Some(row) = rows.first() {
            self.get_string_value(row, "version")
        } else {
            Ok(String::new())
        }
    }

    fn get_string_value(&self, row: &serde_json::Value, key: &str) -> Result<String> {
        row.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Field not found or not a string: {}", key))
    }

    fn get_u64_value(&self, row: &serde_json::Value, key: &str) -> Result<u64> {
        row.get(key)
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("Field not found or not a u64: {}", key))
    }

    fn get_i64_value(&self, row: &serde_json::Value, key: &str) -> Result<i64> {
        row.get(key)
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow!("Field not found or not an i64: {}", key))
    }

    fn get_u32_value(&self, row: &serde_json::Value, key: &str) -> Result<u32> {
        row.get(key)
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .ok_or_else(|| anyhow!("Field not found or not a u32: {}", key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_metadata_serialization() {
        let field = FieldMetadata {
            name: "id".to_string(),
            data_type: "INT".to_string(),
            is_nullable: false,
            is_primary_key: true,
            is_foreign_key: false,
            default_value: None,
            max_length: None,
            precision: Some(11),
            scale: None,
            character_set: None,
            collation: Some("utf8mb4_general_ci".to_string()),
            extra: Some("auto_increment".to_string()),
            comment: Some("Primary key".to_string()),
        };

        let json = serde_json::to_string(&field).unwrap();
        assert!(json.contains("id"));
        assert!(json.contains("INT"));
    }

    #[test]
    fn test_table_metadata_serialization() {
        let table = TableMetadata {
            name: "users".to_string(),
            schema: Some("public".to_string()),
            engine: Some("InnoDB".to_string()),
            row_count: 1000,
            data_length: 65536,
            index_length: 16384,
            auto_increment: Some(1001),
            collation: Some("utf8mb4_general_ci".to_string()),
            fields: Vec::new(),
            foreign_keys: Vec::new(),
            indexes: Vec::new(),
            comment: Some("User table".to_string()),
        };

        let json = serde_json::to_string(&table).unwrap();
        assert!(json.contains("users"));
        assert!(json.contains("1000"));
    }
}
