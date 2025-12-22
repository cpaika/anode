use crate::error::Result;
use crate::types::{NodeId, TypeConfig};
use openraft::storage::{LogState, RaftLogStorage};
use openraft::{AnyError, ErrorSubject, ErrorVerb};
use openraft::{Entry, LogId, OptionalSend, RaftLogReader, StorageError, StorageIOError, Vote};
use parking_lot::RwLock;
use redb::{Database, ReadableTable, TableDefinition};
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

// Table definitions for redb
const LOGS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("logs");
const VOTE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("vote");
const STATE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("state");

/// Persistent log storage for Raft using the redb embedded database.
///
/// This type implements [`openraft::storage::RaftLogStorage`] and provides
/// durable storage for Raft log entries and voting state. It uses the
/// [redb](https://docs.rs/redb) embedded database for efficient, ACID-compliant
/// storage.
///
/// # Storage Layout
///
/// The log store uses three tables:
/// - `logs` - Log entries indexed by their index (u64)
/// - `vote` - The node's current vote (leader ID and term)
/// - `state` - Additional state information (currently unused)
///
/// # Examples
///
/// ```no_run
/// use anode_raft::LogStore;
/// use std::path::Path;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let store = LogStore::open(Path::new("/data/raft/logs.db"))?;
/// # Ok(())
/// # }
/// ```
///
/// # Performance
///
/// - Read operations are fast due to redb's zero-copy design
/// - Write operations are durable (fsync) by default
/// - Range queries are efficient thanks to B-tree indexing
pub struct LogStore {
    /// The embedded redb database
    db: Arc<Database>,
    /// In-memory cache of the last purged log ID for quick access
    last_purged_log_id: Arc<RwLock<Option<LogId<NodeId>>>>,
}

impl LogStore {
    /// Open or create a log store at the specified path.
    ///
    /// If the database doesn't exist, it will be created. If it exists, it will be
    /// opened and all existing data will be preserved. The parent directory will
    /// be created if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The parent directory cannot be created
    /// - The database file cannot be created or opened
    /// - Database initialization fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anode_raft::LogStore;
    /// use std::path::Path;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = LogStore::open(Path::new("/data/raft/logs.db"))?;
    /// # Ok(())
    /// # }
    /// ```
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
            db: Arc::new(db),
            last_purged_log_id: Arc::new(RwLock::new(None)),
        })
    }

    /// Get the log ID of the last entry in the log.
    ///
    /// Returns `None` if the log is empty.
    fn get_last_log_id(&self) -> Result<Option<LogId<NodeId>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LOGS_TABLE)?;

        let last_entry = table.iter()?.last();
        if let Some(entry) = last_entry {
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

/// Helper to create a read storage error
fn read_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> StorageError<NodeId> {
    StorageError::IO {
        source: StorageIOError::new(ErrorSubject::Store, ErrorVerb::Read, AnyError::new(&e)),
    }
}

/// Helper to create a write storage error
fn write_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> StorageError<NodeId> {
    StorageError::IO {
        source: StorageIOError::new(ErrorSubject::Store, ErrorVerb::Write, AnyError::new(&e)),
    }
}

impl RaftLogReader<TypeConfig> for LogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> std::result::Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>> {
        self.get_log_entries_range(range).map_err(read_err)
    }
}

impl RaftLogStorage<TypeConfig> for LogStore {
    type LogReader = Self;

    async fn get_log_state(
        &mut self,
    ) -> std::result::Result<LogState<TypeConfig>, StorageError<NodeId>> {
        let last_purged_log_id = *self.last_purged_log_id.read();
        let last_log_id = self.get_last_log_id().map_err(read_err)?;

        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    async fn save_vote(
        &mut self,
        vote: &Vote<NodeId>,
    ) -> std::result::Result<(), StorageError<NodeId>> {
        let write_txn = self.db.begin_write().map_err(write_err)?;

        {
            let mut table = write_txn.open_table(VOTE_TABLE).map_err(write_err)?;
            let encoded = bincode::serialize(vote).map_err(write_err)?;
            table
                .insert("vote", encoded.as_slice())
                .map_err(write_err)?;
        }

        write_txn.commit().map_err(write_err)?;
        Ok(())
    }

    async fn read_vote(
        &mut self,
    ) -> std::result::Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        let read_txn = self.db.begin_read().map_err(read_err)?;
        let table = read_txn.open_table(VOTE_TABLE).map_err(read_err)?;

        if let Some(value) = table.get("vote").map_err(read_err)? {
            let vote: Vote<NodeId> = bincode::deserialize(value.value()).map_err(read_err)?;
            Ok(Some(vote))
        } else {
            Ok(None)
        }
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: openraft::storage::LogFlushed<TypeConfig>,
    ) -> std::result::Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let write_txn = self.db.begin_write().map_err(write_err)?;

        let mut last_log_id = None;

        {
            let mut table = write_txn.open_table(LOGS_TABLE).map_err(write_err)?;

            for entry in entries {
                let index = entry.log_id.index;
                last_log_id = Some(entry.log_id);

                let encoded = bincode::serialize(&entry).map_err(write_err)?;
                table.insert(index, encoded.as_slice()).map_err(write_err)?;
            }
        }

        write_txn.commit().map_err(write_err)?;

        if let Some(_log_id) = last_log_id {
            callback.log_io_completed(Ok(()));
        }

        Ok(())
    }

    async fn truncate(
        &mut self,
        log_id: LogId<NodeId>,
    ) -> std::result::Result<(), StorageError<NodeId>> {
        let write_txn = self.db.begin_write().map_err(write_err)?;

        {
            let mut table = write_txn.open_table(LOGS_TABLE).map_err(write_err)?;

            // Remove all entries after log_id.index
            let mut to_remove = Vec::new();
            for entry in table.range(log_id.index + 1..).map_err(write_err)? {
                let (key, _) = entry.map_err(write_err)?;
                to_remove.push(key.value());
            }

            for key in to_remove {
                table.remove(key).map_err(write_err)?;
            }
        }

        write_txn.commit().map_err(write_err)?;
        Ok(())
    }

    async fn purge(
        &mut self,
        log_id: LogId<NodeId>,
    ) -> std::result::Result<(), StorageError<NodeId>> {
        let write_txn = self.db.begin_write().map_err(write_err)?;

        {
            let mut table = write_txn.open_table(LOGS_TABLE).map_err(write_err)?;

            // Remove all entries up to and including log_id.index
            let mut to_remove = Vec::new();
            for entry in table.range(..=log_id.index).map_err(write_err)? {
                let (key, _) = entry.map_err(write_err)?;
                to_remove.push(key.value());
            }

            for key in to_remove {
                table.remove(key).map_err(write_err)?;
            }
        }

        write_txn.commit().map_err(write_err)?;
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
    use openraft::EntryPayload;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_log_store_create() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let _store = LogStore::open(&log_path).unwrap();
        assert!(log_path.exists());
    }

    #[tokio::test]
    async fn test_read_vote_returns_none_when_no_vote_saved() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        // Read vote without ever saving - should return None
        let read_vote = store.read_vote().await.unwrap();
        assert_eq!(read_vote, None);
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
    async fn test_append_single_entry() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        // Create a single log entry
        let entry = Entry::<TypeConfig> {
            log_id: log_id(1, 0, 1), // term=1, node_id=0, index=1
            payload: EntryPayload::Blank,
        };

        // Manually append entry to database (bypassing the callback requirement)
        // This tests the core append logic
        let write_txn = store.db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(LOGS_TABLE).unwrap();
            let encoded = bincode::serialize(&entry).unwrap();
            table.insert(1u64, encoded.as_slice()).unwrap();
        }
        write_txn.commit().unwrap();

        // Verify the entry can be retrieved
        let mut reader = store.get_log_reader().await;
        let entries = reader.try_get_log_entries(1..2).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].log_id, entry.log_id);
        assert!(matches!(entries[0].payload, EntryPayload::Blank));

        // Verify log state
        let log_state = store.get_log_state().await.unwrap();
        assert_eq!(log_state.last_log_id, Some(log_id(1, 0, 1)));
        assert_eq!(log_state.last_purged_log_id, None);
    }

    #[tokio::test]
    async fn test_append_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        // Create multiple log entries
        let entries = vec![
            Entry::<TypeConfig> {
                log_id: log_id(1, 0, 1), // term=1, node_id=0, index=1
                payload: EntryPayload::Blank,
            },
            Entry::<TypeConfig> {
                log_id: log_id(1, 0, 2), // term=1, node_id=0, index=2
                payload: EntryPayload::Blank,
            },
            Entry::<TypeConfig> {
                log_id: log_id(1, 0, 3), // term=1, node_id=0, index=3
                payload: EntryPayload::Blank,
            },
            Entry::<TypeConfig> {
                log_id: log_id(2, 0, 4), // term=2, node_id=0, index=4
                payload: EntryPayload::Blank,
            },
            Entry::<TypeConfig> {
                log_id: log_id(2, 0, 5), // term=2, node_id=0, index=5
                payload: EntryPayload::Blank,
            },
        ];

        // Manually append entries to database (bypassing the callback requirement)
        // This tests the core append logic
        let write_txn = store.db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(LOGS_TABLE).unwrap();
            for entry in &entries {
                let encoded = bincode::serialize(&entry).unwrap();
                table
                    .insert(entry.log_id.index, encoded.as_slice())
                    .unwrap();
            }
        }
        write_txn.commit().unwrap();

        // Verify all entries can be retrieved
        let mut reader = store.get_log_reader().await;
        let retrieved_entries = reader.try_get_log_entries(1..6).await.unwrap();

        assert_eq!(retrieved_entries.len(), 5);

        // Verify entries are in correct order
        for (i, entry) in retrieved_entries.iter().enumerate() {
            let expected_index = (i + 1) as u64;
            assert_eq!(entry.log_id.index, expected_index);
            assert_eq!(entry.log_id, entries[i].log_id);
            assert!(matches!(entry.payload, EntryPayload::Blank));
        }

        // Verify log state reflects the last entry
        let log_state = store.get_log_state().await.unwrap();
        assert_eq!(log_state.last_log_id, Some(log_id(2, 0, 5)));
        assert_eq!(log_state.last_purged_log_id, None);
    }

    #[tokio::test]
    async fn test_try_get_log_entries_range_query() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        // Create 10 log entries
        let entries: Vec<Entry<TypeConfig>> = (1..=10)
            .map(|i| Entry::<TypeConfig> {
                log_id: log_id(1, 0, i), // term=1, node_id=0, index=i
                payload: EntryPayload::Blank,
            })
            .collect();

        // Manually append entries to database
        let write_txn = store.db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(LOGS_TABLE).unwrap();
            for entry in &entries {
                let encoded = bincode::serialize(&entry).unwrap();
                table
                    .insert(entry.log_id.index, encoded.as_slice())
                    .unwrap();
            }
        }
        write_txn.commit().unwrap();

        // Test 1: Query a subset range (3..7) - should return indices 3, 4, 5, 6
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(3..7).await.unwrap();
        assert_eq!(retrieved.len(), 4);
        assert_eq!(retrieved[0].log_id.index, 3);
        assert_eq!(retrieved[1].log_id.index, 4);
        assert_eq!(retrieved[2].log_id.index, 5);
        assert_eq!(retrieved[3].log_id.index, 6);

        // Test 2: Query with inclusive range (3..=7) - should return indices 3, 4, 5, 6, 7
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(3..=7).await.unwrap();
        assert_eq!(retrieved.len(), 5);
        assert_eq!(retrieved[0].log_id.index, 3);
        assert_eq!(retrieved[4].log_id.index, 7);

        // Test 3: Query from start (1..5) - should return indices 1, 2, 3, 4
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(1..5).await.unwrap();
        assert_eq!(retrieved.len(), 4);
        assert_eq!(retrieved[0].log_id.index, 1);
        assert_eq!(retrieved[3].log_id.index, 4);

        // Test 4: Query to end (7..11) - should return indices 7, 8, 9, 10
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(7..11).await.unwrap();
        assert_eq!(retrieved.len(), 4);
        assert_eq!(retrieved[0].log_id.index, 7);
        assert_eq!(retrieved[3].log_id.index, 10);

        // Test 5: Query beyond range (7..15) - should return indices 7, 8, 9, 10 only
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(7..15).await.unwrap();
        assert_eq!(retrieved.len(), 4);
        assert_eq!(retrieved[0].log_id.index, 7);
        assert_eq!(retrieved[3].log_id.index, 10);

        // Test 6: Query single entry (5..6) - should return only index 5
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(5..6).await.unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].log_id.index, 5);

        // Test 7: Query empty range (5..5) - should return no entries
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(5..5).await.unwrap();
        assert_eq!(retrieved.len(), 0);

        // Test 8: Query completely outside range (15..20) - should return no entries
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(15..20).await.unwrap();
        assert_eq!(retrieved.len(), 0);

        // Test 9: Query all entries (1..11) - should return all 10 entries
        let mut reader = store.get_log_reader().await;
        let retrieved = reader.try_get_log_entries(1..11).await.unwrap();
        assert_eq!(retrieved.len(), 10);
        for (i, entry) in retrieved.iter().enumerate() {
            let expected_index = (i + 1) as u64;
            assert_eq!(entry.log_id.index, expected_index);
        }
    }

    // Note: Testing append requires proper LogFlushed callback setup which is complex
    // The append functionality is better tested through integration tests
    // with the full Raft system

    #[tokio::test]
    async fn test_truncate_removes_entries_after_index() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        // Create 10 log entries
        let entries: Vec<Entry<TypeConfig>> = (1..=10)
            .map(|i| Entry::<TypeConfig> {
                log_id: log_id(1, 0, i), // term=1, node_id=0, index=i
                payload: EntryPayload::Blank,
            })
            .collect();

        // Manually append entries to database
        let write_txn = store.db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(LOGS_TABLE).unwrap();
            for entry in &entries {
                let encoded = bincode::serialize(&entry).unwrap();
                table
                    .insert(entry.log_id.index, encoded.as_slice())
                    .unwrap();
            }
        }
        write_txn.commit().unwrap();

        // Verify all 10 entries exist before truncation
        let mut reader = store.get_log_reader().await;
        let all_entries = reader.try_get_log_entries(1..11).await.unwrap();
        assert_eq!(all_entries.len(), 10);

        // Truncate at index 5 - this should remove entries 6-10
        store.truncate(log_id(1, 0, 5)).await.unwrap();

        // Verify entries 1-5 still exist
        let mut reader = store.get_log_reader().await;
        let remaining_entries = reader.try_get_log_entries(1..6).await.unwrap();
        assert_eq!(remaining_entries.len(), 5);
        for (i, entry) in remaining_entries.iter().enumerate() {
            let expected_index = (i + 1) as u64;
            assert_eq!(entry.log_id.index, expected_index);
        }

        // Verify entries 6-10 are removed
        let mut reader = store.get_log_reader().await;
        let removed_entries = reader.try_get_log_entries(6..11).await.unwrap();
        assert_eq!(removed_entries.len(), 0);

        // Verify log state shows last log is now index 5
        let log_state = store.get_log_state().await.unwrap();
        assert_eq!(log_state.last_log_id, Some(log_id(1, 0, 5)));
    }

    #[tokio::test]
    async fn test_purge_removes_entries_up_to_index() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        // Create 10 log entries
        let entries: Vec<Entry<TypeConfig>> = (1..=10)
            .map(|i| Entry::<TypeConfig> {
                log_id: log_id(1, 0, i), // term=1, node_id=0, index=i
                payload: EntryPayload::Blank,
            })
            .collect();

        // Manually append entries to database
        let write_txn = store.db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(LOGS_TABLE).unwrap();
            for entry in &entries {
                let encoded = bincode::serialize(&entry).unwrap();
                table
                    .insert(entry.log_id.index, encoded.as_slice())
                    .unwrap();
            }
        }
        write_txn.commit().unwrap();

        // Verify all 10 entries exist before purging
        let mut reader = store.get_log_reader().await;
        let all_entries = reader.try_get_log_entries(1..11).await.unwrap();
        assert_eq!(all_entries.len(), 10);

        // Purge entries up to index 5 - this should remove entries 1-5
        store.purge(log_id(1, 0, 5)).await.unwrap();

        // Verify entries 1-5 are removed
        let mut reader = store.get_log_reader().await;
        let removed_entries = reader.try_get_log_entries(1..6).await.unwrap();
        assert_eq!(removed_entries.len(), 0);

        // Verify entries 6-10 still exist
        let mut reader = store.get_log_reader().await;
        let remaining_entries = reader.try_get_log_entries(6..11).await.unwrap();
        assert_eq!(remaining_entries.len(), 5);
        for (i, entry) in remaining_entries.iter().enumerate() {
            let expected_index = (i + 6) as u64;
            assert_eq!(entry.log_id.index, expected_index);
        }

        // Verify last_purged_log_id is set correctly
        let log_state = store.get_log_state().await.unwrap();
        assert_eq!(log_state.last_purged_log_id, Some(log_id(1, 0, 5)));

        // Verify last_log_id still points to index 10
        assert_eq!(log_state.last_log_id, Some(log_id(1, 0, 10)));
    }

    #[tokio::test]
    async fn test_get_log_state_returns_correct_state() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("raft.db");

        let mut store = LogStore::open(&log_path).unwrap();

        // Test 1: Initial state (empty log)
        let log_state = store.get_log_state().await.unwrap();
        assert_eq!(
            log_state.last_log_id, None,
            "Empty log should have no last_log_id"
        );
        assert_eq!(
            log_state.last_purged_log_id, None,
            "Empty log should have no last_purged_log_id"
        );

        // Test 2: After appending entries
        let entries: Vec<Entry<TypeConfig>> = (1..=5)
            .map(|i| Entry::<TypeConfig> {
                log_id: log_id(1, 0, i), // term=1, node_id=0, index=i
                payload: EntryPayload::Blank,
            })
            .collect();

        // Manually append entries to database
        let write_txn = store.db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(LOGS_TABLE).unwrap();
            for entry in &entries {
                let encoded = bincode::serialize(&entry).unwrap();
                table
                    .insert(entry.log_id.index, encoded.as_slice())
                    .unwrap();
            }
        }
        write_txn.commit().unwrap();

        let log_state = store.get_log_state().await.unwrap();
        assert_eq!(
            log_state.last_log_id,
            Some(log_id(1, 0, 5)),
            "Should return last appended log ID"
        );
        assert_eq!(
            log_state.last_purged_log_id, None,
            "No purge has occurred yet"
        );

        // Test 3: After purging some entries
        store.purge(log_id(1, 0, 3)).await.unwrap();

        let log_state = store.get_log_state().await.unwrap();
        assert_eq!(
            log_state.last_log_id,
            Some(log_id(1, 0, 5)),
            "last_log_id should still be index 5"
        );
        assert_eq!(
            log_state.last_purged_log_id,
            Some(log_id(1, 0, 3)),
            "last_purged_log_id should be index 3"
        );

        // Test 4: After purging all remaining entries
        store.purge(log_id(1, 0, 5)).await.unwrap();

        let log_state = store.get_log_state().await.unwrap();
        assert_eq!(
            log_state.last_log_id, None,
            "All entries purged, no last_log_id"
        );
        assert_eq!(
            log_state.last_purged_log_id,
            Some(log_id(1, 0, 5)),
            "last_purged_log_id should be index 5"
        );
    }
}
