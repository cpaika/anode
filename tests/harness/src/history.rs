//! Operation history tracking for linearizability checking

use crate::workload::Operation;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;

/// Result of an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationResult {
    /// Operation succeeded
    Ok(OperationValue),
    /// Operation failed
    Err(String),
}

/// Value returned by an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationValue {
    /// Get operation returned data
    Data(Vec<u8>),
    /// Operation succeeded with no return value
    Unit,
    /// Operation returned a list of keys
    Keys(Vec<String>),
    /// Object not found
    NotFound,
}

/// A recorded operation with timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unique operation identifier
    pub id: u64,
    /// The operation that was performed
    pub operation: Operation,
    /// When the operation was invoked
    pub invoke_time: SystemTime,
    /// When the operation completed
    pub complete_time: Option<SystemTime>,
    /// The result of the operation
    pub result: Option<OperationResult>,
    /// Which node executed the operation
    pub node_id: Option<u64>,
}

impl HistoryEntry {
    /// Create a new history entry
    pub fn new(id: u64, operation: Operation) -> Self {
        Self {
            id,
            operation,
            invoke_time: SystemTime::now(),
            complete_time: None,
            result: None,
            node_id: None,
        }
    }

    /// Mark the operation as complete
    pub fn complete(&mut self, result: OperationResult) {
        self.complete_time = Some(SystemTime::now());
        self.result = Some(result);
    }

    /// Get the operation duration if completed
    pub fn duration(&self) -> Option<Duration> {
        self.complete_time
            .and_then(|complete| complete.duration_since(self.invoke_time).ok())
    }

    /// Check if the operation is complete
    pub fn is_complete(&self) -> bool {
        self.complete_time.is_some()
    }

    /// Check if the operation succeeded
    pub fn is_success(&self) -> bool {
        matches!(self.result, Some(OperationResult::Ok(_)))
    }
}

/// History of operations for linearizability checking
#[derive(Debug, Clone)]
pub struct OperationHistory {
    entries: Arc<Mutex<Vec<HistoryEntry>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
}

impl OperationHistory {
    /// Create a new operation history
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Record a new operation invocation
    pub async fn record_invoke(&self, operation: Operation) -> u64 {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let entry = HistoryEntry::new(id, operation);
        self.entries.lock().await.push(entry);
        id
    }

    /// Record an operation completion
    pub async fn record_complete(&self, id: u64, result: OperationResult) {
        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            entry.complete(result);
        }
    }

    /// Get all entries
    pub async fn get_entries(&self) -> Vec<HistoryEntry> {
        self.entries.lock().await.clone()
    }

    /// Get completed entries
    pub async fn get_completed_entries(&self) -> Vec<HistoryEntry> {
        self.entries
            .lock()
            .await
            .iter()
            .filter(|e| e.is_complete())
            .cloned()
            .collect()
    }

    /// Get successful entries
    pub async fn get_successful_entries(&self) -> Vec<HistoryEntry> {
        self.entries
            .lock()
            .await
            .iter()
            .filter(|e| e.is_success())
            .cloned()
            .collect()
    }

    /// Get failed entries
    pub async fn get_failed_entries(&self) -> Vec<HistoryEntry> {
        self.entries
            .lock()
            .await
            .iter()
            .filter(|e| e.is_complete() && !e.is_success())
            .cloned()
            .collect()
    }

    /// Clear the history
    pub async fn clear(&self) {
        self.entries.lock().await.clear();
        self.next_id.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    /// Get statistics about the history
    pub async fn stats(&self) -> HistoryStats {
        let entries = self.entries.lock().await;

        let total = entries.len();
        let completed = entries.iter().filter(|e| e.is_complete()).count();
        let successful = entries.iter().filter(|e| e.is_success()).count();
        let failed = completed - successful;

        let durations: Vec<Duration> = entries.iter().filter_map(|e| e.duration()).collect();

        let avg_duration = if !durations.is_empty() {
            Some(durations.iter().sum::<Duration>() / durations.len() as u32)
        } else {
            None
        };

        HistoryStats {
            total,
            completed,
            successful,
            failed,
            avg_duration,
        }
    }

    /// Export history to JSON
    pub async fn export_json(&self) -> serde_json::Result<String> {
        let entries = self.entries.lock().await;
        serde_json::to_string_pretty(&*entries)
    }

    /// Import history from JSON
    pub async fn import_json(&self, json: &str) -> serde_json::Result<()> {
        let entries: Vec<HistoryEntry> = serde_json::from_str(json)?;
        *self.entries.lock().await = entries;
        Ok(())
    }
}

impl Default for OperationHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about operation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryStats {
    pub total: usize,
    pub completed: usize,
    pub successful: usize,
    pub failed: usize,
    pub avg_duration: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workload::Operation;

    #[tokio::test]
    async fn test_history() {
        let history = OperationHistory::new();

        let op = Operation::Put {
            bucket: "test".to_string(),
            key: "key1".to_string(),
            value: vec![1, 2, 3],
        };

        let id = history.record_invoke(op).await;
        assert_eq!(id, 0);

        history
            .record_complete(id, OperationResult::Ok(OperationValue::Unit))
            .await;

        let entries = history.get_entries().await;
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_complete());
        assert!(entries[0].is_success());
    }

    #[tokio::test]
    async fn test_history_stats() {
        let history = OperationHistory::new();

        for i in 0..10 {
            let op = Operation::Get {
                bucket: "test".to_string(),
                key: format!("key{}", i),
            };
            let id = history.record_invoke(op).await;

            if i % 2 == 0 {
                history
                    .record_complete(id, OperationResult::Ok(OperationValue::Data(vec![i as u8])))
                    .await;
            } else {
                history
                    .record_complete(id, OperationResult::Err("not found".to_string()))
                    .await;
            }
        }

        let stats = history.stats().await;
        assert_eq!(stats.total, 10);
        assert_eq!(stats.completed, 10);
        assert_eq!(stats.successful, 5);
        assert_eq!(stats.failed, 5);
    }

    #[tokio::test]
    async fn test_history_export_import() {
        let history = OperationHistory::new();

        let op = Operation::Put {
            bucket: "test".to_string(),
            key: "key1".to_string(),
            value: vec![1, 2, 3],
        };

        let id = history.record_invoke(op).await;
        history
            .record_complete(id, OperationResult::Ok(OperationValue::Unit))
            .await;

        let json = history.export_json().await.unwrap();

        let history2 = OperationHistory::new();
        history2.import_json(&json).await.unwrap();

        let entries = history2.get_entries().await;
        assert_eq!(entries.len(), 1);
    }
}
