//! Data verification utilities for chaos testing
//!
//! Tracks written data and verifies integrity after chaos events.

use crate::{ChaosError, Result};
use bytes::Bytes;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

/// Verification result
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub total_objects: usize,
    pub verified_objects: usize,
    pub missing_objects: Vec<String>,
    pub corrupted_objects: Vec<String>,
}

impl VerificationResult {
    pub fn is_success(&self) -> bool {
        self.missing_objects.is_empty() && self.corrupted_objects.is_empty()
    }
}

/// Data verifier that tracks writes and verifies reads
#[derive(Clone)]
pub struct DataVerifier {
    endpoint: String,
    client: Client,
    // Track written objects: (bucket, key) -> data
    written_objects: Arc<Mutex<HashMap<(String, String), Vec<u8>>>>,
}

impl DataVerifier {
    /// Create a new data verifier
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: Client::new(),
            written_objects: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Write data and track it for later verification
    pub async fn write_and_track(
        &mut self,
        bucket: &str,
        key: &str,
        data: Vec<u8>,
    ) -> Result<()> {
        // First ensure bucket exists
        self.ensure_bucket_exists(bucket).await?;

        // Write the object
        let url = format!("{}/{}/{}", self.endpoint, bucket, key);

        self.client
            .put(&url)
            .body(data.clone())
            .send()
            .await
            .map_err(|e| ChaosError::VerificationFailed(format!("Write failed: {}", e)))?;

        // Track it
        self.written_objects
            .lock()
            .unwrap()
            .insert((bucket.to_string(), key.to_string()), data);

        debug!("Wrote and tracked object {}/{}", bucket, key);

        Ok(())
    }

    /// Verify a specific object matches expected data
    pub async fn verify_read(
        &self,
        bucket: &str,
        key: &str,
        expected_data: &[u8],
    ) -> Result<bool> {
        let url = format!("{}/{}/{}", self.endpoint, bucket, key);

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ChaosError::VerificationFailed(format!("Read failed: {}", e)))?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let actual_data = response
            .bytes()
            .await
            .map_err(|e| ChaosError::VerificationFailed(format!("Failed to read body: {}", e)))?;

        Ok(actual_data.as_ref() == expected_data)
    }

    /// Verify an object exists
    pub async fn verify_exists(&self, bucket: &str, key: &str) -> Result<bool> {
        let url = format!("{}/{}/{}", self.endpoint, bucket, key);

        let response = self.client
            .head(&url)
            .send()
            .await
            .map_err(|e| ChaosError::VerificationFailed(format!("HEAD request failed: {}", e)))?;

        Ok(response.status().is_success())
    }

    /// Verify all tracked objects
    pub async fn verify_all(&self) -> Result<bool> {
        info!("Verifying all tracked objects");

        let objects = self.written_objects.lock().unwrap().clone();

        let mut all_verified = true;

        for ((bucket, key), expected_data) in objects.iter() {
            match self.verify_read(bucket, key, expected_data).await {
                Ok(true) => {
                    debug!("Verified {}/{}", bucket, key);
                }
                Ok(false) => {
                    info!("Verification failed for {}/{}", bucket, key);
                    all_verified = false;
                }
                Err(e) => {
                    info!("Error verifying {}/{}: {}", bucket, key, e);
                    all_verified = false;
                }
            }
        }

        if all_verified {
            info!("All {} objects verified successfully", objects.len());
        } else {
            info!("Some objects failed verification");
        }

        Ok(all_verified)
    }

    /// Get detailed verification results
    pub async fn get_verification_results(&self) -> Result<VerificationResult> {
        let objects = self.written_objects.lock().unwrap().clone();

        let total_objects = objects.len();
        let mut verified_objects = 0;
        let mut missing_objects = Vec::new();
        let mut corrupted_objects = Vec::new();

        for ((bucket, key), expected_data) in objects.iter() {
            let object_path = format!("{}/{}", bucket, key);

            match self.verify_exists(bucket, key).await {
                Ok(false) => {
                    missing_objects.push(object_path);
                    continue;
                }
                Err(e) => {
                    info!("Error checking existence of {}: {}", object_path, e);
                    missing_objects.push(object_path);
                    continue;
                }
                _ => {}
            }

            match self.verify_read(bucket, key, expected_data).await {
                Ok(true) => {
                    verified_objects += 1;
                }
                Ok(false) => {
                    corrupted_objects.push(object_path);
                }
                Err(e) => {
                    info!("Error reading {}: {}", object_path, e);
                    corrupted_objects.push(object_path);
                }
            }
        }

        Ok(VerificationResult {
            total_objects,
            verified_objects,
            missing_objects,
            corrupted_objects,
        })
    }

    /// Clear all tracked objects
    pub fn clear(&mut self) {
        self.written_objects.lock().unwrap().clear();
    }

    /// Get the number of tracked objects
    pub fn tracked_count(&self) -> usize {
        self.written_objects.lock().unwrap().len()
    }

    /// Ensure a bucket exists (create if needed)
    async fn ensure_bucket_exists(&self, bucket: &str) -> Result<()> {
        let url = format!("{}/{}", self.endpoint, bucket);

        // Try to HEAD the bucket
        let response = self.client.head(&url).send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                // Bucket exists
                Ok(())
            }
            _ => {
                // Bucket doesn't exist, try to create it
                let create_response = self.client
                    .put(&url)
                    .send()
                    .await
                    .map_err(|e| ChaosError::VerificationFailed(format!("Failed to create bucket: {}", e)))?;

                if create_response.status().is_success() {
                    debug!("Created bucket {}", bucket);
                    Ok(())
                } else {
                    Err(ChaosError::VerificationFailed(format!(
                        "Failed to create bucket: {}",
                        create_response.status()
                    )))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_verifier_creation() {
        let verifier = DataVerifier::new("http://localhost:9000".to_string());
        assert_eq!(verifier.tracked_count(), 0);
    }

    #[test]
    fn test_verification_result() {
        let result = VerificationResult {
            total_objects: 10,
            verified_objects: 10,
            missing_objects: vec![],
            corrupted_objects: vec![],
        };
        assert!(result.is_success());

        let result_with_failures = VerificationResult {
            total_objects: 10,
            verified_objects: 8,
            missing_objects: vec!["bucket/key1".to_string()],
            corrupted_objects: vec!["bucket/key2".to_string()],
        };
        assert!(!result_with_failures.is_success());
    }
}
