use anode_protocol::{ChunkId, ObjectKey, ObjectManifest};
use anode_raft::{
    create_raft_node, BucketConfig, Command, LogStore, RaftConfig, RaftNodeBuilder, Request,
    StateMachine,
};
use anode_storage::{StorageConfig, StorageEngine};
use openraft::storage::{RaftLogReader, RaftLogStorage, RaftLogStorageExt, RaftStateMachine};
use openraft::{testing::log_id, Entry, EntryPayload, RaftSnapshotBuilder};
use std::sync::Arc;
use tempfile::TempDir;

/// Test creating a Raft node with all components
#[tokio::test]
async fn test_create_complete_raft_node() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().join("raft");

    let storage_config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(StorageEngine::open(storage_config).await.unwrap());

    let result = create_raft_node(1, "test-cluster", data_dir, storage).await;
    assert!(result.is_ok());

    let raft = result.unwrap();
    // Raft node created successfully
}

/// Test the full workflow: config -> storage -> state machine
#[tokio::test]
#[ignore = "TODO: Fix integration test - needs investigation"]
async fn test_full_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().join("raft");

    // 1. Create and validate configuration
    let config = RaftConfig::new(1, "integration-test", data_dir.clone());
    assert!(config.validate().is_ok());

    // 2. Create log store
    std::fs::create_dir_all(config.log_dir()).unwrap();
    let mut log_store = LogStore::open(&config.log_dir().join("raft.db")).unwrap();

    // 3. Create state machine
    let storage_config = StorageConfig::new(data_dir.clone());
    let storage = Arc::new(StorageEngine::open(storage_config).await.unwrap());
    let mut state_machine = StateMachine::new(storage, config.snapshot_dir());

    // 4. Test log operations
    let entries = vec![
        Entry {
            log_id: log_id(1, 1, 1),
            payload: EntryPayload::Normal(Request {
                client_id: "test-client".to_string(),
                serial: 1,
                command: Command::CreateBucket {
                    name: "test-bucket".to_string(),
                    config: BucketConfig {
                        name: "test-bucket".to_string(),
                        replication_factor: 3,
                        created_at: 12345,
                    },
                },
            }),
        },
        Entry {
            log_id: log_id(1, 1, 2),
            payload: EntryPayload::Normal(Request {
                client_id: "test-client".to_string(),
                serial: 2,
                command: Command::PutObject {
                    bucket: "test-bucket".to_string(),
                    key: "test-key".to_string(),
                    manifest: ObjectManifest::new(
                        ObjectKey::new("test-bucket", "test-key"),
                        1024,
                        [0u8; 32],
                        vec![ChunkId::from_hash(&[1u8; 32]).unwrap()],
                    ),
                },
            }),
        },
    ];

    // Append to log
    log_store.blocking_append(entries.clone()).await.unwrap();

    // Read back from log
    let read_entries = log_store.try_get_log_entries(1..=2).await.unwrap();
    assert_eq!(read_entries.len(), 2);

    // 5. Apply entries to state machine
    let responses = state_machine.apply(entries).await.unwrap();
    assert_eq!(responses.len(), 2);
    assert!(responses[0].success); // CreateBucket
    assert!(responses[1].success); // PutObject

    // 6. Verify state machine state
    let (last_applied, _) = state_machine.applied_state().await.unwrap();
    assert_eq!(last_applied, Some(log_id(1, 1, 2)));
}

/// Test snapshot creation and restoration
#[tokio::test]
#[ignore = "TODO: Fix integration test - needs investigation"]
async fn test_snapshot_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().join("raft");

    let storage_config = StorageConfig::new(data_dir.clone());
    let storage = Arc::new(StorageEngine::open(storage_config).await.unwrap());
    let mut state_machine = StateMachine::new(storage, temp_dir.path().join("snapshots"));

    // Apply some entries to build state
    let entries = vec![
        Entry {
            log_id: log_id(1, 1, 1),
            payload: EntryPayload::Normal(Request {
                client_id: "test".to_string(),
                serial: 1,
                command: Command::CreateBucket {
                    name: "bucket1".to_string(),
                    config: BucketConfig {
                        name: "bucket1".to_string(),
                        replication_factor: 3,
                        created_at: 12345,
                    },
                },
            }),
        },
        Entry {
            log_id: log_id(1, 1, 2),
            payload: EntryPayload::Normal(Request {
                client_id: "test".to_string(),
                serial: 2,
                command: Command::CreateBucket {
                    name: "bucket2".to_string(),
                    config: BucketConfig {
                        name: "bucket2".to_string(),
                        replication_factor: 3,
                        created_at: 12345,
                    },
                },
            }),
        },
    ];

    state_machine.apply(entries).await.unwrap();

    // Create snapshot
    let mut builder = state_machine.get_snapshot_builder().await;
    let snapshot = builder.build_snapshot().await.unwrap();

    assert_eq!(snapshot.meta.last_log_id, Some(log_id(1, 1, 2)));

    // Create new state machine and restore from snapshot
    let storage_config2 = StorageConfig::new(data_dir.clone());
    let storage2 = Arc::new(StorageEngine::open(storage_config2).await.unwrap());
    let mut state_machine2 = StateMachine::new(storage2, temp_dir.path().join("snapshots2"));

    state_machine2
        .install_snapshot(&snapshot.meta, snapshot.snapshot)
        .await
        .unwrap();

    // Verify restored state
    let (last_applied, _) = state_machine2.applied_state().await.unwrap();
    assert_eq!(last_applied, Some(log_id(1, 1, 2)));
}

/// Test builder pattern
#[tokio::test]
async fn test_builder_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().join("raft");

    let storage_config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(StorageEngine::open(storage_config).await.unwrap());

    let config = RaftConfig::new(1, "builder-test", data_dir);

    let result = RaftNodeBuilder::new(config)
        .storage_engine(storage)
        .build()
        .await;

    assert!(result.is_ok());
}

/// Test log store operations: append, truncate, purge
#[tokio::test]
#[ignore = "TODO: Fix integration test - needs investigation"]
async fn test_log_store_operations() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.db");

    let mut store = LogStore::open(&log_path).unwrap();

    // Create test entries
    let entries: Vec<Entry<anode_raft::TypeConfig>> = (1..=10)
        .map(|i| Entry {
            log_id: log_id(1, 1, i),
            payload: EntryPayload::Blank,
        })
        .collect();

    // Append entries
    store.blocking_append(entries.clone()).await.unwrap();

    // Verify all entries exist
    let all_entries = store.try_get_log_entries(1..=10).await.unwrap();
    assert_eq!(all_entries.len(), 10);

    // Truncate after index 7
    store.truncate(log_id(1, 1, 7)).await.unwrap();
    let after_truncate = store.try_get_log_entries(1..=10).await.unwrap();
    assert_eq!(after_truncate.len(), 7);

    // Purge up to index 3
    store.purge(log_id(1, 1, 3)).await.unwrap();
    let after_purge = store.try_get_log_entries(1..=10).await.unwrap();
    assert_eq!(after_purge.len(), 4); // Entries 4-7 remain
    assert_eq!(after_purge[0].log_id.index, 4);
}

/// Test configuration validation
#[test]
fn test_config_validation() {
    // Valid config
    let mut config = RaftConfig::default();
    assert!(config.validate().is_ok());

    // Invalid: node_id = 0
    config.node_id = 0;
    assert!(config.validate().is_err());
    config.node_id = 1;

    // Invalid: empty cluster name
    config.cluster_name = String::new();
    assert!(config.validate().is_err());
    config.cluster_name = "test".to_string();

    // Invalid: election timeout range
    config.election_timeout_min_ms = 300;
    config.election_timeout_max_ms = 150;
    assert!(config.validate().is_err());
    config.election_timeout_min_ms = 150;
    config.election_timeout_max_ms = 300;

    // Invalid: heartbeat >= election timeout
    config.heartbeat_interval_ms = 200;
    assert!(config.validate().is_err());
}

/// Test state machine command execution
#[tokio::test]
async fn test_state_machine_commands() {
    let temp_dir = TempDir::new().unwrap();
    let storage_config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(StorageEngine::open(storage_config).await.unwrap());
    let mut sm = StateMachine::new(storage, temp_dir.path().join("snapshots"));

    // Test CreateBucket
    let create_bucket = Entry {
        log_id: log_id(1, 1, 1),
        payload: EntryPayload::Normal(Request {
            client_id: "test".to_string(),
            serial: 1,
            command: Command::CreateBucket {
                name: "my-bucket".to_string(),
                config: BucketConfig {
                    name: "my-bucket".to_string(),
                    replication_factor: 3,
                    created_at: 12345,
                },
            },
        }),
    };

    let responses = sm.apply(vec![create_bucket]).await.unwrap();
    assert!(responses[0].success);

    // Test duplicate CreateBucket (should fail)
    let duplicate = Entry {
        log_id: log_id(1, 1, 2),
        payload: EntryPayload::Normal(Request {
            client_id: "test".to_string(),
            serial: 2,
            command: Command::CreateBucket {
                name: "my-bucket".to_string(),
                config: BucketConfig {
                    name: "my-bucket".to_string(),
                    replication_factor: 3,
                    created_at: 12345,
                },
            },
        }),
    };

    let responses = sm.apply(vec![duplicate]).await.unwrap();
    assert!(!responses[0].success);

    // Test DeleteBucket
    let delete = Entry {
        log_id: log_id(1, 1, 3),
        payload: EntryPayload::Normal(Request {
            client_id: "test".to_string(),
            serial: 3,
            command: Command::DeleteBucket {
                name: "my-bucket".to_string(),
            },
        }),
    };

    let responses = sm.apply(vec![delete]).await.unwrap();
    assert!(responses[0].success);
}
