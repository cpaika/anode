use openraft::{
    error::{NetworkError, RPCError},
    AnyError, BasicNode, ErrorSubject, ErrorVerb, StorageError, StorageIOError,
};
use thiserror::Error;

use crate::types::NodeId;

/// Errors that can occur in the Raft consensus layer.
///
/// This enum encompasses all error types that can occur during Raft operations,
/// including storage errors, network errors, and protocol-level errors.
///
/// # Error Categories
///
/// - **Storage** - Errors from the persistent log or state machine storage
/// - **Network** - Network communication failures
/// - **RPC** - Remote procedure call errors
/// - **NotLeader** - Attempt to perform a write on a non-leader node
/// - **Configuration** - Invalid Raft configuration
/// - **Fatal** - Unrecoverable errors that require node restart
#[derive(Debug, Error)]
pub enum RaftError {
    /// Error occurred in the underlying storage layer.
    ///
    /// This includes errors from the log store or state machine storage.
    /// Storage errors are typically fatal and may require operator intervention.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError<NodeId>),

    /// Network communication error.
    ///
    /// Indicates a failure to communicate with another node in the cluster.
    /// These errors are often transient and may resolve when connectivity is restored.
    #[error("Network error: {0}")]
    Network(String),

    /// Attempted to perform a write operation on a non-leader node.
    ///
    /// Write operations must be sent to the cluster leader. This error includes
    /// the current leader's node ID if known, allowing clients to redirect their request.
    #[error("Not leader, current leader: {leader:?}")]
    NotLeader {
        /// The current leader's node ID, if known
        leader: Option<NodeId>,
    },

    /// Operation timed out before completing.
    ///
    /// This can occur when waiting for consensus or when a network operation
    /// takes too long to complete.
    #[error("Timeout waiting for operation")]
    Timeout,

    /// Error during a remote procedure call.
    ///
    /// This includes errors from the RPC layer, such as payload size issues
    /// or remote node errors.
    #[error("RPC error: {0}")]
    Rpc(String),

    /// Serialization or deserialization error.
    ///
    /// Occurs when encoding or decoding Raft messages, log entries, or snapshots.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Database error from the embedded redb storage.
    ///
    /// These errors indicate problems with the persistent log database,
    /// such as corruption or disk full conditions.
    #[error("Database error: {0}")]
    Database(String),

    /// I/O error from file system operations.
    ///
    /// These can occur when reading or writing log files, snapshots, or
    /// creating directories.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid Raft configuration.
    ///
    /// The provided configuration is invalid or inconsistent. Use the
    /// [`RaftConfig::validate`](crate::RaftConfig::validate) method to check
    /// configuration before use.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Referenced node was not found in the cluster membership.
    ///
    /// The specified node ID is not part of the current cluster configuration.
    #[error("Node {0} not found in cluster")]
    NodeNotFound(NodeId),

    /// Fatal error that requires node restart.
    ///
    /// These errors indicate a serious problem that cannot be recovered from
    /// without restarting the node or operator intervention.
    #[error("Fatal error: {0}")]
    Fatal(String),
}

impl From<bincode::Error> for RaftError {
    fn from(e: bincode::Error) -> Self {
        RaftError::Serialization(e.to_string())
    }
}

impl From<redb::Error> for RaftError {
    fn from(e: redb::Error) -> Self {
        RaftError::Database(e.to_string())
    }
}

impl From<redb::DatabaseError> for RaftError {
    fn from(e: redb::DatabaseError) -> Self {
        RaftError::Database(e.to_string())
    }
}

impl From<redb::TransactionError> for RaftError {
    fn from(e: redb::TransactionError) -> Self {
        RaftError::Database(e.to_string())
    }
}

impl From<redb::TableError> for RaftError {
    fn from(e: redb::TableError) -> Self {
        RaftError::Database(e.to_string())
    }
}

impl From<redb::CommitError> for RaftError {
    fn from(e: redb::CommitError) -> Self {
        RaftError::Database(e.to_string())
    }
}

impl From<redb::StorageError> for RaftError {
    fn from(e: redb::StorageError) -> Self {
        RaftError::Database(e.to_string())
    }
}

impl<E: std::error::Error> From<RPCError<NodeId, BasicNode, E>> for RaftError {
    fn from(e: RPCError<NodeId, BasicNode, E>) -> Self {
        match e {
            RPCError::Timeout(timeout) => RaftError::Rpc(format!(
                "Timeout sending {} to node {}",
                timeout.action, timeout.target
            )),
            RPCError::Unreachable(unreachable) => {
                RaftError::Network(format!("Node unreachable: {}", unreachable))
            },
            RPCError::PayloadTooLarge(payload) => {
                RaftError::Rpc(format!("Payload too large: {}", payload))
            },
            RPCError::Network(net_err) => RaftError::Network(net_err.to_string()),
            RPCError::RemoteError(remote) => RaftError::Rpc(format!(
                "Remote error from node {}: {:?}",
                remote.target, remote.source
            )),
        }
    }
}

impl From<NetworkError> for RaftError {
    fn from(e: NetworkError) -> Self {
        RaftError::Network(e.to_string())
    }
}

impl From<anode_storage::StorageError> for RaftError {
    fn from(e: anode_storage::StorageError) -> Self {
        RaftError::Storage(StorageError::IO {
            source: StorageIOError::new(ErrorSubject::Store, ErrorVerb::Read, AnyError::new(&e)),
        })
    }
}

/// Convenience type alias for `Result<T, RaftError>`.
///
/// This is the standard result type used throughout the anode-raft crate.
pub type Result<T> = std::result::Result<T, RaftError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = RaftError::NotLeader { leader: Some(1) };
        assert!(err.to_string().contains("Not leader"));

        let err = RaftError::Timeout;
        assert_eq!(err.to_string(), "Timeout waiting for operation");

        let err = RaftError::Network("connection failed".to_string());
        assert_eq!(err.to_string(), "Network error: connection failed");
    }

    #[test]
    fn test_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let raft_err: RaftError = io_err.into();
        assert!(matches!(raft_err, RaftError::Io(_)));
    }
}
