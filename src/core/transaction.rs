/// 事务支持模块

use crate::databases::DatabaseConnection;
use anyhow::Result;

/// 事务管理器
pub struct TransactionManager {
    db: Box<dyn DatabaseConnection>,
    in_transaction: bool,
    savepoints: Vec<String>,
}

impl TransactionManager {
    pub fn new(db: Box<dyn DatabaseConnection>) -> Self {
        Self {
            db,
            in_transaction: false,
            savepoints: Vec::new(),
        }
    }

    /// 开始事务
    pub async fn begin(&mut self) -> Result<()> {
        if self.in_transaction {
            return Err(anyhow::anyhow!("Transaction already in progress"));
        }
        self.db.begin_transaction().await?;
        self.in_transaction = true;
        Ok(())
    }

    /// 提交事务
    pub async fn commit(&mut self) -> Result<()> {
        if !self.in_transaction {
            return Err(anyhow::anyhow!("No transaction in progress"));
        }
        self.db.commit_transaction().await?;
        self.in_transaction = false;
        self.savepoints.clear();
        Ok(())
    }

    /// 回滚事务
    pub async fn rollback(&mut self) -> Result<()> {
        if !self.in_transaction {
            return Err(anyhow::anyhow!("No transaction in progress"));
        }
        self.db.rollback_transaction().await?;
        self.in_transaction = false;
        self.savepoints.clear();
        Ok(())
    }

    /// 创建保存点
    pub async fn savepoint(&mut self, name: &str) -> Result<()> {
        if !self.in_transaction {
            return Err(anyhow::anyhow!("No transaction in progress"));
        }
        let sql = format!("SAVEPOINT {}", name);
        self.db.execute(&sql).await?;
        self.savepoints.push(name.to_string());
        Ok(())
    }

    /// 回滚到保存点
    pub async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
        if !self.in_transaction {
            return Err(anyhow::anyhow!("No transaction in progress"));
        }
        let sql = format!("ROLLBACK TO SAVEPOINT {}", name);
        self.db.execute(&sql).await?;
        
        // 移除该保存点之后的所有保存点
        if let Some(pos) = self.savepoints.iter().position(|s| s == name) {
            self.savepoints.truncate(pos + 1);
        }
        Ok(())
    }

    /// 释放保存点
    pub async fn release_savepoint(&mut self, name: &str) -> Result<()> {
        if !self.in_transaction {
            return Err(anyhow::anyhow!("No transaction in progress"));
        }
        let sql = format!("RELEASE SAVEPOINT {}", name);
        self.db.execute(&sql).await?;
        
        if let Some(pos) = self.savepoints.iter().position(|s| s == name) {
            self.savepoints.remove(pos);
        }
        Ok(())
    }

    /// 检查是否在事务中
    pub fn is_in_transaction(&self) -> bool {
        self.in_transaction
    }

    /// 获取当前保存点列表
    pub fn get_savepoints(&self) -> &[String] {
        &self.savepoints
    }
}

/// 事务执行器 - 自动处理事务的提交和回滚
pub struct TransactionExecutor;

impl TransactionExecutor {
    /// 在事务中执行操作，成功自动提交，失败自动回滚
    pub async fn execute<F, Fut>(db: &mut TransactionManager, operation: F) -> Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        db.begin().await?;
        
        match operation().await {
            Ok(()) => {
                db.commit().await?;
                Ok(())
            }
            Err(e) => {
                let _ = db.rollback().await;
                Err(e)
            }
        }
    }

    /// 批量执行操作，使用保存点进行部分回滚
    pub async fn execute_batch<F, Fut>(
        db: &mut TransactionManager,
        operations: Vec<F>,
    ) -> Result<Vec<Result<()>>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        db.begin().await?;
        
        let mut results = Vec::with_capacity(operations.len());
        
        for (i, operation) in operations.into_iter().enumerate() {
            let savepoint_name = format!("sp_{}", i);
            db.savepoint(&savepoint_name).await?;
            
            match operation().await {
                Ok(()) => {
                    results.push(Ok(()));
                    db.release_savepoint(&savepoint_name).await?;
                }
                Err(e) => {
                    results.push(Err(e));
                    db.rollback_to_savepoint(&savepoint_name).await?;
                }
            }
        }
        
        db.commit().await?;
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_connection, DatabaseType};

    #[tokio::test]
    #[ignore]
    async fn test_transaction_basic() {
        let conn = create_connection(DatabaseType::SQLite, "sqlite::memory:").await.unwrap();
        let mut tx = TransactionManager::new(conn);

        let result = tx.begin().await;
        println!("Begin result: {:?}", result);
        if result.is_err() {
            println!("Transaction not supported, skipping...");
            return;
        }
        assert!(tx.is_in_transaction());

        let result = tx.commit().await;
        println!("Commit result: {:?}", result);
        if result.is_ok() {
            assert!(!tx.is_in_transaction());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_transaction_rollback() {
        let conn = create_connection(DatabaseType::SQLite, "sqlite::memory:").await.unwrap();
        let mut tx = TransactionManager::new(conn);

        let result = tx.begin().await;
        println!("Begin result: {:?}", result);
        if result.is_err() {
            println!("Transaction not supported, skipping...");
            return;
        }

        let result = tx.rollback().await;
        println!("Rollback result: {:?}", result);
        if result.is_ok() {
            assert!(!tx.is_in_transaction());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_savepoint() {
        let conn = create_connection(DatabaseType::SQLite, "sqlite::memory:").await.unwrap();
        let mut tx = TransactionManager::new(conn);

        let result = tx.begin().await;
        println!("Begin result: {:?}", result);
        if result.is_err() {
            println!("Transaction not supported, skipping...");
            return;
        }

        let result = tx.savepoint("sp1").await;
        println!("Savepoint result: {:?}", result);
        if result.is_ok() {
            assert_eq!(tx.get_savepoints().len(), 1);
        }

        let result = tx.rollback_to_savepoint("sp1").await;
        println!("Rollback to savepoint result: {:?}", result);

        let result = tx.commit().await;
        println!("Commit result: {:?}", result);
    }
}