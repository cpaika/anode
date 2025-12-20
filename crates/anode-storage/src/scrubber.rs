use crate::integrity::IntegrityChecker;
use crate::repair::{ClusterClient, RepairManager, RepairResult};
use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Background data scrubber for continuous integrity checking
pub struct Scrubber<C: ClusterClient> {
    checker: Arc<IntegrityChecker>,
    repair: Arc<RepairManager<C>>,
    config: ScrubberConfig,
    status: Arc<Mutex<ScrubStatusInternal>>,
    running: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct ScrubberConfig {
    /// How often to run full scrub (e.g., weekly)
    pub full_scrub_interval: Duration,
    /// How many chunks to verify per second (rate limiting)
    pub chunks_per_second: u32,
    /// Whether to auto-repair found issues
    pub auto_repair: bool,
    /// Whether to run on startup
    pub scrub_on_startup: bool,
}

impl Default for ScrubberConfig {
    fn default() -> Self {
        Self {
            full_scrub_interval: Duration::from_secs(7 * 24 * 60 * 60), // 1 week
            chunks_per_second: 100,
            auto_repair: false,
            scrub_on_startup: false,
        }
    }
}

impl ScrubberConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.full_scrub_interval = interval;
        self
    }

    pub fn with_rate_limit(mut self, chunks_per_second: u32) -> Self {
        self.chunks_per_second = chunks_per_second;
        self
    }

    pub fn with_auto_repair(mut self, auto_repair: bool) -> Self {
        self.auto_repair = auto_repair;
        self
    }

    pub fn with_scrub_on_startup(mut self, scrub_on_startup: bool) -> Self {
        self.scrub_on_startup = scrub_on_startup;
        self
    }
}

#[derive(Debug, Clone)]
struct ScrubStatusInternal {
    running: bool,
    last_scrub: Option<DateTime<Utc>>,
    next_scrub: Option<DateTime<Utc>>,
    progress: Option<ScrubProgress>,
}

impl Default for ScrubStatusInternal {
    fn default() -> Self {
        Self {
            running: false,
            last_scrub: None,
            next_scrub: None,
            progress: None,
        }
    }
}

impl<C: ClusterClient + 'static> Scrubber<C> {
    pub fn new(
        checker: Arc<IntegrityChecker>,
        repair: Arc<RepairManager<C>>,
        config: ScrubberConfig,
    ) -> Self {
        let next_scrub = if config.scrub_on_startup {
            Some(Utc::now())
        } else {
            Some(Utc::now() + chrono::Duration::from_std(config.full_scrub_interval).unwrap())
        };

        let status = Arc::new(Mutex::new(ScrubStatusInternal {
            running: false,
            last_scrub: None,
            next_scrub,
            progress: None,
        }));

        Self {
            checker,
            repair,
            config,
            status,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start background scrubbing task
    pub fn start(self: Arc<Self>) -> JoinHandle<()> {
        info!("Starting background scrubber");

        let scrubber = self.clone();

        tokio::spawn(async move {
            scrubber.run_background_loop().await;
        })
    }

    /// Background loop that runs scrubs at configured intervals
    async fn run_background_loop(&self) {
        self.running.store(true, Ordering::SeqCst);

        loop {
            // Check if we should run a scrub
            let should_scrub = {
                let status = self.status.lock().unwrap();
                if let Some(next_scrub) = status.next_scrub {
                    Utc::now() >= next_scrub
                } else {
                    false
                }
            };

            if should_scrub {
                info!("Starting scheduled scrub");

                match self.scrub().await {
                    Ok(result) => {
                        info!(
                            "Scrub completed: {} issues found",
                            result.report.total_issues()
                        );

                        // Update status
                        let mut status = self.status.lock().unwrap();
                        status.last_scrub = Some(Utc::now());
                        status.next_scrub = Some(
                            Utc::now()
                                + chrono::Duration::from_std(self.config.full_scrub_interval)
                                    .unwrap(),
                        );
                        status.progress = None;
                    }
                    Err(e) => {
                        warn!("Scrub failed: {}", e);

                        // Still update next scrub time
                        let mut status = self.status.lock().unwrap();
                        status.next_scrub = Some(
                            Utc::now()
                                + chrono::Duration::from_std(self.config.full_scrub_interval)
                                    .unwrap(),
                        );
                        status.progress = None;
                    }
                }
            }

            // Sleep for a bit before checking again
            tokio::time::sleep(Duration::from_secs(60)).await;

            // Check if we should stop
            if !self.running.load(Ordering::SeqCst) {
                info!("Stopping background scrubber");
                break;
            }
        }
    }

    /// Stop the background scrubbing task
    pub fn stop(&self) {
        info!("Stopping scrubber");
        self.running.store(false, Ordering::SeqCst);
    }

    /// Run a single scrub iteration
    pub async fn scrub(&self) -> Result<ScrubResult, crate::error::StorageError> {
        info!("Starting scrub iteration");

        // Mark as running
        {
            let mut status = self.status.lock().unwrap();
            status.running = true;
            status.progress = Some(ScrubProgress {
                chunks_checked: 0,
                total_chunks: 0,
                errors_found: 0,
            });
        }

        // Run integrity check
        let report = self.checker.full_scan().await?;

        debug!(
            "Integrity scan found {} issues",
            report.total_issues()
        );

        // Update progress
        {
            let mut status = self.status.lock().unwrap();
            if let Some(ref mut progress) = status.progress {
                progress.total_chunks = report.total_chunks;
                progress.chunks_checked = report.total_chunks;
                progress.errors_found = report.total_issues();
            }
        }

        // Auto-repair if enabled
        let repair_result = if self.config.auto_repair && report.total_issues() > 0 {
            info!(
                "Auto-repair enabled, repairing {} issues",
                report.total_issues()
            );

            match self.repair.repair(&report).await {
                Ok(result) => {
                    info!(
                        "Auto-repair completed: {}/{} chunks repaired",
                        result.chunks_repaired,
                        result.chunks_repaired + result.chunks_failed
                    );
                    Some(result)
                }
                Err(e) => {
                    warn!("Auto-repair failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Mark as not running
        {
            let mut status = self.status.lock().unwrap();
            status.running = false;
        }

        Ok(ScrubResult {
            report,
            repair_result,
        })
    }

    /// Get current scrub status
    pub fn status(&self) -> ScrubStatus {
        let internal = self.status.lock().unwrap();

        ScrubStatus {
            running: internal.running,
            last_scrub: internal.last_scrub,
            next_scrub: internal.next_scrub,
            progress: internal.progress.clone(),
        }
    }

    /// Manually trigger a scrub (bypasses schedule)
    pub async fn trigger_scrub(&self) -> Result<ScrubResult, crate::error::StorageError> {
        info!("Manually triggering scrub");
        self.scrub().await
    }
}

#[derive(Debug, Clone)]
pub struct ScrubResult {
    pub report: crate::integrity::IntegrityReport,
    pub repair_result: Option<RepairResult>,
}

impl ScrubResult {
    /// Returns true if the scrub found a healthy system
    pub fn is_healthy(&self) -> bool {
        self.report.is_healthy()
    }

    /// Returns true if auto-repair was performed and successful
    pub fn repair_successful(&self) -> bool {
        if let Some(ref result) = self.repair_result {
            result.chunks_failed == 0
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrubStatus {
    pub running: bool,
    pub last_scrub: Option<DateTime<Utc>>,
    pub next_scrub: Option<DateTime<Utc>>,
    pub progress: Option<ScrubProgress>,
}

impl ScrubStatus {
    /// Returns percentage complete of current scrub (0.0 to 1.0)
    pub fn progress_percentage(&self) -> f64 {
        if let Some(ref progress) = self.progress {
            if progress.total_chunks == 0 {
                0.0
            } else {
                progress.chunks_checked as f64 / progress.total_chunks as f64
            }
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrubProgress {
    pub chunks_checked: usize,
    pub total_chunks: usize,
    pub errors_found: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::BlobStore;
    use crate::chunks::compute_chunk_id;
    use crate::integrity::IntegrityChecker;
    use crate::metadata::{BucketConfig, MetadataStore};
    use crate::repair::{MockClusterClient, RepairManager};
    use anode_protocol::{ChunkId as ProtocolChunkId, ObjectKey, ObjectManifest as ProtocolManifest};
    use sha2::{Digest, Sha256};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn setup_test_env() -> (
        Arc<Scrubber<MockClusterClient>>,
        Arc<MetadataStore>,
        Arc<BlobStore>,
        TempDir,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let blob_dir = temp_dir.path().join("blobs");
        let metadata_path = temp_dir.path().join("metadata.db");

        let blobs = Arc::new(BlobStore::new(blob_dir).unwrap());
        let metadata = Arc::new(MetadataStore::open(&metadata_path).unwrap());
        let cluster = Arc::new(MockClusterClient::new());

        let checker = Arc::new(IntegrityChecker::new(metadata.clone(), blobs.clone()));
        let repair = Arc::new(RepairManager::new(
            metadata.clone(),
            blobs.clone(),
            cluster,
        ));

        let config = ScrubberConfig::default()
            .with_interval(Duration::from_secs(60))
            .with_auto_repair(false);

        let scrubber = Arc::new(Scrubber::new(checker, repair, config));

        (scrubber, metadata, blobs, temp_dir)
    }

    #[tokio::test]
    async fn test_scrubber_config() {
        let config = ScrubberConfig::new()
            .with_interval(Duration::from_secs(3600))
            .with_rate_limit(50)
            .with_auto_repair(true)
            .with_scrub_on_startup(true);

        assert_eq!(config.full_scrub_interval, Duration::from_secs(3600));
        assert_eq!(config.chunks_per_second, 50);
        assert!(config.auto_repair);
        assert!(config.scrub_on_startup);
    }

    #[tokio::test]
    async fn test_scrub_healthy_system() {
        let (scrubber, metadata, blobs, _temp) = setup_test_env();

        // Create bucket
        let bucket_config = BucketConfig::new("test-bucket");
        metadata.create_bucket(&bucket_config).unwrap();

        // Create valid chunk
        let data = b"Valid chunk data";
        let chunk_id = compute_chunk_id(data);
        blobs.write_chunk(&chunk_id, data).unwrap();
        metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Create object
        let object_key = ObjectKey::new("test-bucket", "test-key");
        let protocol_chunk_id = ProtocolChunkId::from_hex(&chunk_id.0).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(data);
        let checksum_bytes = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&checksum_bytes);

        let manifest = ProtocolManifest::new(
            object_key,
            data.len() as u64,
            checksum,
            vec![protocol_chunk_id],
        );

        metadata.put_object(&manifest).unwrap();

        // Run scrub
        let result = scrubber.scrub().await.unwrap();

        assert!(result.is_healthy());
        assert!(result.report.is_healthy());
        assert_eq!(result.report.total_issues(), 0);
    }

    #[tokio::test]
    async fn test_scrub_with_corruption() {
        let (scrubber, metadata, blobs, _temp) = setup_test_env();

        // Create bucket
        let bucket_config = BucketConfig::new("test-bucket");
        metadata.create_bucket(&bucket_config).unwrap();

        // Create corrupted chunk
        let original_data = b"Original data";
        let chunk_id = compute_chunk_id(original_data);

        // Write different data
        blobs.write_chunk(&chunk_id, b"Corrupted!").unwrap();
        metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Create object
        let object_key = ObjectKey::new("test-bucket", "test-key");
        let protocol_chunk_id = ProtocolChunkId::from_hex(&chunk_id.0).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(original_data);
        let checksum_bytes = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&checksum_bytes);

        let manifest = ProtocolManifest::new(
            object_key,
            original_data.len() as u64,
            checksum,
            vec![protocol_chunk_id],
        );

        metadata.put_object(&manifest).unwrap();

        // Run scrub
        let result = scrubber.scrub().await.unwrap();

        assert!(!result.is_healthy());
        assert_eq!(result.report.corrupted_chunks.len(), 1);
        assert_eq!(result.report.total_issues(), 1);
    }

    #[tokio::test]
    async fn test_scrub_with_auto_repair() {
        let temp_dir = TempDir::new().unwrap();
        let blob_dir = temp_dir.path().join("blobs");
        let metadata_path = temp_dir.path().join("metadata.db");

        let blobs = Arc::new(BlobStore::new(blob_dir).unwrap());
        let metadata = Arc::new(MetadataStore::open(&metadata_path).unwrap());
        let mut cluster = MockClusterClient::new();

        // Setup: Create bucket
        let bucket_config = BucketConfig::new("test-bucket");
        metadata.create_bucket(&bucket_config).unwrap();

        // Create corrupted chunk
        let original_data = b"Original data";
        let chunk_id = compute_chunk_id(original_data);

        // Add correct version to cluster
        cluster.add_chunk(chunk_id.clone(), original_data.to_vec());

        // Write corrupted version to local store
        blobs.write_chunk(&chunk_id, b"Corrupted!").unwrap();
        metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Create object
        let object_key = ObjectKey::new("test-bucket", "test-key");
        let protocol_chunk_id = ProtocolChunkId::from_hex(&chunk_id.0).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(original_data);
        let checksum_bytes = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&checksum_bytes);

        let manifest = ProtocolManifest::new(
            object_key,
            original_data.len() as u64,
            checksum,
            vec![protocol_chunk_id],
        );

        metadata.put_object(&manifest).unwrap();

        // Create scrubber with auto-repair enabled
        let cluster_arc = Arc::new(cluster);
        let checker = Arc::new(IntegrityChecker::new(metadata.clone(), blobs.clone()));
        let repair = Arc::new(RepairManager::new(
            metadata.clone(),
            blobs.clone(),
            cluster_arc,
        ));

        let config = ScrubberConfig::default().with_auto_repair(true);
        let scrubber = Arc::new(Scrubber::new(checker, repair, config));

        // Run scrub
        let result = scrubber.scrub().await.unwrap();

        assert!(!result.is_healthy()); // Initially unhealthy
        assert!(result.repair_result.is_some());

        let repair_result = result.repair_result.unwrap();
        assert_eq!(repair_result.chunks_repaired, 1);
        assert_eq!(repair_result.chunks_failed, 0);

        // Verify chunk was repaired
        let stored_data = blobs.read_chunk(&chunk_id).unwrap();
        assert_eq!(stored_data, original_data);
    }

    #[tokio::test]
    async fn test_scrubber_status() {
        let (scrubber, _metadata, _blobs, _temp) = setup_test_env();

        let status = scrubber.status();
        assert!(!status.running);
        assert!(status.last_scrub.is_none());
        assert!(status.next_scrub.is_some());
    }

    #[tokio::test]
    async fn test_scrub_status_progress() {
        let (scrubber, metadata, blobs, _temp) = setup_test_env();

        // Create bucket with some data
        let bucket_config = BucketConfig::new("test-bucket");
        metadata.create_bucket(&bucket_config).unwrap();

        let data = b"Test data";
        let chunk_id = compute_chunk_id(data);
        blobs.write_chunk(&chunk_id, data).unwrap();
        metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Start scrub in background
        let scrubber_clone = scrubber.clone();
        let handle = tokio::spawn(async move {
            scrubber_clone.scrub().await.unwrap()
        });

        // Wait a bit and check status
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Wait for scrub to complete
        let result = handle.await.unwrap();

        // Check final status
        let status = scrubber.status();
        assert!(!status.running);

        // Progress should be complete
        if let Some(progress) = status.progress {
            assert_eq!(progress.chunks_checked, progress.total_chunks);
        }
    }

    #[tokio::test]
    async fn test_trigger_manual_scrub() {
        let (scrubber, _metadata, _blobs, _temp) = setup_test_env();

        let result = scrubber.trigger_scrub().await.unwrap();
        assert!(result.is_healthy());
    }

    #[tokio::test]
    async fn test_scrub_result_helpers() {
        let report = crate::integrity::IntegrityReport {
            total_objects: 0,
            total_chunks: 0,
            corrupted_chunks: Vec::new(),
            missing_chunks: Vec::new(),
            orphaned_chunks: Vec::new(),
            reference_count_errors: Vec::new(),
            verification_time: Duration::from_secs(1),
        };

        let result = ScrubResult {
            report,
            repair_result: None,
        };

        assert!(result.is_healthy());
        assert!(!result.repair_successful());

        let mut repair_res = RepairResult::new();
        repair_res.chunks_repaired = 5;

        let result_with_repair = ScrubResult {
            report: crate::integrity::IntegrityReport {
                total_objects: 0,
                total_chunks: 0,
                corrupted_chunks: Vec::new(),
                missing_chunks: Vec::new(),
                orphaned_chunks: Vec::new(),
                reference_count_errors: Vec::new(),
                verification_time: Duration::from_secs(1),
            },
            repair_result: Some(repair_res),
        };

        assert!(result_with_repair.repair_successful());
    }

    #[tokio::test]
    async fn test_status_progress_percentage() {
        let mut status = ScrubStatus {
            running: true,
            last_scrub: None,
            next_scrub: None,
            progress: Some(ScrubProgress {
                chunks_checked: 50,
                total_chunks: 100,
                errors_found: 0,
            }),
        };

        assert_eq!(status.progress_percentage(), 0.5);

        status.progress = None;
        assert_eq!(status.progress_percentage(), 0.0);
    }
}
