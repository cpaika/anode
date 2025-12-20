use thiserror::Error;
use tonic::Status;

/// Protocol-level errors for inter-node communication
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// Invalid or malformed message
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// Node not found in the cluster
    #[error("Node not found: {0}")]
    NodeNotFound(u64),

    /// Current node is not the leader
    #[error("Not leader")]
    NotLeader { leader: Option<u64> },

    /// Chunk not found
    #[error("Chunk not found: {0}")]
    ChunkNotFound(String),

    /// Object not found
    #[error("Object not found: {0}")]
    ObjectNotFound(String),

    /// Bucket not found
    #[error("Bucket not found: {0}")]
    BucketNotFound(String),

    /// Bucket already exists
    #[error("Bucket already exists: {0}")]
    BucketAlreadyExists(String),

    /// Checksum mismatch
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Insufficient replicas
    #[error("Insufficient replicas: required {required}, available {available}")]
    InsufficientReplicas { required: u32, available: u32 },

    /// Timeout
    #[error("Operation timed out")]
    Timeout,

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Raft error
    #[error("Raft error: {0}")]
    Raft(String),

    /// Term mismatch
    #[error("Term mismatch: current {current}, received {received}")]
    TermMismatch { current: u64, received: u64 },

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Unauthorized
    #[error("Unauthorized")]
    Unauthorized,

    /// Resource exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Already exists
    #[error("Already exists: {0}")]
    AlreadyExists(String),

    /// Conflict
    #[error("Conflict: {0}")]
    Conflict(String),
}

impl From<ProtocolError> for Status {
    fn from(err: ProtocolError) -> Self {
        match err {
            ProtocolError::InvalidMessage(msg) => {
                Status::invalid_argument(format!("Invalid message: {}", msg))
            }
            ProtocolError::NodeNotFound(id) => {
                Status::not_found(format!("Node not found: {}", id))
            }
            ProtocolError::NotLeader { leader } => {
                let msg = match leader {
                    Some(leader_id) => format!("Not leader. Leader: {}", leader_id),
                    None => "Not leader. Leader unknown".to_string(),
                };
                Status::failed_precondition(msg)
            }
            ProtocolError::ChunkNotFound(id) => {
                Status::not_found(format!("Chunk not found: {}", id))
            }
            ProtocolError::ObjectNotFound(key) => {
                Status::not_found(format!("Object not found: {}", key))
            }
            ProtocolError::BucketNotFound(name) => {
                Status::not_found(format!("Bucket not found: {}", name))
            }
            ProtocolError::BucketAlreadyExists(name) => {
                Status::already_exists(format!("Bucket already exists: {}", name))
            }
            ProtocolError::ChecksumMismatch { expected, actual } => Status::data_loss(format!(
                "Checksum mismatch: expected {}, got {}",
                expected, actual
            )),
            ProtocolError::Io(e) => Status::internal(format!("I/O error: {}", e)),
            ProtocolError::Serialization(msg) => {
                Status::internal(format!("Serialization error: {}", msg))
            }
            ProtocolError::Deserialization(msg) => {
                Status::invalid_argument(format!("Deserialization error: {}", msg))
            }
            ProtocolError::Storage(msg) => Status::internal(format!("Storage error: {}", msg)),
            ProtocolError::InsufficientReplicas {
                required,
                available,
            } => Status::unavailable(format!(
                "Insufficient replicas: required {}, available {}",
                required, available
            )),
            ProtocolError::Timeout => Status::deadline_exceeded("Operation timed out"),
            ProtocolError::Internal(msg) => Status::internal(format!("Internal error: {}", msg)),
            ProtocolError::Raft(msg) => Status::internal(format!("Raft error: {}", msg)),
            ProtocolError::TermMismatch { current, received } => Status::failed_precondition(
                format!("Term mismatch: current {}, received {}", current, received),
            ),
            ProtocolError::InvalidState(msg) => {
                Status::failed_precondition(format!("Invalid state: {}", msg))
            }
            ProtocolError::Unauthorized => Status::permission_denied("Unauthorized"),
            ProtocolError::ResourceExhausted(msg) => {
                Status::resource_exhausted(format!("Resource exhausted: {}", msg))
            }
            ProtocolError::AlreadyExists(msg) => {
                Status::already_exists(format!("Already exists: {}", msg))
            }
            ProtocolError::Conflict(msg) => Status::aborted(format!("Conflict: {}", msg)),
        }
    }
}

/// Result type for protocol operations
pub type ProtocolResult<T> = Result<T, ProtocolError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_to_status_not_leader() {
        let err = ProtocolError::NotLeader {
            leader: Some(42),
        };
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
        assert!(status.message().contains("Leader: 42"));
    }

    #[test]
    fn test_error_to_status_not_found() {
        let err = ProtocolError::NodeNotFound(123);
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::NotFound);
        assert!(status.message().contains("123"));
    }

    #[test]
    fn test_error_to_status_checksum_mismatch() {
        let err = ProtocolError::ChecksumMismatch {
            expected: "abc123".to_string(),
            actual: "def456".to_string(),
        };
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::DataLoss);
        assert!(status.message().contains("abc123"));
        assert!(status.message().contains("def456"));
    }

    #[test]
    fn test_error_to_status_invalid_message() {
        let err = ProtocolError::InvalidMessage("bad proto".to_string());
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn test_error_to_status_timeout() {
        let err = ProtocolError::Timeout;
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
    }
}
