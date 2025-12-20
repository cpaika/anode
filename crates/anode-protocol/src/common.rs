use crate::proto;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Unique identifier for a node in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({})", self.0)
    }
}

impl From<proto::common::NodeId> for NodeId {
    fn from(proto: proto::common::NodeId) -> Self {
        Self(proto.id)
    }
}

impl From<NodeId> for proto::common::NodeId {
    fn from(node_id: NodeId) -> Self {
        Self { id: node_id.0 }
    }
}

/// Unique identifier for a cluster
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClusterId(pub String);

impl fmt::Display for ClusterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cluster({})", self.0)
    }
}

impl From<proto::common::ClusterId> for ClusterId {
    fn from(proto: proto::common::ClusterId) -> Self {
        Self(proto.id)
    }
}

impl From<ClusterId> for proto::common::ClusterId {
    fn from(cluster_id: ClusterId) -> Self {
        Self { id: cluster_id.0 }
    }
}

/// Content-addressed chunk identifier (SHA-256 hash)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId(pub [u8; 32]);

impl ChunkId {
    pub fn from_hash(hash: &[u8]) -> Option<Self> {
        if hash.len() == 32 {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(hash);
            Some(Self(bytes))
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(hex_str: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(hex_str)?;
        Self::from_hash(&bytes).ok_or(hex::FromHexError::InvalidStringLength)
    }
}

impl fmt::Display for ChunkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Chunk({})", hex::encode(&self.0[..8]))
    }
}

impl From<proto::common::ChunkId> for ChunkId {
    fn from(proto: proto::common::ChunkId) -> Self {
        let mut bytes = [0u8; 32];
        if proto.hash.len() >= 32 {
            bytes.copy_from_slice(&proto.hash[..32]);
        } else {
            bytes[..proto.hash.len()].copy_from_slice(&proto.hash);
        }
        Self(bytes)
    }
}

impl From<ChunkId> for proto::common::ChunkId {
    fn from(chunk_id: ChunkId) -> Self {
        Self {
            hash: chunk_id.0.to_vec(),
        }
    }
}

/// Key identifying an object in the storage system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectKey {
    pub bucket: String,
    pub key: String,
}

impl ObjectKey {
    pub fn new(bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            key: key.into(),
        }
    }
}

impl fmt::Display for ObjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.bucket, self.key)
    }
}

impl From<proto::common::ObjectKey> for ObjectKey {
    fn from(proto: proto::common::ObjectKey) -> Self {
        Self {
            bucket: proto.bucket,
            key: proto.key,
        }
    }
}

impl From<ObjectKey> for proto::common::ObjectKey {
    fn from(object_key: ObjectKey) -> Self {
        Self {
            bucket: object_key.bucket,
            key: object_key.key,
        }
    }
}

/// Metadata for an object, including its chunks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectManifest {
    pub key: ObjectKey,
    pub total_size: u64,
    pub checksum: [u8; 32],
    pub chunks: Vec<ChunkId>,
    pub created_at: i64,
    pub modified_at: i64,
    pub metadata: HashMap<String, String>,
    pub parquet_metadata: Option<ParquetMeta>,
}

impl ObjectManifest {
    pub fn new(
        key: ObjectKey,
        total_size: u64,
        checksum: [u8; 32],
        chunks: Vec<ChunkId>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        Self {
            key,
            total_size,
            checksum,
            chunks,
            created_at: now,
            modified_at: now,
            metadata: HashMap::new(),
            parquet_metadata: None,
        }
    }
}

impl From<proto::common::ObjectManifest> for ObjectManifest {
    fn from(proto: proto::common::ObjectManifest) -> Self {
        Self {
            key: proto.key.unwrap_or_default().into(),
            total_size: proto.total_size,
            checksum: {
                let mut bytes = [0u8; 32];
                if proto.checksum.len() >= 32 {
                    bytes.copy_from_slice(&proto.checksum[..32]);
                } else {
                    bytes[..proto.checksum.len()].copy_from_slice(&proto.checksum);
                }
                bytes
            },
            chunks: proto.chunks.into_iter().map(|c| c.into()).collect(),
            created_at: proto.created_at,
            modified_at: proto.modified_at,
            metadata: proto.metadata,
            parquet_metadata: proto.parquet_meta.map(|pm| pm.into()),
        }
    }
}

impl From<ObjectManifest> for proto::common::ObjectManifest {
    fn from(manifest: ObjectManifest) -> Self {
        Self {
            key: Some(manifest.key.into()),
            total_size: manifest.total_size,
            checksum: manifest.checksum.to_vec(),
            chunks: manifest.chunks.into_iter().map(|c| c.into()).collect(),
            created_at: manifest.created_at,
            modified_at: manifest.modified_at,
            metadata: manifest.metadata,
            parquet_meta: manifest.parquet_metadata.map(|pm| pm.into()),
        }
    }
}

/// Parquet-specific metadata cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParquetMeta {
    pub schema: Vec<u8>,
    pub row_groups: Vec<RowGroupInfo>,
}

impl From<proto::common::ParquetMetadata> for ParquetMeta {
    fn from(proto: proto::common::ParquetMetadata) -> Self {
        Self {
            schema: proto.schema,
            row_groups: proto.row_groups.into_iter().map(|rg| rg.into()).collect(),
        }
    }
}

impl From<ParquetMeta> for proto::common::ParquetMetadata {
    fn from(meta: ParquetMeta) -> Self {
        Self {
            schema: meta.schema,
            row_groups: meta.row_groups.into_iter().map(|rg| rg.into()).collect(),
        }
    }
}

/// Row group information for Parquet files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowGroupInfo {
    pub num_rows: u64,
    pub total_byte_size: u64,
    pub columns: Vec<ColumnChunkInfo>,
}

impl From<proto::common::RowGroupInfo> for RowGroupInfo {
    fn from(proto: proto::common::RowGroupInfo) -> Self {
        Self {
            num_rows: proto.num_rows,
            total_byte_size: proto.total_byte_size,
            columns: proto.columns.into_iter().map(|c| c.into()).collect(),
        }
    }
}

impl From<RowGroupInfo> for proto::common::RowGroupInfo {
    fn from(info: RowGroupInfo) -> Self {
        Self {
            num_rows: info.num_rows,
            total_byte_size: info.total_byte_size,
            columns: info.columns.into_iter().map(|c| c.into()).collect(),
        }
    }
}

/// Column chunk information for Parquet files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnChunkInfo {
    pub column_path: String,
    pub offset: u64,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
}

impl From<proto::common::ColumnChunkInfo> for ColumnChunkInfo {
    fn from(proto: proto::common::ColumnChunkInfo) -> Self {
        Self {
            column_path: proto.column_path,
            offset: proto.offset,
            compressed_size: proto.compressed_size,
            uncompressed_size: proto.uncompressed_size,
        }
    }
}

impl From<ColumnChunkInfo> for proto::common::ColumnChunkInfo {
    fn from(info: ColumnChunkInfo) -> Self {
        Self {
            column_path: info.column_path,
            offset: info.offset,
            compressed_size: info.compressed_size,
            uncompressed_size: info.uncompressed_size,
        }
    }
}

/// Bucket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketConfig {
    pub name: String,
    pub replication_factor: u32,
    pub created_at: i64,
}

impl From<proto::common::BucketConfig> for BucketConfig {
    fn from(proto: proto::common::BucketConfig) -> Self {
        Self {
            name: proto.name,
            replication_factor: proto.replication_factor,
            created_at: proto.created_at,
        }
    }
}

impl From<BucketConfig> for proto::common::BucketConfig {
    fn from(config: BucketConfig) -> Self {
        Self {
            name: config.name,
            replication_factor: config.replication_factor,
            created_at: config.created_at,
        }
    }
}

/// Placement group for data distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementGroup {
    pub id: u32,
    pub members: Vec<NodeId>,
    pub leader: NodeId,
}

impl From<proto::common::PlacementGroup> for PlacementGroup {
    fn from(proto: proto::common::PlacementGroup) -> Self {
        Self {
            id: proto.id,
            members: proto.members.into_iter().map(|n| n.into()).collect(),
            leader: proto.leader.unwrap_or_default().into(),
        }
    }
}

impl From<PlacementGroup> for proto::common::PlacementGroup {
    fn from(group: PlacementGroup) -> Self {
        Self {
            id: group.id,
            members: group.members.into_iter().map(|n| n.into()).collect(),
            leader: Some(group.leader.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_conversion() {
        let node_id = NodeId(42);
        let proto: proto::common::NodeId = node_id.into();
        assert_eq!(proto.id, 42);

        let converted: NodeId = proto.into();
        assert_eq!(converted, node_id);
    }

    #[test]
    fn test_cluster_id_conversion() {
        let cluster_id = ClusterId("test-cluster".to_string());
        let proto: proto::common::ClusterId = cluster_id.clone().into();
        assert_eq!(proto.id, "test-cluster");

        let converted: ClusterId = proto.into();
        assert_eq!(converted, cluster_id);
    }

    #[test]
    fn test_chunk_id_conversion() {
        let hash = [0u8; 32];
        let chunk_id = ChunkId(hash);
        let proto: proto::common::ChunkId = chunk_id.clone().into();
        assert_eq!(proto.hash.len(), 32);

        let converted: ChunkId = proto.into();
        assert_eq!(converted, chunk_id);
    }

    #[test]
    fn test_chunk_id_hex() {
        let hash = [0xab; 32];
        let chunk_id = ChunkId(hash);
        let hex = chunk_id.to_hex();
        assert_eq!(hex.len(), 64);

        let converted = ChunkId::from_hex(&hex).unwrap();
        assert_eq!(converted, chunk_id);
    }

    #[test]
    fn test_object_key_conversion() {
        let key = ObjectKey::new("bucket1", "path/to/object");
        let proto: proto::common::ObjectKey = key.clone().into();
        assert_eq!(proto.bucket, "bucket1");
        assert_eq!(proto.key, "path/to/object");

        let converted: ObjectKey = proto.into();
        assert_eq!(converted, key);
    }

    #[test]
    fn test_object_manifest_conversion() {
        let key = ObjectKey::new("bucket", "key");
        let checksum = [0xab; 32];
        let chunks = vec![ChunkId([0x01; 32]), ChunkId([0x02; 32])];
        let manifest = ObjectManifest::new(key, 1024, checksum, chunks);

        let proto: proto::common::ObjectManifest = manifest.clone().into();
        assert_eq!(proto.total_size, 1024);
        assert_eq!(proto.chunks.len(), 2);

        let converted: ObjectManifest = proto.into();
        assert_eq!(converted.total_size, manifest.total_size);
        assert_eq!(converted.chunks.len(), manifest.chunks.len());
    }
}
