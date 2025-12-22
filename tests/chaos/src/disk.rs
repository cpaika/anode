//! Disk chaos utilities
//!
//! Provides disk manipulation capabilities for chaos testing.

use crate::{ChaosError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

/// Disk chaos controller
pub struct DiskChaos {
    data_dir: PathBuf,
}

impl DiskChaos {
    /// Create a new disk chaos controller
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Fill disk to a specified percentage
    pub async fn fill_disk(&self, target_percent: u8) -> Result<PathBuf> {
        if target_percent > 100 {
            return Err(ChaosError::DiskChaos(
                "Target percentage must be <= 100".to_string(),
            ));
        }

        info!("Filling disk to {}%", target_percent);

        // Get current disk usage
        let total_space = self.get_total_space()?;
        let used_space = self.get_used_space()?;
        let _available_space = total_space - used_space;

        // Calculate how much to fill
        let target_used = (total_space * target_percent as u64) / 100;
        let bytes_to_fill = target_used.saturating_sub(used_space);

        if bytes_to_fill == 0 {
            info!("Disk already at target usage");
            return Ok(self.data_dir.join("fillfile"));
        }

        info!("Filling {} bytes", bytes_to_fill);

        // Create a large file using dd
        let fillfile = self.data_dir.join("fillfile");

        let output = Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", fillfile.display()),
                "bs=1M",
                &format!("count={}", bytes_to_fill / (1024 * 1024)),
            ])
            .output()
            .map_err(|e| ChaosError::DiskChaos(format!("Failed to fill disk: {}", e)))?;

        if !output.status.success() {
            return Err(ChaosError::DiskChaos("dd command failed".to_string()));
        }

        Ok(fillfile)
    }

    /// Clear disk fill file
    pub async fn clear_disk_fill(&self) -> Result<()> {
        let fillfile = self.data_dir.join("fillfile");

        if fillfile.exists() {
            info!("Removing disk fill file");
            std::fs::remove_file(&fillfile)
                .map_err(|e| ChaosError::DiskChaos(format!("Failed to remove fill file: {}", e)))?;
        }

        Ok(())
    }

    /// Make filesystem read-only
    pub async fn make_readonly(&self) -> Result<()> {
        info!("Making filesystem read-only");

        warn!("Making filesystem read-only requires root privileges");
        warn!("This operation is not fully implemented");

        // This would require:
        // 1. Finding the mount point
        // 2. Running: mount -o remount,ro <mount_point>
        // Requires root privileges

        Err(ChaosError::DiskChaos(
            "Read-only filesystem not implemented - requires root".to_string(),
        ))
    }

    /// Make filesystem read-write again
    pub async fn make_readwrite(&self) -> Result<()> {
        info!("Making filesystem read-write");

        warn!("This operation is not fully implemented");

        Err(ChaosError::DiskChaos(
            "Read-write filesystem not implemented - requires root".to_string(),
        ))
    }

    /// Corrupt a file by overwriting random bytes
    pub async fn corrupt_file(&self, file_path: &Path, num_bytes: usize) -> Result<()> {
        info!("Corrupting file: {:?} ({} bytes)", file_path, num_bytes);

        if !file_path.exists() {
            return Err(ChaosError::DiskChaos(format!(
                "File does not exist: {:?}",
                file_path
            )));
        }

        // Read file
        let data = std::fs::read(file_path)
            .map_err(|e| ChaosError::DiskChaos(format!("Failed to read file: {}", e)))?;

        if data.is_empty() {
            return Ok(());
        }

        // Corrupt random bytes
        let mut corrupted = data.clone();
        let mut rng = rand::thread_rng();

        use rand::Rng;
        for _ in 0..num_bytes.min(data.len()) {
            let pos = rng.gen_range(0..data.len());
            corrupted[pos] = rng.gen();
        }

        // Write back
        std::fs::write(file_path, corrupted)
            .map_err(|e| ChaosError::DiskChaos(format!("Failed to write corrupted file: {}", e)))?;

        info!("File corrupted successfully");

        Ok(())
    }

    /// Delete a file
    pub async fn delete_file(&self, file_path: &Path) -> Result<()> {
        info!("Deleting file: {:?}", file_path);

        if file_path.exists() {
            std::fs::remove_file(file_path)
                .map_err(|e| ChaosError::DiskChaos(format!("Failed to delete file: {}", e)))?;
        }

        Ok(())
    }

    /// Delete a directory recursively
    pub async fn delete_directory(&self, dir_path: &Path) -> Result<()> {
        info!("Deleting directory: {:?}", dir_path);

        if dir_path.exists() {
            std::fs::remove_dir_all(dir_path)
                .map_err(|e| ChaosError::DiskChaos(format!("Failed to delete directory: {}", e)))?;
        }

        Ok(())
    }

    /// Set disk I/O scheduler (Linux only)
    pub async fn set_io_scheduler(&self, scheduler: &str) -> Result<()> {
        info!("Setting I/O scheduler to {}", scheduler);

        #[cfg(target_os = "linux")]
        {
            // This would require finding the block device and setting scheduler
            // Example: echo "deadline" > /sys/block/sda/queue/scheduler
            warn!("I/O scheduler setting not fully implemented - requires root");
        }

        #[cfg(not(target_os = "linux"))]
        {
            warn!("I/O scheduler setting only supported on Linux");
        }

        Err(ChaosError::DiskChaos(
            "I/O scheduler setting not implemented - requires root".to_string(),
        ))
    }

    /// Get total disk space in bytes
    fn get_total_space(&self) -> Result<u64> {
        #[cfg(unix)]
        {
            let _metadata = std::fs::metadata(&self.data_dir)
                .map_err(|e| ChaosError::DiskChaos(format!("Failed to get metadata: {}", e)))?;

            // This is a simplified version - would need statvfs for accurate info
            Ok(100 * 1024 * 1024 * 1024) // Assume 100GB for now
        }

        #[cfg(not(unix))]
        {
            Ok(100 * 1024 * 1024 * 1024) // Assume 100GB
        }
    }

    /// Get used disk space in bytes
    fn get_used_space(&self) -> Result<u64> {
        // This is a simplified version
        let mut total = 0u64;

        if let Ok(entries) = std::fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    total += metadata.len();
                }
            }
        }

        Ok(total)
    }

    /// Inject I/O errors using dm-flakey (Linux only)
    pub async fn inject_io_errors(&self, error_rate: f64) -> Result<()> {
        info!("Injecting I/O errors at {}% rate", error_rate * 100.0);

        warn!("I/O error injection requires dm-flakey and root privileges");
        warn!("This feature is not fully implemented");

        // This would require:
        // 1. Creating a dm-flakey device
        // 2. Configuring error injection
        // 3. Mounting the data directory on the flakey device

        Err(ChaosError::DiskChaos(
            "I/O error injection not implemented - requires dm-flakey and root".to_string(),
        ))
    }

    /// Clear I/O error injection
    pub async fn clear_io_errors(&self) -> Result<()> {
        info!("Clearing I/O error injection");

        warn!("This feature is not fully implemented");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_disk_chaos_creation() {
        let temp_dir = TempDir::new().unwrap();
        let _chaos = DiskChaos::new(temp_dir.path().to_path_buf());
    }

    #[tokio::test]
    async fn test_corrupt_file() {
        let temp_dir = TempDir::new().unwrap();
        let chaos = DiskChaos::new(temp_dir.path().to_path_buf());

        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, b"hello world").unwrap();

        let result = chaos.corrupt_file(&test_file, 5).await;
        assert!(result.is_ok());

        let corrupted = std::fs::read(&test_file).unwrap();
        assert_eq!(corrupted.len(), 11); // Same length
                                         // Content should be different (with very high probability)
    }

    #[tokio::test]
    async fn test_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let chaos = DiskChaos::new(temp_dir.path().to_path_buf());

        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, b"test").unwrap();
        assert!(test_file.exists());

        chaos.delete_file(&test_file).await.unwrap();
        assert!(!test_file.exists());
    }
}
