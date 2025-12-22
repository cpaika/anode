//! Chunk replication protocol implementation.
//!
//! This module implements the gRPC services for replicating chunk data between
//! nodes in the cluster. It provides streaming APIs for efficient transfer of
//! large chunks and verification of chunk integrity.
//!
//! # Replication Flow
//!
//! 1. **Replicate**: Leader streams chunks to followers
//! 2. **Verify**: Nodes verify chunk existence and integrity
//! 3. **Fetch**: Nodes can fetch missing chunks from peers
//! 4. **List**: Nodes can list available chunks for synchronization

use crate::error::ProtocolResult;
use crate::proto;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

/// Type alias for streaming responses.
type ResponseStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;

/// Replication service server implementation.
///
/// This service handles chunk replication between nodes, including streaming
/// chunk data, verifying chunk existence, and listing chunks. It requires
/// both a [`BlobStore`] for chunk data and a [`ChunkManager`] for metadata.
pub struct ReplicationServiceImpl<B, C> {
    blob_store: Arc<B>,
    chunk_manager: Arc<C>,
}

impl<B, C> ReplicationServiceImpl<B, C> {
    /// Creates a new ReplicationServiceImpl with the given blob store and chunk manager.
    pub fn new(blob_store: Arc<B>, chunk_manager: Arc<C>) -> Self {
        Self {
            blob_store,
            chunk_manager,
        }
    }
}

/// Trait that must be implemented by the blob store.
///
/// The blob store is responsible for the physical storage and retrieval
/// of chunk data on disk or other storage backends.
#[tonic::async_trait]
pub trait BlobStore: Send + Sync + 'static {
    /// Stores chunk data received from replication.
    ///
    /// The chunk ID is a 32-byte SHA-256 hash that uniquely identifies
    /// the chunk. Implementations should verify the hash matches the data.
    async fn store_chunk(&self, chunk_id: &[u8; 32], data: Vec<u8>) -> ProtocolResult<()>;

    /// Fetches chunk data for replication.
    ///
    /// Returns the raw bytes of the chunk. Returns an error if the chunk
    /// does not exist.
    async fn fetch_chunk(&self, chunk_id: &[u8; 32]) -> ProtocolResult<Vec<u8>>;
}

/// Trait that must be implemented by the chunk manager.
///
/// The chunk manager maintains metadata about chunks, including their
/// existence, validity, and reference counts.
#[tonic::async_trait]
pub trait ChunkManager: Send + Sync + 'static {
    /// Verifies that a chunk exists and is valid.
    ///
    /// Returns metadata about the chunk including whether it exists,
    /// is valid (checksum matches), and its size.
    async fn verify_chunk(
        &self,
        chunk_id: &[u8; 32],
    ) -> ProtocolResult<proto::replication::VerifyChunkResponse>;

    /// Lists chunks on this node.
    ///
    /// Returns a paginated list of chunks. Use `start_after` to continue
    /// from a previous query, and `limit` to control page size.
    async fn list_chunks(
        &self,
        start_after: Option<Vec<u8>>,
        limit: u32,
    ) -> ProtocolResult<Vec<proto::replication::ChunkInfo>>;
}

#[tonic::async_trait]
impl<B: BlobStore, C: ChunkManager>
    proto::replication::replication_service_server::ReplicationService
    for ReplicationServiceImpl<B, C>
{
    type ReplicateChunksStream = ResponseStream<proto::replication::ReplicateChunkResponse>;

    /// Handle bidirectional streaming chunk replication
    ///
    /// The client streams chunks to be replicated, and the server streams back
    /// acknowledgments for each chunk received.
    async fn replicate_chunks(
        &self,
        request: Request<Streaming<proto::replication::ReplicateChunkRequest>>,
    ) -> Result<Response<Self::ReplicateChunksStream>, Status> {
        let mut stream = request.into_inner();
        let blob_store = Arc::clone(&self.blob_store);

        tracing::info!("Starting ReplicateChunks bidirectional stream");

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let mut current_chunk_id: Option<Vec<u8>> = None;
            let mut current_data: Vec<u8> = Vec::new();

            while let Some(result) = stream.next().await {
                match result {
                    Ok(req) => {
                        let chunk_id = req
                            .chunk_id
                            .as_ref()
                            .map(|c| c.hash.clone())
                            .unwrap_or_default();

                        tracing::debug!(
                            chunk_id = %hex::encode(&chunk_id),
                            data_size = req.data.len(),
                            is_last = req.is_last,
                            "Received chunk replication request"
                        );

                        // Accumulate chunk data
                        if current_chunk_id.as_ref() != Some(&chunk_id) {
                            current_chunk_id = Some(chunk_id.clone());
                            current_data.clear();
                        }

                        current_data.extend_from_slice(&req.data);

                        // If this is the last piece, store the chunk
                        if req.is_last {
                            let chunk_hash = if chunk_id.len() == 32 {
                                let mut hash = [0u8; 32];
                                hash.copy_from_slice(&chunk_id);
                                hash
                            } else {
                                tracing::error!("Invalid chunk ID length");
                                let _ = tx
                                    .send(Err(Status::invalid_argument("Invalid chunk ID")))
                                    .await;
                                continue;
                            };

                            match blob_store
                                .store_chunk(&chunk_hash, current_data.clone())
                                .await
                            {
                                Ok(_) => {
                                    tracing::info!(
                                        chunk_id = %hex::encode(&chunk_id),
                                        size = current_data.len(),
                                        "Chunk replicated successfully"
                                    );

                                    let response = proto::replication::ReplicateChunkResponse {
                                        success: true,
                                        error: String::new(),
                                        chunk_id: req.chunk_id,
                                    };

                                    if tx.send(Ok(response)).await.is_err() {
                                        tracing::error!("Failed to send response");
                                        break;
                                    }
                                },
                                Err(e) => {
                                    tracing::error!(
                                        chunk_id = %hex::encode(&chunk_id),
                                        error = %e,
                                        "Failed to store chunk"
                                    );

                                    let response = proto::replication::ReplicateChunkResponse {
                                        success: false,
                                        error: e.to_string(),
                                        chunk_id: req.chunk_id,
                                    };

                                    if tx.send(Ok(response)).await.is_err() {
                                        tracing::error!("Failed to send error response");
                                        break;
                                    }
                                },
                            }

                            current_data.clear();
                        }
                    },
                    Err(e) => {
                        tracing::error!(error = %e, "Stream error");
                        let _ = tx.send(Err(e)).await;
                        break;
                    },
                }
            }

            tracing::info!("ReplicateChunks stream completed");
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::ReplicateChunksStream
        ))
    }

    /// Verify that a chunk exists and is valid
    ///
    /// Returns whether the chunk exists, is valid, and its size.
    async fn verify_chunk(
        &self,
        request: Request<proto::replication::VerifyChunkRequest>,
    ) -> Result<Response<proto::replication::VerifyChunkResponse>, Status> {
        let req = request.into_inner();

        let chunk_id = req
            .chunk_id
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing chunk ID"))?;

        tracing::debug!(
            chunk_id = %hex::encode(&chunk_id.hash),
            "Received VerifyChunk request"
        );

        let chunk_hash = if chunk_id.hash.len() == 32 {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&chunk_id.hash);
            hash
        } else {
            return Err(Status::invalid_argument("Invalid chunk ID length"));
        };

        let response = self
            .chunk_manager
            .verify_chunk(&chunk_hash)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "VerifyChunk failed");
                Status::from(e)
            })?;

        tracing::debug!(
            chunk_id = %hex::encode(&chunk_id.hash),
            exists = response.exists,
            valid = response.valid,
            size = response.size,
            "Sending VerifyChunk response"
        );

        Ok(Response::new(response))
    }

    type FetchChunkStream = ResponseStream<proto::replication::FetchChunkResponse>;

    /// Fetch chunk data (streaming)
    ///
    /// Streams the chunk data in pieces to the client.
    async fn fetch_chunk(
        &self,
        request: Request<proto::replication::FetchChunkRequest>,
    ) -> Result<Response<Self::FetchChunkStream>, Status> {
        let req = request.into_inner();

        let chunk_id = req
            .chunk_id
            .ok_or_else(|| Status::invalid_argument("Missing chunk ID"))?;

        tracing::info!(
            chunk_id = %hex::encode(&chunk_id.hash),
            "Received FetchChunk request"
        );

        let chunk_hash = if chunk_id.hash.len() == 32 {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&chunk_id.hash);
            hash
        } else {
            return Err(Status::invalid_argument("Invalid chunk ID length"));
        };

        let blob_store = Arc::clone(&self.blob_store);

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            match blob_store.fetch_chunk(&chunk_hash).await {
                Ok(data) => {
                    tracing::info!(
                        chunk_id = %hex::encode(chunk_hash),
                        size = data.len(),
                        "Fetched chunk data"
                    );

                    // Stream data in 64KB chunks
                    const CHUNK_SIZE: usize = 64 * 1024;
                    let total_chunks = data.len().div_ceil(CHUNK_SIZE);

                    for (i, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
                        let is_last = i == total_chunks - 1;

                        let response = proto::replication::FetchChunkResponse {
                            data: chunk.to_vec(),
                            is_last,
                        };

                        if tx.send(Ok(response)).await.is_err() {
                            tracing::error!("Failed to send chunk data");
                            break;
                        }
                    }
                },
                Err(e) => {
                    tracing::error!(
                        chunk_id = %hex::encode(chunk_hash),
                        error = %e,
                        "Failed to fetch chunk"
                    );
                    let _ = tx.send(Err(Status::from(e))).await;
                },
            }

            tracing::info!("FetchChunk stream completed");
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::FetchChunkStream
        ))
    }

    type ListChunksStream = ResponseStream<proto::replication::ListChunksResponse>;

    /// List chunks on this node (streaming)
    ///
    /// Returns a stream of chunk information, paginated by the limit parameter.
    async fn list_chunks(
        &self,
        request: Request<proto::replication::ListChunksRequest>,
    ) -> Result<Response<Self::ListChunksStream>, Status> {
        let req = request.into_inner();

        tracing::debug!(
            start_after = req.start_after.as_ref().map(hex::encode),
            limit = req.limit,
            "Received ListChunks request"
        );

        let chunk_manager = Arc::clone(&self.chunk_manager);

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            match chunk_manager.list_chunks(req.start_after, req.limit).await {
                Ok(chunks) => {
                    tracing::info!(count = chunks.len(), "Listed chunks");

                    let response = proto::replication::ListChunksResponse { chunks };

                    if tx.send(Ok(response)).await.is_err() {
                        tracing::error!("Failed to send chunk list");
                    }
                },
                Err(e) => {
                    tracing::error!(error = %e, "Failed to list chunks");
                    let _ = tx.send(Err(Status::from(e))).await;
                },
            }

            tracing::debug!("ListChunks stream completed");
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::ListChunksStream
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::common;
    use crate::proto::replication::replication_service_server::ReplicationService;
    use crate::proto::replication::*;
    use crate::ProtocolError;
    use std::collections::BTreeMap;
    use tokio::sync::Mutex;

    struct MockBlobStore {
        chunks: Arc<Mutex<BTreeMap<[u8; 32], Vec<u8>>>>,
    }

    impl MockBlobStore {
        #[allow(dead_code)]
        fn new() -> Self {
            Self {
                chunks: Arc::new(Mutex::new(BTreeMap::new())),
            }
        }
    }

    #[tonic::async_trait]
    impl BlobStore for MockBlobStore {
        async fn store_chunk(&self, chunk_id: &[u8; 32], data: Vec<u8>) -> ProtocolResult<()> {
            self.chunks.lock().await.insert(*chunk_id, data);
            Ok(())
        }

        async fn fetch_chunk(&self, chunk_id: &[u8; 32]) -> ProtocolResult<Vec<u8>> {
            self.chunks
                .lock()
                .await
                .get(chunk_id)
                .cloned()
                .ok_or_else(|| ProtocolError::ChunkNotFound(hex::encode(chunk_id)))
        }
    }

    struct MockChunkManager {
        chunks: Arc<Mutex<BTreeMap<[u8; 32], Vec<u8>>>>,
    }

    impl MockChunkManager {
        fn new(chunks: Arc<Mutex<BTreeMap<[u8; 32], Vec<u8>>>>) -> Self {
            Self { chunks }
        }
    }

    #[tonic::async_trait]
    impl ChunkManager for MockChunkManager {
        async fn verify_chunk(&self, chunk_id: &[u8; 32]) -> ProtocolResult<VerifyChunkResponse> {
            let chunks = self.chunks.lock().await;
            let exists = chunks.contains_key(chunk_id);
            let size = chunks.get(chunk_id).map(|d| d.len() as u64).unwrap_or(0);

            Ok(VerifyChunkResponse {
                exists,
                valid: exists,
                size,
            })
        }

        async fn list_chunks(
            &self,
            _start_after: Option<Vec<u8>>,
            limit: u32,
        ) -> ProtocolResult<Vec<ChunkInfo>> {
            let chunks = self.chunks.lock().await;
            let chunk_infos: Vec<ChunkInfo> = chunks
                .iter()
                .take(limit as usize)
                .map(|(id, data)| ChunkInfo {
                    chunk_id: Some(common::ChunkId { hash: id.to_vec() }),
                    size: data.len() as u64,
                    ref_count: 1,
                })
                .collect();

            Ok(chunk_infos)
        }
    }

    #[tokio::test]
    async fn test_verify_chunk_exists() {
        let chunks = Arc::new(Mutex::new(BTreeMap::new()));
        let chunk_id = [0xab; 32];
        chunks.lock().await.insert(chunk_id, vec![1, 2, 3]);

        let blob_store = Arc::new(MockBlobStore {
            chunks: Arc::clone(&chunks),
        });
        let chunk_manager = Arc::new(MockChunkManager::new(Arc::clone(&chunks)));
        let service = ReplicationServiceImpl::new(blob_store, chunk_manager);

        let request = Request::new(VerifyChunkRequest {
            chunk_id: Some(common::ChunkId {
                hash: chunk_id.to_vec(),
            }),
        });

        let response = service.verify_chunk(request).await.unwrap();
        let resp = response.into_inner();

        assert!(resp.exists);
        assert!(resp.valid);
        assert_eq!(resp.size, 3);
    }

    #[tokio::test]
    async fn test_verify_chunk_not_exists() {
        let chunks = Arc::new(Mutex::new(BTreeMap::new()));

        let blob_store = Arc::new(MockBlobStore {
            chunks: Arc::clone(&chunks),
        });
        let chunk_manager = Arc::new(MockChunkManager::new(Arc::clone(&chunks)));
        let service = ReplicationServiceImpl::new(blob_store, chunk_manager);

        let chunk_id = [0xab; 32];
        let request = Request::new(VerifyChunkRequest {
            chunk_id: Some(common::ChunkId {
                hash: chunk_id.to_vec(),
            }),
        });

        let response = service.verify_chunk(request).await.unwrap();
        let resp = response.into_inner();

        assert!(!resp.exists);
        assert!(!resp.valid);
        assert_eq!(resp.size, 0);
    }

    #[tokio::test]
    async fn test_fetch_chunk_success() {
        let chunks: Arc<Mutex<BTreeMap<[u8; 32], Vec<u8>>>> = Arc::new(Mutex::new(BTreeMap::new()));
        let chunk_id = [0xab; 32];
        let data = vec![1, 2, 3, 4, 5];
        chunks.lock().await.insert(chunk_id, data.clone());

        let blob_store = Arc::new(MockBlobStore {
            chunks: Arc::clone(&chunks),
        });
        let chunk_manager = Arc::new(MockChunkManager::new(Arc::clone(&chunks)));
        let service = ReplicationServiceImpl::new(blob_store, chunk_manager);

        let request = Request::new(FetchChunkRequest {
            chunk_id: Some(common::ChunkId {
                hash: chunk_id.to_vec(),
            }),
        });

        let response = service.fetch_chunk(request).await.unwrap();
        let mut stream = response.into_inner();

        let mut received_data = Vec::new();
        while let Some(Ok(chunk)) = stream.next().await {
            received_data.extend_from_slice(&chunk.data);
            if chunk.is_last {
                break;
            }
        }

        assert_eq!(received_data, data);
    }

    #[tokio::test]
    async fn test_list_chunks() {
        let chunks: Arc<Mutex<BTreeMap<[u8; 32], Vec<u8>>>> = Arc::new(Mutex::new(BTreeMap::new()));
        chunks.lock().await.insert([0x01; 32], vec![1, 2, 3]);
        chunks.lock().await.insert([0x02; 32], vec![4, 5, 6]);

        let blob_store = Arc::new(MockBlobStore {
            chunks: Arc::clone(&chunks),
        });
        let chunk_manager = Arc::new(MockChunkManager::new(Arc::clone(&chunks)));
        let service = ReplicationServiceImpl::new(blob_store, chunk_manager);

        let request = Request::new(ListChunksRequest {
            start_after: None,
            limit: 10,
        });

        let response = service.list_chunks(request).await.unwrap();
        let mut stream = response.into_inner();

        let mut total_chunks = 0;
        while let Some(Ok(resp)) = stream.next().await {
            total_chunks += resp.chunks.len();
        }

        assert_eq!(total_chunks, 2);
    }
}
