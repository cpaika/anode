use crate::error::{RaftError, Result};
use crate::types::{NodeId, TypeConfig};
use openraft::storage::{LogState, RaftLogStorage, RaftStateMachine, Snapshot};
use openraft::{
    Entry, EntryPayload, LogId, RaftLogReader, RaftSnapshotBuilder, SnapshotMeta, StorageError,
    StoredMembership, Vote,
};
use parking_lot::RwLock;
use redb::{Database, ReadableTable, TableDefinition};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

// Table definitions for redb
const LOGS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("logs");
const VOTE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("vote");
const STATE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("state");

/// Raft log storage implementation using redb
pub struct LogStore {
    db: Database,
    /// In-memory cache of last applied log for quick access
    last_purged_log_id: Arc<RwLock<Option<LogId<NodeId>>>>,
}

impl LogStore {
    /// Open or create a log store at the given path
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;

        let db = Database::create(path)?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _logs = write_txn.open_table(LOGS_TABLE)?;
            let _vote = write_txn.open_table(VOTE_TABLE)?;
            let _state = write_txn.open_table(STATE_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self {
            db,
            last_purged_log_id: Arc::new(RwLock::new(None)),
        })
    }

    /// Get the last log ID in storage
    fn get_last_log_id(&self) -> Result<Option<LogId<NodeId>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LOGS_TABLE)?;

        if let Some(entry) = table.iter()?.last() {
            let (_, value) = entry?;
            let entry: Entry<TypeConfig> = bincode::deserialize(value.value())?;
            Ok(Some(entry.log_id))
        } else {
            Ok(None)
        }
    }

    /// Get entries in the specified range
    fn get_log_entries_range<RB: RangeBounds<u64>>(
        &self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LOGS_TABLE)?;

        let mut entries = Vec::new();

        // Convert range bounds to concrete values
        let start = match range.start_bound() {
            std::ops::Bound::Included(&n) => n,
            std::ops::Bound::Excluded(&n) => n + 1,
            std::ops::Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            std::ops::Bound::Included(&n) => n + 1,
            std::ops::Bound::Excluded(&n) => n,
            std::ops::Bound::Unbounded => u64::MAX,
        };

        for idx in start..end {
            if let Some(value) = table.get(idx)? {
                let entry: Entry<TypeConfig> = bincode::deserialize(value.value())?;
                entries.push(entry);
            } else {
                break;
            }
        }

        Ok(entries)
    }
}

#[async_trait::async_trait]
impl RaftLogReader<TypeConfig> for LogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Send + Sync>(
        &mut self,
        range: RB,
    ) -> std::result::Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>> {
        self.get_log_entries_range(range)
            .map_err(|e| StorageError::IO {
                source: openraft::error::StorageIOError::read(&e),
            })
    }

    async fn read_vote(&mut self) -> std::result::Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        let read_txn = self.db.begin_read().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::read(&e),
        })?;

        let table = read_txn.open_table(VOTE_TABLE).map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::read(&e),
        })?;

        if let Some(value) = table.get("vote").map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::read(&e),
        })? {
            let vote: Vote<NodeId> = bincode::deserialize(value.value()).map_err(|e| {
                StorageError::IO {
                    source: openraft::error::StorageIOError::read(&e),
                }
            })?;
            Ok(Some(vote))
        } else {
            Ok(None)
        }
    }
}

#[async_trait::async_trait]
impl RaftLogStorage<TypeConfig> for LogStore {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> std::result::Result<LogState<TypeConfig>, StorageError<NodeId>> {
        let last_purged_log_id = *self.last_purged_log_id.read();
        let last_log_id = self.get_last_log_id().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::read(&e),
        })?;

        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    async fn save_vote(
        &mut self,
        vote: &Vote<NodeId>,
    ) -> std::result::Result<(), StorageError<NodeId>> {
        let write_txn = self.db.begin_write().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::write(&e),
        })?;

        {
            let mut table = write_txn.open_table(VOTE_TABLE).map_err(|e| StorageError::IO {
                source: openraft::error::StorageIOError::write(&e),
            })?;

            let encoded = bincode::serialize(vote).map_err(|e| StorageError::IO {
                source: openraft::error::StorageIOError::write(&e),
            })?;

            table.insert("vote", encoded.as_slice()).map_err(|e| StorageError::IO {
                source: openraft::error::StorageIOError::write(&e),
            })?;
        }

        write_txn.commit().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::write(&e),
        })?;

        Ok(())
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: openraft::storage::LogFlushed<TypeConfig>,
    ) -> std::result::Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + Send,
        I::IntoIter: Send,
    {
        let write_txn = self.db.begin_write().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::write(&e),
        })?;

        let mut last_log_id = None;

        {
            let mut table = write_txn.open_table(LOGS_TABLE).map_err(|e| StorageError::IO {
                source: openraft::error::StorageIOError::write(&e),
            })?;

            for entry in entries {
                let index = entry.log_id.index;
                last_log_id = Some(entry.log_id);

                let encoded = bincode::serialize(&entry).map_err(|e| StorageError::IO {
                    source: openraft::error::StorageIOError::write(&e),
                })?;

                table.insert(index, encoded.as_slice()).map_err(|e| StorageError::IO {
                    source: openraft::error::StorageIOError::write(&e),
                })?;
            }
        }

        write_txn.commit().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::write(&e),
        })?;

        if let Some(log_id) = last_log_id {
            callback.log_io_completed(Ok(()));
        }

        Ok(())
    }

    async fn truncate(
        &mut self,
        log_id: LogId<NodeId>,
    ) -> std::result::Result<(), StorageError<NodeId>> {
        let write_txn = self.db.begin_write().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::write(&e),
        })?;

        {
            let mut table = write_txn.open_table(LOGS_TABLE).map_err(|e| StorageError::IO {
                source: openraft::error::StorageIOError::write(&e),
            })?;

            // Remove all entries after log_id.index
            let mut to_remove = Vec::new();
            for entry in table.range(log_id.index + 1..)? {
                let (key, _) = entry.map_err(|e| StorageError::IO {
                    source: openraft::error::StorageIOError::write(&e),
                })?;
                to_remove.push(key.value());
            }

            for key in to_remove {
                table.remove(key).map_err(|e| StorageError::IO {
                    source: openraft::error::StorageIOError::write(&e),
                })?;
            }
        }

        write_txn.commit().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::write(&e),
        })?;

        Ok(())
    }

    async fn purge(
        &mut self,
        log_id: LogId<NodeId>,
    ) -> std::result::Result<(), StorageError<NodeId>> {
        let write_txn = self.db.begin_write().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::write(&e),
        })?;

        {
            let mut table = write_txn.open_table(LOGS_TABLE).map_err(|e| StorageError::IO {
                source: openraft::error::StorageIOError::write(&e),
            })?;

            // Remove all entries up to and including log_id.index
            let mut to_remove = Vec::new();
            for entry in table.range(..=log_id.index)? {
                let (key, _) = entry.map_err(|e| StorageError::IO {
                    source: openraft::error::StorageIOError::write(&e),
                })?;
                to_remove.push(key.value());
            }

            for key in to_remove {
                table.remove(key).map_err(|e| StorageError::IO {
                    source: openraft::error::StorageIOError::write(&e),
                })?;
            }
        }

        write_txn.commit().map_err(|e| StorageError::IO {
            source: openraft::error::StorageIOError::write(&e),
        })?;

        *self.last_purged_log_id.write() = Some(log_id);

        Ok(())
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        Self {
            db: self.db.clone(),
            last_purged_log_id: self.last_purged_log_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openraft::testing::log_id;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_log_store_create() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let store = LogStore::open(&log_path).unwrap();
        assert!(log_path.exists());
    }

    #[tokio::test]
    async fn test_save_and_read_vote() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        let vote = Vote::new(5, 1);
        store.save_vote(&vote).await.unwrap();

        let read_vote = store.read_vote().await.unwrap();
        assert_eq!(read_vote, Some(vote));
    }

    #[tokio::test]
    async fn test_append_and_read_entries() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        use crate::types::{Command, Request};

        let entries = vec![
            Entry {
                log_id: log_id(1, 1, 1),
                payload: EntryPayload::Blank,
            },
            Entry {
                log_id: log_id(1, 1, 2),
                payload: EntryPayload::Normal(Request {
                    client_id: "test".to_string(),
                    serial: 1,
                    command: Command::CreateBucket {
                        name: "test-bucket".to_string(),
                        config: crate::types::BucketConfig {
                            name: "test-bucket".to_string(),
                            replication_factor: 3,
                            created_at: 0,
                        },
                    },
                }),
            },
        ];

        store
            .append(entries.clone(), openraft::storage::LogFlushed::new(None, None))
            .await
            .unwrap();

        let read_entries = store.try_get_log_entries(1..=2).await.unwrap();
        assert_eq!(read_entries.len(), 2);
    }

    #[tokio::test]
    async fn test_truncate() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        let entries: Vec<Entry<TypeConfig>> = (1..=5)
            .map(|i| Entry {
                log_id: log_id(1, 1, i),
                payload: EntryPayload::Blank,
            })
            .collect();

        store
            .append(entries, openraft::storage::LogFlushed::new(None, None))
            .await
            .unwrap();

        store.truncate(log_id(1, 1, 3)).await.unwrap();

        let read_entries = store.try_get_log_entries(1..=5).await.unwrap();
        assert_eq!(read_entries.len(), 3);
    }

    #[tokio::test]
    async fn test_purge() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        let entries: Vec<Entry<TypeConfig>> = (1..=5)
            .map(|i| Entry {
                log_id: log_id(1, 1, i),
                payload: EntryPayload::Blank,
            })
            .collect();

        store
            .append(entries, openraft::storage::LogFlushed::new(None, None))
            .await
            .unwrap();

        store.purge(log_id(1, 1, 2)).await.unwrap();

        let read_entries = store.try_get_log_entries(1..=5).await.unwrap();
        assert_eq!(read_entries.len(), 3);
        assert_eq!(read_entries[0].log_id.index, 3);
    }
}
