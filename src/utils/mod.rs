/// 工具函数模块

/// 错误处理工具
pub mod error;
pub use error::{SqlToolError, SqlResult, ErrorLevel, ErrorCategory};

/// 操作结果工具
pub mod result;
pub use result::{OperationResult, OperationDetails, TransferDetails, BackupDetails, CompareDetails, SyncDetails, OperationTimer, OperationBuilder, IntoOperationResult};

/// 数据验证工具
pub mod validation;
pub use validation::{DataValidator, DataTransformer, DataFilter};

/// 性能优化工具
pub mod performance;
pub use performance::{BatchConfig, BatchProcessor, ParallelBatchProcessor, ProgressTracker, ProgressInfo, PerformanceMonitor, PerformanceMetrics, PerformanceSummary, ConnectionPoolOptimizer};

/// 安全防御工具
pub mod security;
pub use security::{SqlInjectionDetector, InjectionReport, RiskLevel, Finding, FindingCategory, FieldSecurityValidator, SimpleEncryptor, SafeSqlBuilder};

/// 连接字符串验证
pub mod connection_validator;
pub use connection_validator::{ConnectionString, validate_connection_string};

/// 字符串处理工具
pub mod string {
    /// 计算两个字符串的相似度
    pub fn similarity(s1: &str, s2: &str) -> f64 {
        let len1 = s1.len();
        let len2 = s2.len();
        
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }
        
        let distance = levenshtein_distance(s1, s2);
        1.0 - (distance as f64 / std::cmp::max(len1, len2) as f64)
    }
    
    /// 计算Levenshtein距离
    fn levenshtein_distance(s1: &str, s2: &str) -> usize {
        let len1 = s1.len();
        let len2 = s2.len();
        
        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
        
        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        
        for j in 0..=len2 {
            matrix[0][j] = j;
        }
        
        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if s1.chars().nth(i - 1) == s2.chars().nth(j - 1) {
                    0
                } else {
                    1
                };
                
                matrix[i][j] = std::cmp::min(
                    std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                    matrix[i - 1][j - 1] + cost,
                );
            }
        }
        
        matrix[len1][len2]
    }
}

/// 配置工具
pub mod config {
    use serde::{Deserialize, Serialize};
    use std::fs::File;
    use std::io::Read;
    
    /// 配置结构
    #[derive(Debug, Serialize, Deserialize)]
    pub struct Config {
        pub database: DatabaseConfig,
        pub transfer: TransferConfig,
    }
    
    /// 数据库配置
    #[derive(Debug, Serialize, Deserialize)]
    pub struct DatabaseConfig {
        pub connection_timeout: u32,
        pub max_connections: u32,
    }
    
    /// 转移配置
    #[derive(Debug, Serialize, Deserialize)]
    pub struct TransferConfig {
        pub batch_size: u32,
        pub commit_interval: u32,
    }
    
    /// 从文件加载配置
    pub fn load_config(file_path: &str) -> anyhow::Result<Config> {
        let mut file = File::open(file_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
}
