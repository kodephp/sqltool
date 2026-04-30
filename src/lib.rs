//! SQLTool 库 - 数据库数据和结构转移工具
//!
//! 提供数据库之间的数据转移、结构迁移和比较功能
//! 支持多种数据库类型，包括 MySQL、PostgreSQL、SQLite、Redis、MongoDB 等

#![allow(unused)]
#![allow(non_snake_case)]

use env_logger;

pub mod core;
pub mod databases;
pub mod models;
pub mod commands;
pub mod utils;

#[cfg(test)]
mod tests;

/// 库的版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 初始化库
pub fn init() {
    env_logger::init();
}

/// 导出核心功能
pub use core::DataTransfer;
pub use core::StructureMigration;
pub use core::SchemaComparator;
pub use core::FieldMapper;
pub use core::SchemaComparison;
pub use core::SmartFieldMatcher;
pub use core::DatabaseBackup;
pub use core::{BackupConfig, BackupType, BackupMetadata, BackupProgress, BackupPhase, RestoreReport, BackupVerificationReport, IncrementalBackupTracker, ChangeRecord};
pub use core::{DataCompareConfig, CompareMode, CompareResult, SchemaDiff, SchemaDiffType, DataDiff, DataDiffType, RowDiff, CompareStats, DataComparer};
pub use core::{TransactionManager, TransactionExecutor};
pub use core::{IncrementalSync, IncrementalConfig, ConflictResolution, SyncResult};
pub use core::{SqlGenerator, SqlGeneratorConfig, FieldConnectionMapper, PlaceholderStyle};
pub use core::{TableSync, TableSyncConfig, SyncMode, ConflictStrategy};
pub use core::{DatabaseMaintainer, DatabaseBackup as DbBackup, DatabaseRestore, OptimizationResult, RepairResult, CleanupResult, DatabaseStatus, BackupResult, RestoreResult};
pub use core::{FieldMatcherConfig, MatchStrategy, MatchResult, MatchType, TargetSchemaInfo, ConversionSuggestionGenerator, ConversionSuggestion, SuggestionType, Priority, ConversionSuggestions};
pub use core::{RedisConverter, MongoDbConverter, KvConversionConfig, KvDataType, JsonMode, FieldMergeStrategy, RedisCommand, MongoOperation, KvConversionResult, BatchKvConverter};
pub use core::{TimeSeriesConfig, TimeSeriesConverter, InfluxDbConverter, TimescaleDbConverter, TaodbConverter, TimeSeriesPoint, MetricValue, PartitionStrategy, TimeInterval, BatchWriteResult, WriteStatus};
pub use core::{ShardingConfig, ShardingStrategy, ShardingManager, ShardInfo, ShardParam, ShardMigrationPlan, ShardMigrationTask, ShardRouter, GlobalShardingManager};
pub use core::{QueryFusionEngine, FusionQuery, FusionResult, FusionStrategy, CrossShardPlan, QueryAnalysis, QueryType, ComplexityLevel, UnionStrategy, JoinStrategy, JoinType};
pub use core::{DistributedTransaction, TxParticipant, TxStatus, ParticipantStatus, DistributedTxManager, TxCoordinator, SagaTransaction, SagaStep, CompensationStep, SagaStatus, SagaManager, XaTransaction, XaResourceManager, XaState};
pub use core::{MultiDatabaseConfig, DatabaseInstanceConfig, DatabaseInstance, MultiDatabaseManager, DatabaseInfo, HealthStatus, BackupInfo, BackupStatus, DatabaseRouter, RoutingRule, TableRouter, TableShardingRule, ShardingType as DbShardingType};
pub use core::{DatabaseMetadata, TableMetadata, FieldMetadata, ForeignKeyMetadata, ViewMetadata, FunctionMetadata, FunctionArgument, StoredProcedureMetadata, TriggerMetadata, IndexMetadata, MetadataManager};
pub use core::{SqlExecutor, ExecutionResult, ExecutionMetadata, QueryBuilder, SqlType};
pub use core::{Session, SessionManager, DatabaseSession, ViewManager, FunctionManager, IndexManager, ViewOperation};
pub use core::{SlowQueryConfig, SlowQueryLog, QueryProfile, IndexUsageInfo, QueryOptimizationSuggestion, IssueType, Severity, SlowQueryDetector, QueryMetrics};
pub use core::{NewSqlAnalyzer, NewSqlCapabilities, PerformanceMetrics, DistributionInfo, NodeHealth, NodeStatus, NodeRole, ShardAnalysis, ShardingType, DataSkewReport, ConflictAnalysis, HotRecord, NewSqlOptimizationResult, QueryRouter, RoutingRule as SqlRoutingRule, ConsistencyCheck, ConsistencyStatus, RepairReport, NewSqlShardInfo, SplitPoint, NewSqlAnalyzerBuilder};
pub use core::{TableConfig, ShardStrategy, ShardInfo as AutoShardInfo, SpanningQueryResult, CompressionConfig, AutoShardingManager, StorageBackend, CompressionHandler, SmartAutoOptimizer, OptimizationRule, OptimizationReport, ShardSummary};
pub use core::{LogTableConfig, PartitionType, LogPartition, LogQuery, LogQueryResult, LogStats, LogTableManager, LogAggregator, TimeWindowAggregation};
pub use databases::{DatabaseConnection, DatabaseType, create_connection, DatabaseVersion, DatabaseCompatibility, MySqlCompatibility, PostgresCompatibility, SqliteCompatibility};
pub use databases::connection::{ConnectionConfig, MySqlConfig, PostgresConfig, SqliteConfig, RedisConfig, SslMode, JournalMode};
pub use models::{TableSchema, Field, FieldMapping, TableMapping};
pub use commands::{Args, execute, start_http_server};
pub use utils::{SqlToolError, SqlResult, DataValidator, DataTransformer, DataFilter, SqlInjectionDetector, InjectionReport, RiskLevel, Finding, FindingCategory, FieldSecurityValidator, SimpleEncryptor, SafeSqlBuilder};
pub use utils::{OperationResult, OperationDetails, TransferDetails, BackupDetails, CompareDetails, SyncDetails, OperationTimer, OperationBuilder};
pub use utils::{ConnectionString, validate_connection_string};

/// FFI模块，用于其他语言调用
pub mod ffi;
pub use ffi::*;
