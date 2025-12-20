pub mod common;
pub mod raft;
pub mod cluster;
pub mod replication;
pub mod error;

// Re-export common types
pub use common::{ChunkId, ClusterId, NodeId, ObjectKey, ObjectManifest};
pub use error::{ProtocolError, ProtocolResult};

// Include generated protobuf code
pub mod proto {
    pub mod common {
        tonic::include_proto!("anode.common");
    }

    pub mod raft {
        tonic::include_proto!("anode.raft");
    }

    pub mod cluster {
        tonic::include_proto!("anode.cluster");
    }

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
