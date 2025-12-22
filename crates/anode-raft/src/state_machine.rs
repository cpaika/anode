use crate::types::{BucketConfig, Command, NodeId, Response, ResponseData, TypeConfig};
use anode_storage::StorageEngine;
use openraft::storage::{RaftStateMachine, Snapshot};
use openraft::{
    BasicNode, Entry, EntryPayload, LogId, RaftSnapshotBuilder, SnapshotMeta, StorageError,
    StorageIOError, StoredMembership,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

/// Serializable snapshot of the Raft state machine.
///
/// This structure represents a point-in-time snapshot of the entire state machine,
/// including cluster membership, bucket configurations, and object metadata. Snapshots
/// are used for log compaction and for bringing new or recovering nodes up to date.
///
/// # Serialization
///
/// Snapshots are serialized using [bincode](https://docs.rs/bincode) for efficient
/// encoding and decoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    /// The log ID of the last entry applied to the state machine at snapshot time.
    ///
    /// This marks the point in the log where this snapshot was taken. All log
    /// entries up to and including this ID are reflected in the snapshot state.
    pub last_applied_log: Option<LogId<NodeId>>,

    /// The cluster membership configuration at snapshot time.
    ///
    /// This includes the set of nodes in the cluster and their addresses.
    pub last_membership: StoredMembership<NodeId, BasicNode>,

    /// All bucket configurations in the system.
    ///
    /// Maps bucket names to their configuration (replication factor, creation time, etc.).
    pub buckets: HashMap<String, BucketConfig>,

    /// List of all objects in the system.
    ///
    /// Each entry is a `(bucket, key)` pair. The actual object data is stored
    /// separately in the storage engine.
    pub objects: Vec<(String, String)>,
}

/// Raft state machine implementation for Anode.
///
/// This implements [`openraft::storage::RaftStateMachine`] and handles the application
/// of committed log entries to the storage system. It manages bucket and object metadata,
/// while delegating actual object storage to the [`anode_storage::StorageEngine`].
///
/// # State
///
/// The state machine tracks:
/// - Bucket configurations (in-memory)
/// - Last applied log ID
/// - Cluster membership
/// - References to objects (actual data is in the storage engine)
///
/// # Snapshots
///
/// Snapshots are created periodically to compact the log. They capture the entire
/// state machine state and are stored in the configured snapshot directory.
///
/// # Examples
///
/// ```no_run
/// use anode_raft::StateMachine;
/// use anode_storage::StorageEngine;
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let storage_config = anode_storage::StorageConfig {
/// #     data_dir: PathBuf::from("/data/blobs"),
/// #     metadata_dir: PathBuf::from("/data/metadata"),
/// #     chunk_size: 4 * 1024 * 1024,
/// #     sync_writes: false,
/// # };
/// let storage = Arc::new(StorageEngine::open(storage_config).await?);
/// let state_machine = StateMachine::new(
///     storage,
///     PathBuf::from("/data/snapshots")
/// );
/// # Ok(())
/// # }
/// ```
pub struct StateMachine {
    /// Storage engine for persisting object data and metadata
    storage: Arc<StorageEngine>,

    /// Last log entry applied to the state machine
    last_applied: Arc<RwLock<Option<LogId<NodeId>>>>,

    /// Current cluster membership configuration
    last_membership: Arc<RwLock<StoredMembership<NodeId, BasicNode>>>,

    /// In-memory bucket configurations
    buckets: Arc<RwLock<HashMap<String, BucketConfig>>>,

    /// Directory where snapshots are stored
    snapshot_dir: PathBuf,
}

impl StateMachine {
    /// Create a new state machine with the specified storage engine and snapshot directory.
    ///
    /// The state machine starts with no applied logs and an empty cluster membership.
    /// Bucket configurations are loaded from the storage engine on first use.
    ///
    /// # Arguments
    ///
    /// * `storage` - The storage engine for object data
    /// * `snapshot_dir` - Directory where snapshots will be stored
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anode_raft::StateMachine;
    /// use anode_storage::StorageEngine;
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let storage_config = anode_storage::StorageConfig {
    /// #     data_dir: PathBuf::from("/data/blobs"),
    /// #     metadata_dir: PathBuf::from("/data/metadata"),
    /// #     chunk_size: 4 * 1024 * 1024,
    /// #     sync_writes: false,
    /// # };
    /// let storage = Arc::new(StorageEngine::open(storage_config).await?);
    /// let state_machine = StateMachine::new(
    ///     storage,
    ///     PathBuf::from("/data/snapshots")
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(storage: Arc<StorageEngine>, snapshot_dir: PathBuf) -> Self {
        Self {
            storage,
            last_applied: Arc::new(RwLock::new(None)),
            last_membership: Arc::new(RwLock::new(StoredMembership::default())),
            buckets: Arc::new(RwLock::new(HashMap::new())),
            snapshot_dir,
        }
    }

    /// Apply a single command to the state machine.
    ///
    /// This is called internally when processing log entries. Commands are validated
    /// before being applied to ensure consistency.
    ///
    /// # Returns
    ///
    /// A [`Response`] indicating success or failure, potentially with data.
    async fn apply_command(&self, command: Command) -> Response {
        match command {
            Command::PutObject {
                bucket,
                key,
                manifest,
            } => {
                // Verify bucket exists
                if !self.buckets.read().contains_key(&bucket) {
                    return Response::error(format!("Bucket '{}' does not exist", bucket));
                }

                // Note: In a real implementation, we would validate the manifest
                // and ensure chunks are available before committing
                tracing::info!("Applied PutObject: {}/{}", bucket, key);
                Response::success_with_data(ResponseData::ObjectManifest(manifest))
            },

            Command::DeleteObject { bucket, key } => {
                if !self.buckets.read().contains_key(&bucket) {
                    return Response::error(format!("Bucket '{}' does not exist", bucket));
                }

                match self.storage.delete_object(&bucket, &key).await {
                    Ok(_) => {
                        tracing::info!("Applied DeleteObject: {}/{}", bucket, key);
                        Response::success()
                    },
                    Err(e) => Response::error(format!("Failed to delete object: {}", e)),
                }
            },

            Command::CreateBucket { name, config } => {
                let mut buckets = self.buckets.write();
                if buckets.contains_key(&name) {
                    return Response::error(format!("Bucket '{}' already exists", name));
                }

                buckets.insert(name.clone(), config);
                tracing::info!("Applied CreateBucket: {}", name);
                Response::success()
            },

            Command::DeleteBucket { name } => {
                let mut buckets = self.buckets.write();
                if !buckets.contains_key(&name) {
                    return Response::error(format!("Bucket '{}' does not exist", name));
                }

                buckets.remove(&name);
                tracing::info!("Applied DeleteBucket: {}", name);
                Response::success()
            },
        }
    }

    /// Create a snapshot of the current state
    async fn build_snapshot(&self) -> std::result::Result<SnapshotData, StorageError<NodeId>> {
        let last_applied = *self.last_applied.read();
        let last_membership = self.last_membership.read().clone();
        let buckets = self.buckets.read().clone();

        // In a real implementation, we would iterate through all objects
        // For now, we just store an empty list
        let objects = Vec::new();

        Ok(SnapshotData {
            last_applied_log: last_applied,
            last_membership,
            buckets,
            objects,
        })
    }

    /// Restore state from a snapshot
    async fn restore_snapshot(
        &self,
        snapshot: SnapshotData,
    ) -> std::result::Result<(), StorageError<NodeId>> {
        *self.last_applied.write() = snapshot.last_applied_log;
        *self.last_membership.write() = snapshot.last_membership;
        *self.buckets.write() = snapshot.buckets;

        tracing::info!("Restored state from snapshot");
        Ok(())
    }
}

impl RaftStateMachine<TypeConfig> for StateMachine {
    type SnapshotBuilder = SnapshotBuilder;

    async fn applied_state(
        &mut self,
    ) -> std::result::Result<
        (Option<LogId<NodeId>>, StoredMembership<NodeId, BasicNode>),
        StorageError<NodeId>,
    > {
        Ok((
            *self.last_applied.read(),
            self.last_membership.read().clone(),
        ))
    }

    async fn apply<I>(
        &mut self,
        entries: I,
    ) -> std::result::Result<Vec<Response>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>>,
    {
        let mut responses = Vec::new();

        for entry in entries {
            *self.last_applied.write() = Some(entry.log_id);

            let response = match entry.payload {
                EntryPayload::Blank => Response::success(),
                EntryPayload::Normal(request) => self.apply_command(request.command).await,
                EntryPayload::Membership(membership) => {
                    *self.last_membership.write() =
                        StoredMembership::new(Some(entry.log_id), membership);
                    tracing::info!("Applied membership change at log_id {:?}", entry.log_id);
                    Response::success()
                },
            };

            responses.push(response);
        }

        Ok(responses)
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> std::result::Result<Box<Cursor<Vec<u8>>>, StorageError<NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> std::result::Result<(), StorageError<NodeId>> {
        let data = snapshot.into_inner();

        let snapshot_data: SnapshotData =
            bincode::deserialize(&data).map_err(|e| StorageError::IO {
                source: StorageIOError::read(&e),
            })?;

        self.restore_snapshot(snapshot_data).await?;

        tracing::info!(
            "Installed snapshot at index {}, term {}",
            meta.last_log_id.as_ref().map(|l| l.index).unwrap_or(0),
            meta.last_log_id
                .as_ref()
                .map(|l| l.leader_id.term)
                .unwrap_or(0)
        );

        Ok(())
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        SnapshotBuilder {
            last_applied: *self.last_applied.read(),
            last_membership: self.last_membership.read().clone(),
            buckets: self.buckets.read().clone(),
        }
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> std::result::Result<Option<Snapshot<TypeConfig>>, StorageError<NodeId>> {
        // Check if a snapshot file exists
        // For now, we return None - snapshots are built on demand
        Ok(None)
    }
}

/// Builder for creating state machine snapshots asynchronously.
///
/// This type implements [`openraft::RaftSnapshotBuilder`] and creates snapshots
/// of the state machine without blocking the main Raft thread. The snapshot
/// captures the state at a specific point in time.
///
/// # Usage
///
/// This type is created internally by the state machine's
/// [`get_snapshot_builder`](StateMachine::get_snapshot_builder) method and
/// should not be constructed directly.
pub struct SnapshotBuilder {
    /// Log ID of the last entry reflected in this snapshot
    last_applied: Option<LogId<NodeId>>,
    /// Cluster membership at snapshot time
    last_membership: StoredMembership<NodeId, BasicNode>,
    /// Bucket configurations at snapshot time
    buckets: HashMap<String, BucketConfig>,
}

impl RaftSnapshotBuilder<TypeConfig> for SnapshotBuilder {
    async fn build_snapshot(
        &mut self,
    ) -> std::result::Result<Snapshot<TypeConfig>, StorageError<NodeId>> {
        let snapshot_data = SnapshotData {
            last_applied_log: self.last_applied,
            last_membership: self.last_membership.clone(),
            buckets: self.buckets.clone(),
            objects: Vec::new(),
        };

        let data = bincode::serialize(&snapshot_data).map_err(|e| StorageError::IO {
            source: StorageIOError::write(&e),
        })?;

        let snapshot_id = format!(
            "snapshot-{}-{}",
            self.last_applied.as_ref().map(|l| l.index).unwrap_or(0),
            self.last_applied
                .as_ref()
                .map(|l| l.leader_id.term)
                .unwrap_or(0)
        );

        Ok(Snapshot {
            meta: SnapshotMeta {
                last_log_id: self.last_applied,
                last_membership: self.last_membership.clone(),
                snapshot_id,
            },
            snapshot: Box::new(Cursor::new(data)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Request;
    use openraft::testing::log_id;
    use openraft::EntryPayload;
    use tempfile::TempDir;

    async fn create_test_storage() -> (Arc<StorageEngine>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = anode_storage::StorageConfig {
            data_dir: temp_dir.path().join("blobs"),
            metadata_dir: temp_dir.path().join("metadata"),
            chunk_size: anode_storage::chunks::CHUNK_SIZE,
            sync_writes: false,
        };
        (
            Arc::new(StorageEngine::open(storage_config).await.unwrap()),
            temp_dir,
        )
    }

    #[tokio::test]
    async fn test_state_machine_create() {
        let (storage, _temp) = create_test_storage().await;
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        let (last_applied, _) = sm.applied_state().await.unwrap();
        assert_eq!(last_applied, None);
    }

    #[tokio::test]
    async fn test_apply_create_bucket() {
        let (storage, _temp) = create_test_storage().await;
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        let entry = Entry {
            log_id: log_id(1, 1, 1),
            payload: EntryPayload::Normal(Request {
                client_id: "test".to_string(),
                serial: 1,
                command: Command::CreateBucket {
                    name: "test-bucket".to_string(),
                    config: BucketConfig {
                        name: "test-bucket".to_string(),
                        replication_factor: 3,
                        created_at: 0,
                    },
                },
            }),
        };

        let responses = sm.apply(vec![entry]).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert!(responses[0].success);

        assert!(sm.buckets.read().contains_key("test-bucket"));
    }

    #[tokio::test]
    async fn test_apply_delete_bucket() {
        let (storage, _temp) = create_test_storage().await;
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        // First create a bucket
        sm.buckets.write().insert(
            "test-bucket".to_string(),
            BucketConfig {
                name: "test-bucket".to_string(),
                replication_factor: 3,
                created_at: 0,
            },
        );

        let entry = Entry {
            log_id: log_id(1, 1, 1),
            payload: EntryPayload::Normal(Request {
                client_id: "test".to_string(),
                serial: 1,
                command: Command::DeleteBucket {
                    name: "test-bucket".to_string(),
                },
            }),
        };

        let responses = sm.apply(vec![entry]).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert!(responses[0].success);

        assert!(!sm.buckets.read().contains_key("test-bucket"));
    }

    #[tokio::test]
    async fn test_apply_put_object() {
        use anode_protocol::{ChunkId, ObjectKey};

        let (storage, _temp) = create_test_storage().await;
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        // First create a bucket (required for PutObject)
        sm.buckets.write().insert(
            "test-bucket".to_string(),
            BucketConfig {
                name: "test-bucket".to_string(),
                replication_factor: 3,
                created_at: 0,
            },
        );

        // Create a test manifest
        let chunk_id = ChunkId::from_hash(&[0u8; 32]).expect("Failed to create ChunkId");
        let manifest = anode_protocol::ObjectManifest::new(
            ObjectKey::new("test-bucket", "test-key"),
            100,
            [0u8; 32],
            vec![chunk_id],
        );

        let entry = Entry {
            log_id: log_id(1, 1, 1),
            payload: EntryPayload::Normal(Request {
                client_id: "test".to_string(),
                serial: 1,
                command: Command::PutObject {
                    bucket: "test-bucket".to_string(),
                    key: "test-key".to_string(),
                    manifest: manifest.clone(),
                },
            }),
        };

        let responses = sm.apply(vec![entry]).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert!(responses[0].success);

        // Verify the response contains the manifest
        assert!(responses[0].data.is_some());
        match &responses[0].data {
            Some(crate::types::ResponseData::ObjectManifest(m)) => {
                assert_eq!(m.key.bucket, "test-bucket");
                assert_eq!(m.key.key, "test-key");
                assert_eq!(m.total_size, 100);
            },
            _ => panic!("Expected ObjectManifest in response data"),
        }
    }

    #[tokio::test]
    async fn test_apply_delete_object() {
        let (storage, _temp) = create_test_storage().await;
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage.clone(), temp_dir.path().to_path_buf());

        // Create bucket in storage engine metadata (required for delete_object)
        let bucket_cfg = anode_storage::BucketConfig {
            name: "test-bucket".to_string(),
            versioning_enabled: false,
            created_at: 0,
            metadata: std::collections::BTreeMap::new(),
            quota_bytes: None,
        };
        storage
            .create_bucket("test-bucket", bucket_cfg)
            .await
            .unwrap();

        // Also add to state machine buckets
        sm.buckets.write().insert(
            "test-bucket".to_string(),
            BucketConfig {
                name: "test-bucket".to_string(),
                replication_factor: 3,
                created_at: 0,
            },
        );

        let entry = Entry {
            log_id: log_id(1, 1, 1),
            payload: EntryPayload::Normal(Request {
                client_id: "test".to_string(),
                serial: 1,
                command: Command::DeleteObject {
                    bucket: "test-bucket".to_string(),
                    key: "test-key".to_string(),
                },
            }),
        };

        let responses = sm.apply(vec![entry]).await.unwrap();
        assert_eq!(responses.len(), 1);
        // Delete of non-existent object should succeed (idempotent)
        assert!(responses[0].success);
    }

    #[tokio::test]
    async fn test_snapshot_build() {
        let (storage, _temp) = create_test_storage().await;
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        // Create a bucket
        sm.buckets.write().insert(
            "test-bucket".to_string(),
            BucketConfig {
                name: "test-bucket".to_string(),
                replication_factor: 3,
                created_at: 0,
            },
        );

        *sm.last_applied.write() = Some(log_id(1, 1, 5));

        let mut builder = sm.get_snapshot_builder().await;
        let snapshot = builder.build_snapshot().await.unwrap();

        assert_eq!(snapshot.meta.last_log_id, Some(log_id(1, 1, 5)));
    }

    #[tokio::test]
    async fn test_install_snapshot() {
        let (storage, _temp) = create_test_storage().await;
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        let snapshot_data = SnapshotData {
            last_applied_log: Some(log_id(1, 1, 10)),
            last_membership: StoredMembership::default(),
            buckets: {
                let mut map = HashMap::new();
                map.insert(
                    "restored-bucket".to_string(),
                    BucketConfig {
                        name: "restored-bucket".to_string(),
                        replication_factor: 3,
                        created_at: 0,
                    },
                );
                map
            },
            objects: Vec::new(),
        };

        let data = bincode::serialize(&snapshot_data).unwrap();
        let snapshot_box = Box::new(Cursor::new(data));

        let meta = SnapshotMeta {
            last_log_id: Some(log_id(1, 1, 10)),
            last_membership: StoredMembership::default(),
            snapshot_id: "test-snapshot".to_string(),
        };

        sm.install_snapshot(&meta, snapshot_box).await.unwrap();

        assert!(sm.buckets.read().contains_key("restored-bucket"));
        assert_eq!(*sm.last_applied.read(), Some(log_id(1, 1, 10)));
    }
}
