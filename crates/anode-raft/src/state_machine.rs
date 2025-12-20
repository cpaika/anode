use crate::error::Result;
use crate::types::{
    BucketConfig, Command, NodeId, Request, Response, ResponseData, TypeConfig,
};
use anode_protocol::ObjectKey;
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

/// Snapshot data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    /// Last applied log ID
    pub last_applied_log: Option<LogId<NodeId>>,

    /// Last membership configuration
    pub last_membership: StoredMembership<NodeId, BasicNode>,

    /// Bucket configurations
    pub buckets: HashMap<String, BucketConfig>,

    /// Object metadata snapshot
    pub objects: Vec<(String, String)>, // (bucket, key) pairs
}

/// State machine for Anode
pub struct StateMachine {
    /// Storage engine for persisting objects
    storage: Arc<StorageEngine>,

    /// Last applied log entry
    last_applied: Arc<RwLock<Option<LogId<NodeId>>>>,

    /// Last membership configuration
    last_membership: Arc<RwLock<StoredMembership<NodeId, BasicNode>>>,

    /// Bucket configurations in memory
    buckets: Arc<RwLock<HashMap<String, BucketConfig>>>,

    /// Snapshot directory
    snapshot_dir: PathBuf,
}

impl StateMachine {
    /// Create a new state machine
    pub fn new(storage: Arc<StorageEngine>, snapshot_dir: PathBuf) -> Self {
        Self {
            storage,
            last_applied: Arc::new(RwLock::new(None)),
            last_membership: Arc::new(RwLock::new(StoredMembership::default())),
            buckets: Arc::new(RwLock::new(HashMap::new())),
            snapshot_dir,
        }
    }

    /// Apply a single command to the state machine
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
            }

            Command::DeleteObject { bucket, key } => {
                if !self.buckets.read().contains_key(&bucket) {
                    return Response::error(format!("Bucket '{}' does not exist", bucket));
                }

                let obj_key = ObjectKey::new(&bucket, &key);
                match self.storage.delete_object(&obj_key).await {
                    Ok(_) => {
                        tracing::info!("Applied DeleteObject: {}/{}", bucket, key);
                        Response::success()
                    }
                    Err(e) => Response::error(format!("Failed to delete object: {}", e)),
                }
            }

            Command::CreateBucket { name, config } => {
                let mut buckets = self.buckets.write();
                if buckets.contains_key(&name) {
                    return Response::error(format!("Bucket '{}' already exists", name));
                }

                buckets.insert(name.clone(), config);
                tracing::info!("Applied CreateBucket: {}", name);
                Response::success()
            }

            Command::DeleteBucket { name } => {
                let mut buckets = self.buckets.write();
                if !buckets.contains_key(&name) {
                    return Response::error(format!("Bucket '{}' does not exist", name));
                }

                buckets.remove(&name);
                tracing::info!("Applied DeleteBucket: {}", name);
                Response::success()
            }
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

#[async_trait::async_trait]
impl RaftStateMachine<TypeConfig> for StateMachine {
    type SnapshotBuilder = SnapshotBuilder;

    async fn applied_state(
        &mut self,
    ) -> std::result::Result<
        (
            Option<LogId<NodeId>>,
            StoredMembership<NodeId, BasicNode>,
        ),
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
        I: IntoIterator<Item = Entry<TypeConfig>> + Send,
        I::IntoIter: Send,
    {
        let mut responses = Vec::new();

        for entry in entries {
            *self.last_applied.write() = Some(entry.log_id);

            let response = match entry.payload {
                EntryPayload::Blank => Response::success(),
                EntryPayload::Normal(request) => self.apply_command(request.command).await,
                EntryPayload::Membership(membership) => {
                    *self.last_membership.write() = membership.clone();
                    tracing::info!("Applied membership change: {:?}", membership);
                    Response::success()
                }
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

        let snapshot_data: SnapshotData = bincode::deserialize(&data).map_err(|e| {
            StorageError::IO {
                source: StorageIOError::read(&e),
            }
        })?;

        self.restore_snapshot(snapshot_data).await?;

        tracing::info!(
            "Installed snapshot at index {}, term {}",
            meta.last_log_id.as_ref().map(|l| l.index).unwrap_or(0),
            meta.last_log_id.as_ref().map(|l| l.leader_id.term).unwrap_or(0)
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

/// Snapshot builder for creating snapshots asynchronously
pub struct SnapshotBuilder {
    last_applied: Option<LogId<NodeId>>,
    last_membership: StoredMembership<NodeId, BasicNode>,
    buckets: HashMap<String, BucketConfig>,
}

#[async_trait::async_trait]
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
    use anode_protocol::{ChunkId, ObjectManifest};
    use openraft::testing::log_id;
    use tempfile::TempDir;

    fn create_test_storage() -> Arc<StorageEngine> {
        let temp_dir = TempDir::new().unwrap();
        Arc::new(StorageEngine::new(temp_dir.path().to_path_buf()).unwrap())
    }

    #[tokio::test]
    async fn test_state_machine_create() {
        let storage = create_test_storage();
        let temp_dir = TempDir::new().unwrap();
        let sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        let (last_applied, _) = sm.applied_state().await.unwrap();
        assert_eq!(last_applied, None);
    }

    #[tokio::test]
    async fn test_apply_create_bucket() {
        let storage = create_test_storage();
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
        let storage = create_test_storage();
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        // First create a bucket
        sm.buckets
            .write()
            .insert(
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
    async fn test_snapshot_build() {
        let storage = create_test_storage();
        let temp_dir = TempDir::new().unwrap();
        let mut sm = StateMachine::new(storage, temp_dir.path().to_path_buf());

        // Create a bucket
        sm.buckets
            .write()
            .insert(
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

        assert_eq!(
            snapshot.meta.last_log_id,
            Some(log_id(1, 1, 5))
        );
    }

    #[tokio::test]
    async fn test_install_snapshot() {
        let storage = create_test_storage();
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
