//! Raft consensus protocol implementation.
//!
//! This module implements the core Raft consensus protocol for distributed
//! log replication and leader election. It provides the gRPC service
//! definitions for AppendEntries, RequestVote, and InstallSnapshot RPCs.
//!
//! # Raft Basics
//!
//! Raft is a consensus algorithm that ensures all nodes in a cluster agree
//! on a sequence of log entries. It provides:
//!
//! - **Leader election**: Automatic selection of a leader node
//! - **Log replication**: Reliable replication of operations across nodes
//! - **Safety**: Guarantees that committed entries are never lost
//!
//! For more details, see the [Raft paper](https://raft.github.io/raft.pdf).

use crate::error::ProtocolResult;
use crate::proto;
use std::sync::Arc;
use tonic::{Request, Response, Status, Streaming};

/// Raft service server implementation.
///
/// This service handles core Raft consensus protocol operations including
/// log replication, leader election, and snapshot installation. It wraps
/// a [`RaftEngine`] implementation and exposes it as a gRPC service.
pub struct RaftServiceImpl<R> {
    raft: Arc<R>,
}

impl<R> RaftServiceImpl<R> {
    /// Creates a new RaftServiceImpl wrapping the given Raft engine.
    pub fn new(raft: Arc<R>) -> Self {
        Self { raft }
    }
}

/// Trait that must be implemented by the Raft consensus engine.
///
/// This trait abstracts the actual Raft implementation from the gRPC service
/// layer. Implementors provide the core Raft consensus logic.
#[tonic::async_trait]
pub trait RaftEngine: Send + Sync + 'static {
    /// Handles AppendEntries RPC from the leader.
    ///
    /// This RPC is used both for log replication and as a heartbeat.
    /// Followers use this to stay synchronized with the leader's log.
    async fn append_entries(
        &self,
        request: proto::raft::AppendEntriesRequest,
    ) -> ProtocolResult<proto::raft::AppendEntriesResponse>;

    /// Handles RequestVote RPC from a candidate.
    ///
    /// During leader election, candidates send this RPC to request votes
    /// from other nodes. Nodes vote for at most one candidate per term.
    async fn request_vote(
        &self,
        request: proto::raft::VoteRequest,
    ) -> ProtocolResult<proto::raft::VoteResponse>;

    /// Handles InstallSnapshot RPC from the leader (streaming).
    ///
    /// When a follower is too far behind, the leader sends a snapshot
    /// instead of individual log entries. This is a streaming RPC to
    /// handle large snapshots efficiently.
    async fn install_snapshot(
        &self,
        stream: Streaming<proto::raft::InstallSnapshotRequest>,
    ) -> ProtocolResult<proto::raft::InstallSnapshotResponse>;
}

#[tonic::async_trait]
impl<R: RaftEngine> proto::raft::raft_service_server::RaftService for RaftServiceImpl<R> {
    /// Handle AppendEntries RPC
    ///
    /// This is the core of Raft log replication. Leaders send this to replicate
    /// log entries to followers, and also as a heartbeat when no entries need
    /// to be replicated.
    async fn append_entries(
        &self,
        request: Request<proto::raft::AppendEntriesRequest>,
    ) -> Result<Response<proto::raft::AppendEntriesResponse>, Status> {
        let req = request.into_inner();

        tracing::debug!(
            term = req.term,
            leader_id = req.leader_id.as_ref().map(|id| id.id).unwrap_or(0),
            prev_log_index = req.prev_log_index,
            prev_log_term = req.prev_log_term,
            entries_count = req.entries.len(),
            leader_commit = req.leader_commit,
            "Received AppendEntries request"
        );

        let response = self.raft.append_entries(req).await.map_err(|e| {
            tracing::error!(error = %e, "AppendEntries failed");
            Status::from(e)
        })?;

        tracing::debug!(
            term = response.term,
            success = response.success,
            match_index = response.match_index,
            "Sending AppendEntries response"
        );

        Ok(Response::new(response))
    }

    /// Handle RequestVote RPC
    ///
    /// Candidates send this during leader election to request votes from
    /// other nodes in the cluster.
    async fn vote(
        &self,
        request: Request<proto::raft::VoteRequest>,
    ) -> Result<Response<proto::raft::VoteResponse>, Status> {
        let req = request.into_inner();

        tracing::debug!(
            term = req.term,
            candidate_id = req.candidate_id.as_ref().map(|id| id.id).unwrap_or(0),
            last_log_index = req.last_log_index,
            last_log_term = req.last_log_term,
            "Received Vote request"
        );

        let response = self.raft.request_vote(req).await.map_err(|e| {
            tracing::error!(error = %e, "Vote request failed");
            Status::from(e)
        })?;

        tracing::debug!(
            term = response.term,
            vote_granted = response.vote_granted,
            "Sending Vote response"
        );

        Ok(Response::new(response))
    }

    /// Handle InstallSnapshot RPC (streaming)
    ///
    /// Leaders use this to send snapshots to followers that are too far behind
    /// to catch up via log replication alone.
    async fn install_snapshot(
        &self,
        request: Request<Streaming<proto::raft::InstallSnapshotRequest>>,
    ) -> Result<Response<proto::raft::InstallSnapshotResponse>, Status> {
        let stream = request.into_inner();

        tracing::info!("Starting InstallSnapshot streaming RPC");

        let response = self.raft.install_snapshot(stream).await.map_err(|e| {
            tracing::error!(error = %e, "InstallSnapshot failed");
            Status::from(e)
        })?;

        tracing::info!(term = response.term, "InstallSnapshot completed");

        Ok(Response::new(response))
    }
}

/// Helper functions for working with Raft log entries.
///
/// This module provides type-checking utilities for examining the payload
/// of log entries to determine what kind of command they contain.
pub mod log_entry {
    use crate::proto::raft::log_entry::Payload;
    use crate::proto::raft::LogEntry;

    /// Checks if a log entry is a PutObject command.
    pub fn is_put_object(entry: &LogEntry) -> bool {
        matches!(entry.payload, Some(Payload::PutObject(_)))
    }

    /// Checks if a log entry is a DeleteObject command.
    pub fn is_delete_object(entry: &LogEntry) -> bool {
        matches!(entry.payload, Some(Payload::DeleteObject(_)))
    }

    /// Checks if a log entry is a CreateBucket command.
    pub fn is_create_bucket(entry: &LogEntry) -> bool {
        matches!(entry.payload, Some(Payload::CreateBucket(_)))
    }

    /// Checks if a log entry is a DeleteBucket command.
    pub fn is_delete_bucket(entry: &LogEntry) -> bool {
        matches!(entry.payload, Some(Payload::DeleteBucket(_)))
    }

    /// Checks if a log entry is a MembershipChange command.
    pub fn is_membership_change(entry: &LogEntry) -> bool {
        matches!(entry.payload, Some(Payload::Membership(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProtocolError;
    use crate::proto::common;
    use crate::proto::raft::log_entry::Payload;
    use crate::proto::raft::raft_service_server::RaftService;
    use crate::proto::raft::*;

    struct MockRaftEngine {
        current_term: u64,
    }

    #[tonic::async_trait]
    impl RaftEngine for MockRaftEngine {
        async fn append_entries(
            &self,
            request: AppendEntriesRequest,
        ) -> ProtocolResult<AppendEntriesResponse> {
            Ok(AppendEntriesResponse {
                term: self.current_term,
                success: request.term >= self.current_term,
                match_index: request.prev_log_index,
            })
        }

        async fn request_vote(&self, request: VoteRequest) -> ProtocolResult<VoteResponse> {
            Ok(VoteResponse {
                term: self.current_term,
                vote_granted: request.term > self.current_term,
            })
        }

        async fn install_snapshot(
            &self,
            mut stream: Streaming<InstallSnapshotRequest>,
        ) -> ProtocolResult<InstallSnapshotResponse> {
            // Consume the stream
            while let Some(_chunk) = stream
                .message()
                .await
                .map_err(|e| ProtocolError::Internal(format!("Stream error: {}", e)))?
            {
                // Process chunk
            }

            Ok(InstallSnapshotResponse {
                term: self.current_term,
            })
        }
    }

    #[tokio::test]
    async fn test_append_entries_success() {
        let engine = Arc::new(MockRaftEngine { current_term: 5 });
        let service = RaftServiceImpl::new(engine);

        let request = Request::new(AppendEntriesRequest {
            term: 5,
            leader_id: Some(common::NodeId { id: 1 }),
            prev_log_index: 10,
            prev_log_term: 4,
            entries: vec![],
            leader_commit: 10,
        });

        let response = service.append_entries(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.term, 5);
        assert!(resp.success);
        assert_eq!(resp.match_index, 10);
    }

    #[tokio::test]
    async fn test_append_entries_stale_term() {
        let engine = Arc::new(MockRaftEngine { current_term: 10 });
        let service = RaftServiceImpl::new(engine);

        let request = Request::new(AppendEntriesRequest {
            term: 5,
            leader_id: Some(common::NodeId { id: 1 }),
            prev_log_index: 10,
            prev_log_term: 4,
            entries: vec![],
            leader_commit: 10,
        });

        let response = service.append_entries(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.term, 10);
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_vote_granted() {
        let engine = Arc::new(MockRaftEngine { current_term: 5 });
        let service = RaftServiceImpl::new(engine);

        let request = Request::new(VoteRequest {
            term: 6,
            candidate_id: Some(common::NodeId { id: 2 }),
            last_log_index: 10,
            last_log_term: 5,
        });

        let response = service.vote(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.term, 5);
        assert!(resp.vote_granted);
    }

    #[tokio::test]
    async fn test_vote_rejected_stale_term() {
        let engine = Arc::new(MockRaftEngine { current_term: 10 });
        let service = RaftServiceImpl::new(engine);

        let request = Request::new(VoteRequest {
            term: 5,
            candidate_id: Some(common::NodeId { id: 2 }),
            last_log_index: 10,
            last_log_term: 5,
        });

        let response = service.vote(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.term, 10);
        assert!(!resp.vote_granted);
    }

    #[test]
    fn test_log_entry_helpers() {
        let put_entry = LogEntry {
            term: 1,
            index: 1,
            payload: Some(Payload::PutObject(PutObjectCommand { manifest: None })),
        };
        assert!(super::log_entry::is_put_object(&put_entry));
        assert!(!super::log_entry::is_delete_object(&put_entry));

        let delete_entry = LogEntry {
            term: 1,
            index: 2,
            payload: Some(Payload::DeleteObject(DeleteObjectCommand { key: None })),
        };
        assert!(super::log_entry::is_delete_object(&delete_entry));
        assert!(!super::log_entry::is_put_object(&delete_entry));
    }
}
