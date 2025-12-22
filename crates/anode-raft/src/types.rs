use anode_protocol::ObjectManifest;
use openraft::BasicNode;
use serde::{Deserialize, Serialize};

/// Node identifier type used throughout the Raft system.
///
/// This is a simple `u64` value that uniquely identifies each node in the cluster.
/// Node IDs must be non-zero and unique within a cluster.
pub type NodeId = u64;

/// Request wrapper for commands submitted to the Raft state machine.
///
/// Each request includes a unique client identifier and serial number for
/// deduplication and ordering guarantees. The actual operation is specified
/// in the [`Command`] field.
///
/// # Examples
///
/// ```
/// use anode_raft::{Request, Command, BucketConfig};
///
/// let request = Request {
///     client_id: "client-1".to_string(),
///     serial: 42,
///     command: Command::CreateBucket {
///         name: "my-bucket".to_string(),
///         config: BucketConfig {
///             name: "my-bucket".to_string(),
///             replication_factor: 3,
///             created_at: 1234567890,
///         },
///     },
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique identifier for the client submitting this request
    pub client_id: String,

    /// Serial number for request ordering and deduplication
    pub serial: u64,

    /// The command to execute on the state machine
    pub command: Command,
}

/// Commands that can be replicated through the Raft consensus protocol.
///
/// These commands represent all state-changing operations in the Anode storage system.
/// Each command is replicated to the Raft log and applied to the state machine once
/// committed by the cluster.
///
/// # Variants
///
/// - `PutObject` - Store an object in a bucket using its manifest
/// - `DeleteObject` - Remove an object from a bucket
/// - `CreateBucket` - Create a new bucket with the given configuration
/// - `DeleteBucket` - Remove a bucket and all its contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Store an object in a bucket.
    ///
    /// The manifest contains all metadata needed to reconstruct the object,
    /// including chunk IDs, size, and hash.
    PutObject {
        /// Name of the bucket to store the object in
        bucket: String,
        /// Key (name) of the object within the bucket
        key: String,
        /// Object manifest containing metadata and chunk references
        manifest: ObjectManifest,
    },

    /// Delete an object from a bucket.
    DeleteObject {
        /// Name of the bucket containing the object
        bucket: String,
        /// Key (name) of the object to delete
        key: String,
    },

    /// Create a new bucket.
    ///
    /// The bucket configuration specifies replication settings and other metadata.
    CreateBucket {
        /// Name of the bucket to create
        name: String,
        /// Configuration for the new bucket
        config: BucketConfig,
    },

    /// Delete a bucket.
    ///
    /// This operation will fail if the bucket is not empty.
    DeleteBucket {
        /// Name of the bucket to delete
        name: String,
    },
}

/// Configuration for a storage bucket.
///
/// Buckets are the top-level organizational unit in Anode. Each bucket
/// has its own replication settings and metadata.
///
/// # Examples
///
/// ```
/// use anode_raft::BucketConfig;
///
/// let config = BucketConfig {
///     name: "my-bucket".to_string(),
///     replication_factor: 3,
///     created_at: 1234567890,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BucketConfig {
    /// Name of the bucket (must be unique within the cluster)
    pub name: String,

    /// Number of replicas to maintain for objects in this bucket
    pub replication_factor: u32,

    /// Unix timestamp (seconds since epoch) when the bucket was created
    pub created_at: i64,
}

/// Response from a Raft state machine operation.
///
/// Responses indicate whether an operation succeeded and may optionally
/// include data returned by the operation.
///
/// # Examples
///
/// ```
/// use anode_raft::Response;
///
/// // Successful operation with no data
/// let success = Response::success();
/// assert!(success.success);
///
/// // Failed operation with error message
/// let error = Response::error("Bucket not found");
/// assert!(!error.success);
/// assert_eq!(error.error, Some("Bucket not found".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Whether the operation completed successfully
    pub success: bool,

    /// Error message if the operation failed
    pub error: Option<String>,

    /// Optional data returned by the operation
    pub data: Option<ResponseData>,
}

/// Optional data returned in a [`Response`].
///
/// Different operations may return different types of data. Currently,
/// only object manifests are supported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseData {
    /// An object manifest returned by a PutObject operation
    ObjectManifest(ObjectManifest),
}

/// OpenRaft type configuration for Anode.
///
/// This type implements [`openraft::RaftTypeConfig`] and defines all the concrete
/// types used by the Raft implementation, including request/response types,
/// node identifiers, and the async runtime.
///
/// # Type Parameters
///
/// - `D` (Data) - [`Request`] - Commands submitted to the state machine
/// - `R` (Response) - [`Response`] - Responses from the state machine
/// - `NodeId` - [`NodeId`] - Node identifier type (u64)
/// - `Node` - [`BasicNode`] - Node information (address, etc.)
/// - `Entry` - [`openraft::Entry<TypeConfig>`] - Log entry type
/// - `SnapshotData` - `Cursor<Vec<u8>>` - Snapshot data format
/// - `AsyncRuntime` - [`openraft::TokioRuntime`] - Tokio-based async runtime
/// - `Responder` - [`openraft::impls::OneshotResponder`] - Response channel type
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
    type Responder = openraft::impls::OneshotResponder<TypeConfig>;
}

/// Type alias for the complete Raft instance used in Anode.
///
/// This is a fully configured [`openraft::Raft`] instance using Anode's
/// [`TypeConfig`]. It provides all Raft consensus functionality including
/// leader election, log replication, and membership changes.
pub type AnodeRaft = openraft::Raft<TypeConfig>;

impl Response {
    /// Create a successful response with no data.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::Response;
    ///
    /// let response = Response::success();
    /// assert!(response.success);
    /// assert!(response.error.is_none());
    /// assert!(response.data.is_none());
    /// ```
    pub fn success() -> Self {
        Self {
            success: true,
            error: None,
            data: None,
        }
    }

    /// Create a successful response with associated data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anode_raft::{Response, ResponseData};
    /// use anode_protocol::{ObjectManifest, ObjectKey, ChunkId};
    ///
    /// let chunk_id = ChunkId::from_hash(&[0u8; 32]).unwrap();
    /// let manifest = ObjectManifest::new(
    ///     ObjectKey::new("bucket", "key"),
    ///     100,
    ///     [0u8; 32],
    ///     vec![chunk_id],
    /// );
    ///
    /// let response = Response::success_with_data(
    ///     ResponseData::ObjectManifest(manifest)
    /// );
    /// assert!(response.success);
    /// assert!(response.data.is_some());
    /// ```
    pub fn success_with_data(data: ResponseData) -> Self {
        Self {
            success: true,
            error: None,
            data: Some(data),
        }
    }

    /// Create an error response with a message.
    ///
    /// # Examples
    ///
    /// ```
    /// use anode_raft::Response;
    ///
    /// let response = Response::error("Bucket not found");
    /// assert!(!response.success);
    /// assert_eq!(response.error, Some("Bucket not found".to_string()));
    /// ```
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
        use anode_protocol::{ChunkId, ObjectKey};

        let chunk_id = ChunkId::from_hash(&[0u8; 32]).expect("Failed to create ChunkId");
        let manifest = ObjectManifest::new(
            ObjectKey::new("bucket", "key"),
            100,
            [0u8; 32],
            vec![chunk_id],
        );

        let cmd = Command::PutObject {
            bucket: "bucket".to_string(),
            key: "key".to_string(),
            manifest: manifest.clone(),
        };

        let serialized = bincode::serialize(&cmd).unwrap();
        let deserialized: Command = bincode::deserialize(&serialized).unwrap();

        // Check that serialization worked by comparing bucket and key
        match deserialized {
            Command::PutObject {
                bucket,
                key,
                manifest: m,
            } => {
                assert_eq!(bucket, "bucket");
                assert_eq!(key, "key");
                assert_eq!(m.key.bucket, "bucket");
                assert_eq!(m.key.key, "key");
            },
            _ => panic!("Expected PutObject command"),
        }
    }

    #[test]
    fn test_type_config_satisfies_raft_type_config() {
        // This function will only compile if TypeConfig properly implements RaftTypeConfig
        // with all the necessary trait bounds satisfied
        fn assert_raft_type_config<T: openraft::RaftTypeConfig>() {}

        // If this compiles, TypeConfig satisfies all RaftTypeConfig trait bounds
        assert_raft_type_config::<TypeConfig>();

        // Also verify that we can create the AnodeRaft type alias
        // This ensures TypeConfig is usable in the actual Raft context
        fn assert_raft_instantiable<T: openraft::RaftTypeConfig>() {
            // This is a compile-time check - the function body doesn't need to do anything
            let _type_check: Option<openraft::Raft<T>> = None;
        }

        assert_raft_instantiable::<TypeConfig>();
    }
}
