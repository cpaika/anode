use anode_protocol::{ObjectKey, ObjectManifest};
use openraft::BasicNode;
use serde::{Deserialize, Serialize};

pub type NodeId = u64;

/// Request types for the Raft state machine
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Request {
    pub client_id: String,
    pub serial: u64,
    pub command: Command,
}

/// Commands that can be replicated through Raft
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Command {
    PutObject {
        bucket: String,
        key: String,
        manifest: ObjectManifest,
    },
    DeleteObject {
        bucket: String,
        key: String,
    },
    CreateBucket {
        name: String,
        config: BucketConfig,
    },
    DeleteBucket {
        name: String,
    },
}

/// Bucket configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BucketConfig {
    pub name: String,
    pub replication_factor: u32,
    pub created_at: i64,
}

/// Response from the state machine
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Response {
    pub success: bool,
    pub error: Option<String>,
    pub data: Option<ResponseData>,
}

/// Optional response data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResponseData {
    ObjectManifest(ObjectManifest),
}

/// OpenRaft type configuration for Anode
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct TypeConfig;

impl openraft::RaftTypeConfig for TypeConfig {
    type D = Request;
    type R = Response;
    type NodeId = NodeId;
    type Node = BasicNode;
    type Entry = openraft::Entry<TypeConfig>;
    type SnapshotData = std::io::Cursor<Vec<u8>>;
    type AsyncRuntime = openraft::TokioRuntime;
}

/// Type alias for the Raft instance
pub type AnodeRaft = openraft::Raft<TypeConfig>;

impl Response {
    pub fn success() -> Self {
        Self {
            success: true,
            error: None,
            data: None,
        }
    }

    pub fn success_with_data(data: ResponseData) -> Self {
        Self {
            success: true,
            error: None,
            data: Some(data),
        }
    }

    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(error.into()),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_creation() {
        let success = Response::success();
        assert!(success.success);
        assert!(success.error.is_none());

        let error = Response::error("test error");
        assert!(!error.success);
        assert_eq!(error.error, Some("test error".to_string()));
    }

    #[test]
    fn test_command_serialization() {
        use anode_protocol::ChunkId;

        let manifest = ObjectManifest {
            object_key: ObjectKey::new("bucket", "key"),
            size: 100,
            chunks: vec![ChunkId::from_hash(b"test")],
            content_type: None,
            metadata: Default::default(),
            created_at: 0,
            modified_at: 0,
            etag: String::new(),
        };

        let cmd = Command::PutObject {
            bucket: "bucket".to_string(),
            key: "key".to_string(),
            manifest,
        };

        let serialized = bincode::serialize(&cmd).unwrap();
        let deserialized: Command = bincode::deserialize(&serialized).unwrap();
        assert_eq!(cmd, deserialized);
    }
}
