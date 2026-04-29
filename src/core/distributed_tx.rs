use crate::databases::DatabaseConnection;
use crate::core::sharding::ShardInfo;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedTransaction {
    pub tx_id: String,
    pub participants: Vec<TxParticipant>,
    pub status: TxStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxParticipant {
    pub shard_id: String,
    pub connection_info: String,
    pub status: ParticipantStatus,
    pub prepared_at: Option<i64>,
    pub committed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxStatus {
    Initializing,
    Preparing,
    Prepared,
    Committing,
    Committed,
    Aborting,
    Aborted,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantStatus {
    Pending,
    Prepared,
    Committed,
    Aborted,
}

pub struct DistributedTxManager {
    active_transactions: RwLock<HashMap<String, Arc<Mutex<DistributedTransaction>>>>,
    coordinator: TxCoordinator,
}

#[derive(Clone)]
pub struct TxCoordinator {
    transactions: Arc<RwLock<HashMap<String, DistributedTransaction>>>,
    participant_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
}

impl TxCoordinator {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            participant_locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_transaction(&self, tx_id: &str) -> Result<DistributedTransaction> {
        let tx = DistributedTransaction {
            tx_id: tx_id.to_string(),
            participants: Vec::new(),
            status: TxStatus::Initializing,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };
        
        let mut transactions = self.transactions.write().unwrap();
        transactions.insert(tx_id.to_string(), tx.clone());
        
        Ok(tx)
    }

    pub async fn add_participant(&self, tx_id: &str, shard: &ShardInfo) -> Result<()> {
        let mut transactions = self.transactions.write().unwrap();

        if let Some(tx) = transactions.get_mut(tx_id) {
            let participant = TxParticipant {
                shard_id: format!("shard_{}", shard.shard_index),
                connection_info: shard.target_db.clone().unwrap_or_default(),
                status: ParticipantStatus::Pending,
                prepared_at: None,
                committed_at: None,
            };
            tx.participants.push(participant);
            tx.updated_at = chrono::Utc::now().timestamp();
            Ok(())
        } else {
            Err(anyhow!("Transaction not found: {}", tx_id))
        }
    }

    pub async fn prepare(&self, tx_id: &str) -> Result<bool> {
        let mut transactions = self.transactions.write().unwrap();
        
        if let Some(tx) = transactions.get_mut(tx_id) {
            tx.status = TxStatus::Preparing;
            tx.updated_at = chrono::Utc::now().timestamp();
            
            for participant in &mut tx.participants {
                participant.status = ParticipantStatus::Prepared;
                participant.prepared_at = Some(chrono::Utc::now().timestamp());
            }
            
            tx.status = TxStatus::Prepared;
            tx.updated_at = chrono::Utc::now().timestamp();
            
            Ok(true)
        } else {
            Err(anyhow!("Transaction not found: {}", tx_id))
        }
    }

    pub async fn commit(&self, tx_id: &str) -> Result<()> {
        let mut transactions = self.transactions.write().unwrap();
        
        if let Some(tx) = transactions.get_mut(tx_id) {
            tx.status = TxStatus::Committing;
            tx.updated_at = chrono::Utc::now().timestamp();
            
            for participant in &mut tx.participants {
                participant.status = ParticipantStatus::Committed;
                participant.committed_at = Some(chrono::Utc::now().timestamp());
            }
            
            tx.status = TxStatus::Committed;
            tx.updated_at = chrono::Utc::now().timestamp();
            
            Ok(())
        } else {
            Err(anyhow!("Transaction not found: {}", tx_id))
        }
    }

    pub async fn rollback(&self, tx_id: &str) -> Result<()> {
        let mut transactions = self.transactions.write().unwrap();
        
        if let Some(tx) = transactions.get_mut(tx_id) {
            tx.status = TxStatus::Aborting;
            tx.updated_at = chrono::Utc::now().timestamp();
            
            for participant in &mut tx.participants {
                participant.status = ParticipantStatus::Aborted;
            }
            
            tx.status = TxStatus::RolledBack;
            tx.updated_at = chrono::Utc::now().timestamp();
            
            Ok(())
        } else {
            Err(anyhow!("Transaction not found: {}", tx_id))
        }
    }

    pub async fn get_transaction(&self, tx_id: &str) -> Option<DistributedTransaction> {
        let transactions = self.transactions.read().unwrap();
        transactions.get(tx_id).cloned()
    }
}

impl Default for TxCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl DistributedTxManager {
    pub fn new() -> Self {
        Self {
            active_transactions: RwLock::new(HashMap::new()),
            coordinator: TxCoordinator::new(),
        }
    }

    pub async fn begin_distributed_tx(&self, tx_id: &str) -> Result<DistributedTransaction> {
        let tx = self.coordinator.create_transaction(tx_id).await?;
        let tx_arc = Arc::new(Mutex::new(tx.clone()));
        
        let mut active = self.active_transactions.write().unwrap();
        active.insert(tx_id.to_string(), tx_arc);
        
        Ok(tx)
    }

    pub async fn add_participant(&self, tx_id: &str, shard: &ShardInfo) -> Result<()> {
        self.coordinator.add_participant(tx_id, shard).await
    }

    pub async fn prepare(&self, tx_id: &str) -> Result<bool> {
        self.coordinator.prepare(tx_id).await
    }

    pub async fn commit(&self, tx_id: &str) -> Result<()> {
        self.coordinator.commit(tx_id).await
    }

    pub async fn rollback(&self, tx_id: &str) -> Result<()> {
        self.coordinator.rollback(tx_id).await
    }

    pub async fn two_phase_commit(&self, tx_id: &str) -> Result<()> {
        let prepared = self.prepare(tx_id).await?;
        
        if prepared {
            self.commit(tx_id).await?;
        } else {
            self.rollback(tx_id).await?;
        }
        
        let mut active = self.active_transactions.write().unwrap();
        active.remove(tx_id);
        
        Ok(())
    }

    pub async fn execute_in_distributed_tx<F, Fut>(
        &self,
        tx_id: &str,
        operations: Vec<F>,
    ) -> Result<()>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let tx = self.begin_distributed_tx(tx_id).await?;
        
        for (participant, operation) in tx.participants.iter().zip(operations) {
            if let Err(e) = operation(participant.shard_id.clone()).await {
                self.rollback(tx_id).await?;
                return Err(e);
            }
        }
        
        self.two_phase_commit(tx_id).await?;
        Ok(())
    }
}

impl Default for DistributedTxManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct SagaTransaction {
    pub saga_id: String,
    pub steps: Vec<SagaStep>,
    pub compensations: Vec<CompensationStep>,
    pub status: SagaStatus,
}

#[derive(Debug, Clone)]
pub struct SagaStep {
    pub step_id: String,
    pub shard_id: String,
    pub operation: String,
    pub parameters: Vec<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct CompensationStep {
    pub step_id: String,
    pub original_step_id: String,
    pub compensating_operation: String,
    pub parameters: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SagaStatus {
    Running,
    Completed,
    Compensating,
    Compensated,
    Failed,
}

pub struct SagaManager {
    transactions: RwLock<HashMap<String, SagaTransaction>>,
}

impl SagaManager {
    pub fn new() -> Self {
        Self {
            transactions: RwLock::new(HashMap::new()),
        }
    }

    pub fn create_saga(&self, saga_id: &str) -> Result<SagaTransaction> {
        let saga = SagaTransaction {
            saga_id: saga_id.to_string(),
            steps: Vec::new(),
            compensations: Vec::new(),
            status: SagaStatus::Running,
        };
        
        let mut transactions = self.transactions.write().unwrap();
        transactions.insert(saga_id.to_string(), saga.clone());
        
        Ok(saga)
    }

    pub fn add_step(&self, saga_id: &str, step: SagaStep, compensation: CompensationStep) -> Result<()> {
        let mut transactions = self.transactions.write().unwrap();
        
        if let Some(saga) = transactions.get_mut(saga_id) {
            saga.steps.push(step);
            saga.compensations.push(compensation);
            Ok(())
        } else {
            Err(anyhow!("Saga not found: {}", saga_id))
        }
    }

    pub fn execute_saga<'a, F, Fut>(&self, saga_id: &'a str, executor: F) -> Result<()>
    where
        F: Fn(&SagaStep) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let saga = {
            let transactions = self.transactions.read().unwrap();
            transactions.get(saga_id).cloned()
        };

        let saga = saga.ok_or_else(|| anyhow!("Saga not found: {}", saga_id))?;

        let mut executed_steps: Vec<usize> = Vec::new();
        let rt = tokio::runtime::Runtime::new().unwrap();

        for (i, step) in saga.steps.iter().enumerate() {
            let result = rt.block_on(executor(step));
            match result {
                Ok(()) => {
                    executed_steps.push(i);
                }
                Err(e) => {
                    let _ = self.compensate_saga_sync(saga_id, executed_steps);
                    return Err(e);
                }
            }
        }

        let mut transactions = self.transactions.write().unwrap();
        if let Some(saga) = transactions.get_mut(saga_id) {
            saga.status = SagaStatus::Completed;
        }

        Ok(())
    }

    fn compensate_saga_sync(&self, saga_id: &str, _executed_steps: Vec<usize>) -> Result<()> {
        let mut transactions = self.transactions.write().unwrap();

        if let Some(saga) = transactions.get_mut(saga_id) {
            saga.status = SagaStatus::Compensating;
        }

        if let Some(saga) = transactions.get_mut(saga_id) {
            saga.status = SagaStatus::Compensated;
        }

        Ok(())
    }
}

impl Default for SagaManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct XaTransaction {
    pub xid: String,
    pub format_id: i32,
    pub global_transaction_id: Vec<u8>,
    pub branch_qualifier: Vec<u8>,
    pub resourceManagers: HashMap<String, XaResourceManager>,
    pub state: XaState,
}

pub struct XaResourceManager {
    pub name: String,
    pub connection: Box<dyn DatabaseConnection>,
    pub prepared: bool,
}

impl std::fmt::Debug for XaResourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XaResourceManager")
            .field("name", &self.name)
            .field("prepared", &self.prepared)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XaState {
    Active,
    Idle,
    Prepared,
    MarkedRollback,
    RolledBack,
    Committed,
    Unknown,
}

impl XaTransaction {
    pub fn new(xid: &str) -> Self {
        Self {
            xid: xid.to_string(),
            format_id: 0,
            global_transaction_id: xid.as_bytes().to_vec(),
            branch_qualifier: Vec::new(),
            resourceManagers: HashMap::new(),
            state: XaState::Active,
        }
    }

    pub fn add_resource_manager(&mut self, name: &str, conn: Box<dyn DatabaseConnection>) {
        self.resourceManagers.insert(name.to_string(), XaResourceManager {
            name: name.to_string(),
            connection: conn,
            prepared: false,
        });
    }

    pub async fn start(&mut self) -> Result<()> {
        for (_, rm) in &mut self.resourceManagers {
            let sql = format!("XA START '{}'", self.xid);
            rm.connection.execute(&sql).await?;
        }
        self.state = XaState::Active;
        Ok(())
    }

    pub async fn end(&mut self) -> Result<()> {
        for (_, rm) in &mut self.resourceManagers {
            let sql = format!("XA END '{}'", self.xid);
            rm.connection.execute(&sql).await?;
        }
        self.state = XaState::Idle;
        Ok(())
    }

    pub async fn prepare(&mut self) -> Result<bool> {
        for (_, rm) in &mut self.resourceManagers {
            let sql = format!("XA PREPARE '{}'", self.xid);
            rm.connection.execute(&sql).await?;
            rm.prepared = true;
        }
        self.state = XaState::Prepared;
        Ok(true)
    }

    pub async fn commit(&mut self) -> Result<()> {
        for (_, rm) in &mut self.resourceManagers {
            let sql = format!("XA COMMIT '{}'", self.xid);
            rm.connection.execute(&sql).await?;
        }
        self.state = XaState::Committed;
        Ok(())
    }

    pub async fn rollback(&mut self) -> Result<()> {
        for (_, rm) in &mut self.resourceManagers {
            let sql = format!("XA ROLLBACK '{}'", self.xid);
            rm.connection.execute(&sql).await?;
        }
        self.state = XaState::RolledBack;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distributed_transaction_creation() {
        let coordinator = TxCoordinator::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let tx = coordinator.create_transaction("tx_001").await.unwrap();
            assert_eq!(tx.tx_id, "tx_001");
            assert_eq!(tx.status, TxStatus::Initializing);
        });
    }

    #[test]
    fn test_saga_transaction() {
        let manager = SagaManager::new();
        
        let saga = manager.create_saga("saga_001").unwrap();
        assert_eq!(saga.saga_id, "saga_001");
        assert_eq!(saga.status, SagaStatus::Running);
    }

    #[test]
    fn test_xa_transaction() {
        let xa_tx = XaTransaction::new("xa_001");
        assert_eq!(xa_tx.xid, "xa_001");
        assert_eq!(xa_tx.state, XaState::Active);
    }
}
