use openraft::{error::NetworkError, error::RPCError, error::RemoteError, BasicNode, StorageError};
use thiserror::Error;

use crate::types::NodeId;

/// Raft-specific errors for Anode
#[derive(Debug, Error)]
pub enum RaftError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError<NodeId>),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Not leader, current leader: {leader:?}")]
    NotLeader { leader: Option<NodeId> },

    #[error("Timeout waiting for operation")]
    Timeout,

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Node {0} not found in cluster")]
    NodeNotFound(NodeId),

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
            RPCError::NodeNotFound(node) => RaftError::NodeNotFound(node.node_id),
            RPCError::Timeout(_) => RaftError::Timeout,
            RPCError::Network(net_err) => RaftError::Network(net_err.to_string()),
            RPCError::RemoteError(remote) => {
                RaftError::Rpc(format!("Remote error: {:?}", remote.source))
            }
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
            source: openraft::error::StorageIOError::read(&e),
        })
    }
}

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
