//! Anode Protocol Library
//!
//! This crate defines the wire protocols and data types for communication between
//! nodes in the Anode distributed object storage cluster. It provides:
//!
//! - **Common types**: Node IDs, chunk IDs, object keys, and manifests
//! - **Error handling**: Protocol-level error types and conversions
//! - **Cluster management**: Services for node join/leave and cluster status
//! - **Raft consensus**: Implementation of the Raft consensus protocol
//! - **Replication**: Chunk replication and verification services
//!
//! # Protocol Buffers
//!
//! The core protocol messages are defined using Protocol Buffers (protobuf) and
//! compiled at build time. The generated code is available in the [`proto`] module.
//!
//! # Example
//!
//! ```rust,no_run
//! use anode_protocol::{NodeId, ObjectKey, ChunkId};
//!
//! // Create a node ID
//! let node = NodeId(1);
//!
//! // Create an object key
//! let key = ObjectKey::new("my-bucket", "path/to/object");
//!
//! // Work with chunk IDs
//! let chunk_id = ChunkId([0u8; 32]);
//! let hex = chunk_id.to_hex();
//! ```

pub mod cluster;
pub mod common;
pub mod error;
pub mod raft;
pub mod replication;

// Re-export common types
pub use common::{ChunkId, ClusterId, NodeId, ObjectKey, ObjectManifest};
pub use error::{ProtocolError, ProtocolResult};

/// Generated Protocol Buffer definitions for inter-node communication.
///
/// This module contains auto-generated code from `.proto` files that define
/// the wire format for all gRPC services and messages.
pub mod proto {
    /// Common protocol buffer types shared across all services.
    pub mod common {
        tonic::include_proto!("anode.common");
    }

    /// Raft consensus protocol messages.
    pub mod raft {
        tonic::include_proto!("anode.raft");
    }

    /// Cluster management protocol messages.
    pub mod cluster {
        tonic::include_proto!("anode.cluster");
    }

    /// Chunk replication protocol messages.
    pub mod replication {
        tonic::include_proto!("anode.replication");
    }
}

// Re-export generated service traits and clients
pub use proto::cluster::{
    cluster_service_client::ClusterServiceClient, cluster_service_server::ClusterService,
    cluster_service_server::ClusterServiceServer,
};
pub use proto::raft::{
    raft_service_client::RaftServiceClient, raft_service_server::RaftService,
    raft_service_server::RaftServiceServer,
};
pub use proto::replication::{
    replication_service_client::ReplicationServiceClient,
    replication_service_server::ReplicationService,
    replication_service_server::ReplicationServiceServer,
};
