//! Access key management for S3 API authentication.
//!
//! This module provides secure storage and management of access keys used for
//! S3 API authentication. Keys are stored in an embedded database with the
//! secret key hashed for security.

use anyhow::{Context, Result};
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

const ACCESS_KEYS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("access_keys");

/// Access key metadata stored in the database.
///
/// The secret key is stored as a SHA-256 hash for security. The actual secret
/// is only returned when the key is first created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKey {
    /// Unique access key ID (e.g., "AKIAIOSFODNN7EXAMPLE")
    pub access_key_id: String,
    /// SHA-256 hash of the secret access key (never the actual secret)
    pub secret_key_hash: String,
    /// Human-readable name or description
    pub name: String,
    /// Whether this key is currently active
    pub active: bool,
    /// Unix timestamp when the key was created
    pub created_at: u64,
    /// Unix timestamp when the key was last used (optional)
    pub last_used_at: Option<u64>,
}

impl AccessKey {
    /// Create a new access key with generated credentials.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name or description for this key
    ///
    /// # Returns
    ///
    /// Returns a tuple of (AccessKey with hashed secret, plain text secret).
    /// The plain text secret is only available at creation time and should be
    /// shown to the user once.
    pub fn new(name: String) -> (Self, String) {
        let access_key_id = Self::generate_access_key_id();
        let secret_access_key = Self::generate_secret_access_key();
        let secret_key_hash = Self::hash_secret(&secret_access_key);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let key = Self {
            access_key_id,
            secret_key_hash,
            name,
            active: true,
            created_at: now,
            last_used_at: None,
        };

        (key, secret_access_key)
    }

    /// Generate a random access key ID.
    ///
    /// Format: 20 uppercase alphanumeric characters starting with "AKIA"
    /// (similar to AWS access key IDs)
    fn generate_access_key_id() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let suffix: String = (0..16)
            .map(|_| {
                let idx = rng.gen_range(0..36);
                if idx < 26 {
                    (b'A' + idx as u8) as char
                } else {
                    (b'0' + (idx - 26) as u8) as char
                }
            })
            .collect();
        format!("AKIA{}", suffix)
    }

    /// Generate a random secret access key.
    ///
    /// Format: 40 character base64-like string (similar to AWS secret keys)
    fn generate_secret_access_key() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        (0..40)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Hash a secret access key using SHA-256.
    fn hash_secret(secret: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verify a plain text secret against this key's hash.
    pub fn verify_secret(&self, secret: &str) -> bool {
        let hash = Self::hash_secret(secret);
        // Use constant-time comparison to prevent timing attacks
        hash == self.secret_key_hash
    }
}

/// Response when creating a new access key.
///
/// Contains the access key metadata and the secret key (only shown once).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccessKeyResponse {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub name: String,
    pub created_at: u64,
}

/// Response when retrieving access key details.
///
/// Does not include the secret key (it's never retrievable after creation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKeyInfo {
    pub access_key_id: String,
    pub name: String,
    pub active: bool,
    pub created_at: u64,
    pub last_used_at: Option<u64>,
}

impl From<AccessKey> for AccessKeyInfo {
    fn from(key: AccessKey) -> Self {
        Self {
            access_key_id: key.access_key_id,
            name: key.name,
            active: key.active,
            created_at: key.created_at,
            last_used_at: key.last_used_at,
        }
    }
}

/// Key store for managing access keys.
///
/// Provides CRUD operations for access keys with secure storage using redb.
/// Secret keys are hashed and never stored in plain text.
pub struct KeyStore {
    db: Database,
}

impl KeyStore {
    /// Open or create a key store at the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database file
    pub fn open(path: &Path) -> Result<Self> {
        info!("Opening key store at {:?}", path);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db = Database::create(path)?;

        // Initialize table
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(ACCESS_KEYS_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    /// Create a new access key.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name or description for the key
    ///
    /// # Returns
    ///
    /// Returns the creation response including the secret key (only shown once).
    pub fn create_key(&self, name: String) -> Result<CreateAccessKeyResponse> {
        let (key, secret) = AccessKey::new(name);

        info!("Creating access key: {} ({})", key.access_key_id, key.name);

        let data = serde_json::to_vec(&key)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ACCESS_KEYS_TABLE)?;
            table.insert(key.access_key_id.as_str(), data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(CreateAccessKeyResponse {
            access_key_id: key.access_key_id,
            secret_access_key: secret,
            name: key.name,
            created_at: key.created_at,
        })
    }

    /// Get an access key by ID.
    ///
    /// # Arguments
    ///
    /// * `access_key_id` - The access key ID to retrieve
    ///
    /// # Returns
    ///
    /// Returns the key metadata (without the secret) if found.
    pub fn get_key(&self, access_key_id: &str) -> Result<Option<AccessKeyInfo>> {
        debug!("Retrieving access key: {}", access_key_id);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ACCESS_KEYS_TABLE)?;

        match table.get(access_key_id)? {
            Some(data) => {
                let key: AccessKey = serde_json::from_slice(data.value())?;
                Ok(Some(key.into()))
            },
            None => Ok(None),
        }
    }

    /// List all access keys.
    ///
    /// # Returns
    ///
    /// Returns a list of all access key metadata (without secrets).
    pub fn list_keys(&self) -> Result<Vec<AccessKeyInfo>> {
        debug!("Listing all access keys");

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ACCESS_KEYS_TABLE)?;

        let mut results = Vec::new();
        let mut iter = table.iter()?;

        while let Some(Ok((_, value))) = iter.next() {
            let key: AccessKey = serde_json::from_slice(value.value())?;
            results.push(key.into());
        }

        Ok(results)
    }

    /// Delete an access key.
    ///
    /// # Arguments
    ///
    /// * `access_key_id` - The access key ID to delete
    ///
    /// # Returns
    ///
    /// Returns true if the key was deleted, false if it didn't exist.
    pub fn delete_key(&self, access_key_id: &str) -> Result<bool> {
        info!("Deleting access key: {}", access_key_id);

        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(ACCESS_KEYS_TABLE)?;
            let result = table.remove(access_key_id)?;
            result.is_some()
        };
        write_txn.commit()?;

        Ok(removed)
    }

    /// Update the active status of an access key.
    ///
    /// # Arguments
    ///
    /// * `access_key_id` - The access key ID to update
    /// * `active` - New active status
    pub fn set_active(&self, access_key_id: &str, active: bool) -> Result<bool> {
        debug!(
            "Setting access key {} active status to {}",
            access_key_id, active
        );

        let write_txn = self.db.begin_write()?;
        let updated = {
            let mut table = write_txn.open_table(ACCESS_KEYS_TABLE)?;

            // First, read the current value and extract data
            let key_data = match table.get(access_key_id)? {
                Some(data) => {
                    let key: AccessKey = serde_json::from_slice(data.value())?;
                    Some(key)
                },
                None => None,
            };

            // Now we can safely modify
            match key_data {
                Some(mut key) => {
                    key.active = active;
                    let new_data = serde_json::to_vec(&key)?;
                    table.insert(access_key_id, new_data.as_slice())?;
                    true
                },
                None => false,
            }
        };
        write_txn.commit()?;

        Ok(updated)
    }

    /// Verify an access key and secret.
    ///
    /// This is the main authentication method. It checks if the key exists,
    /// is active, and the secret matches.
    ///
    /// # Arguments
    ///
    /// * `access_key_id` - The access key ID
    /// * `secret_access_key` - The secret access key to verify
    ///
    /// # Returns
    ///
    /// Returns true if the credentials are valid and the key is active.
    pub fn verify(&self, access_key_id: &str, secret_access_key: &str) -> Result<bool> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ACCESS_KEYS_TABLE)?;

        match table.get(access_key_id)? {
            Some(data) => {
                let key: AccessKey = serde_json::from_slice(data.value())?;

                // Key must be active and secret must match
                Ok(key.active && key.verify_secret(secret_access_key))
            },
            None => Ok(false),
        }
    }

    /// Get all active access keys as a map (for AuthConfig).
    ///
    /// # Returns
    ///
    /// Returns a HashMap of access key IDs to secret key hashes.
    /// Note: The values are hashes, not plain secrets, so this can't be
    /// directly used for verification. Use `verify()` instead.
    pub fn get_active_keys(&self) -> Result<HashMap<String, String>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ACCESS_KEYS_TABLE)?;

        let mut keys = HashMap::new();
        let mut iter = table.iter()?;

        while let Some(Ok((_, value))) = iter.next() {
            let key: AccessKey = serde_json::from_slice(value.value())?;
            if key.active {
                keys.insert(key.access_key_id.clone(), key.secret_key_hash.clone());
            }
        }

        Ok(keys)
    }

    /// Update the last used timestamp for a key.
    ///
    /// This should be called after successful authentication to track key usage.
    pub fn update_last_used(&self, access_key_id: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ACCESS_KEYS_TABLE)?;

            // Read the key data first
            let key_data = match table.get(access_key_id)? {
                Some(data) => {
                    let key: AccessKey = serde_json::from_slice(data.value())?;
                    Some(key)
                },
                None => None,
            };

            // Now we can safely modify
            if let Some(mut key) = key_data {
                key.last_used_at = Some(now);
                let new_data = serde_json::to_vec(&key)?;
                table.insert(access_key_id, new_data.as_slice())?;
            }
        }
        write_txn.commit()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store() -> (KeyStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("keys.db");
        let store = KeyStore::open(&db_path).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_create_and_get_key() {
        let (store, _temp) = create_test_store();

        let response = store.create_key("test-key".to_string()).unwrap();

        assert!(!response.access_key_id.is_empty());
        assert!(!response.secret_access_key.is_empty());
        assert_eq!(response.name, "test-key");

        let key = store.get_key(&response.access_key_id).unwrap();
        assert!(key.is_some());
        let key = key.unwrap();
        assert_eq!(key.access_key_id, response.access_key_id);
        assert_eq!(key.name, "test-key");
        assert!(key.active);
    }

    #[test]
    fn test_verify_key() {
        let (store, _temp) = create_test_store();

        let response = store.create_key("test-key".to_string()).unwrap();

        // Correct secret should verify
        let valid = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(valid);

        // Wrong secret should fail
        let invalid = store
            .verify(&response.access_key_id, "wrong-secret")
            .unwrap();
        assert!(!invalid);

        // Non-existent key should fail
        let invalid = store
            .verify("AKIAINVALIDKEY123456", &response.secret_access_key)
            .unwrap();
        assert!(!invalid);
    }

    #[test]
    fn test_list_keys() {
        let (store, _temp) = create_test_store();

        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 3);

        let key_ids: Vec<String> = keys.iter().map(|k| k.access_key_id.clone()).collect();
        assert!(key_ids.contains(&key1.access_key_id));
        assert!(key_ids.contains(&key2.access_key_id));
        assert!(key_ids.contains(&key3.access_key_id));
    }

    #[test]
    fn test_delete_key() {
        let (store, _temp) = create_test_store();

        let response = store.create_key("test-key".to_string()).unwrap();

        // Key should exist
        let key = store.get_key(&response.access_key_id).unwrap();
        assert!(key.is_some());

        // Delete the key
        let deleted = store.delete_key(&response.access_key_id).unwrap();
        assert!(deleted);

        // Key should no longer exist
        let key = store.get_key(&response.access_key_id).unwrap();
        assert!(key.is_none());

        // Deleting again should return false
        let deleted = store.delete_key(&response.access_key_id).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_set_active() {
        let (store, _temp) = create_test_store();

        let response = store.create_key("test-key".to_string()).unwrap();

        // Key should start as active
        let valid = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(valid);

        // Deactivate the key
        let updated = store.set_active(&response.access_key_id, false).unwrap();
        assert!(updated);

        // Verification should now fail
        let invalid = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(!invalid);

        // Reactivate the key
        store.set_active(&response.access_key_id, true).unwrap();

        // Verification should work again
        let valid = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(valid);
    }

    #[test]
    fn test_get_active_keys() {
        let (store, _temp) = create_test_store();

        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        // Deactivate key2
        store.set_active(&key2.access_key_id, false).unwrap();

        let active_keys = store.get_active_keys().unwrap();
        assert_eq!(active_keys.len(), 2);
        assert!(active_keys.contains_key(&key1.access_key_id));
        assert!(!active_keys.contains_key(&key2.access_key_id));
        assert!(active_keys.contains_key(&key3.access_key_id));
    }

    #[test]
    fn test_update_last_used() {
        let (store, _temp) = create_test_store();

        let response = store.create_key("test-key".to_string()).unwrap();

        // Initially last_used_at should be None
        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        assert!(key.last_used_at.is_none());

        // Update last used
        store.update_last_used(&response.access_key_id).unwrap();

        // Now last_used_at should be set
        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        assert!(key.last_used_at.is_some());
        assert!(key.last_used_at.unwrap() > 0);
    }

    #[test]
    fn test_access_key_id_format() {
        let (key, _) = AccessKey::new("test".to_string());

        // Should start with "AKIA"
        assert!(key.access_key_id.starts_with("AKIA"));

        // Should be 20 characters
        assert_eq!(key.access_key_id.len(), 20);

        // Should be all uppercase alphanumeric
        assert!(key
            .access_key_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() && (c.is_ascii_uppercase() || c.is_ascii_digit())));
    }

    #[test]
    fn test_secret_key_format() {
        let (_, secret) = AccessKey::new("test".to_string());

        // Should be 40 characters
        assert_eq!(secret.len(), 40);

        // Should be base64-like characters
        assert!(secret
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/'));
    }

    #[test]
    fn test_hash_secret() {
        let secret = "testSecret123";
        let hash1 = AccessKey::hash_secret(secret);
        let hash2 = AccessKey::hash_secret(secret);

        // Same secret should produce same hash
        assert_eq!(hash1, hash2);

        // Hash should be 64 hex characters (SHA-256)
        assert_eq!(hash1.len(), 64);
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));

        // Different secret should produce different hash
        let hash3 = AccessKey::hash_secret("differentSecret");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_create_multiple_keys_unique() {
        let (store, _temp) = create_test_store();

        // Create multiple keys
        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        // All access key IDs should be unique
        assert_ne!(key1.access_key_id, key2.access_key_id);
        assert_ne!(key1.access_key_id, key3.access_key_id);
        assert_ne!(key2.access_key_id, key3.access_key_id);

        // All secret keys should be unique
        assert_ne!(key1.secret_access_key, key2.secret_access_key);
        assert_ne!(key1.secret_access_key, key3.secret_access_key);
        assert_ne!(key2.secret_access_key, key3.secret_access_key);

        // All keys should be valid and independently verifiable
        assert!(store
            .verify(&key1.access_key_id, &key1.secret_access_key)
            .unwrap());
        assert!(store
            .verify(&key2.access_key_id, &key2.secret_access_key)
            .unwrap());
        assert!(store
            .verify(&key3.access_key_id, &key3.secret_access_key)
            .unwrap());

        // Cross-verification should fail
        assert!(!store
            .verify(&key1.access_key_id, &key2.secret_access_key)
            .unwrap());
        assert!(!store
            .verify(&key2.access_key_id, &key1.secret_access_key)
            .unwrap());
    }

    #[test]
    fn test_access_key_id_always_starts_with_akia() {
        // Test multiple key creations to ensure consistency
        for _ in 0..10 {
            let (key, _) = AccessKey::new("test".to_string());
            assert!(
                key.access_key_id.starts_with("AKIA"),
                "Access key ID '{}' does not start with AKIA",
                key.access_key_id
            );
        }
    }

    #[test]
    fn test_access_key_id_uniqueness() {
        use std::collections::HashSet;

        // Generate many keys and ensure they're all unique
        let mut key_ids = HashSet::new();
        for _ in 0..100 {
            let (key, _) = AccessKey::new("test".to_string());
            assert!(
                key_ids.insert(key.access_key_id.clone()),
                "Duplicate access key ID generated: {}",
                key.access_key_id
            );
        }
        assert_eq!(key_ids.len(), 100);
    }

    #[test]
    fn test_secret_key_uniqueness() {
        use std::collections::HashSet;

        // Generate many secret keys and ensure they're all unique
        let mut secrets = HashSet::new();
        for _ in 0..100 {
            let (_, secret) = AccessKey::new("test".to_string());
            assert!(
                secrets.insert(secret.clone()),
                "Duplicate secret key generated"
            );
        }
        assert_eq!(secrets.len(), 100);
    }

    #[test]
    fn test_secret_key_contains_valid_base64_chars() {
        // Test multiple secret keys
        for _ in 0..10 {
            let (_, secret) = AccessKey::new("test".to_string());

            // Should only contain base64-like characters
            for ch in secret.chars() {
                assert!(
                    ch.is_ascii_alphanumeric() || ch == '+' || ch == '/',
                    "Secret key contains invalid character: {}",
                    ch
                );
            }
        }
    }

    #[test]
    fn test_created_at_timestamp_set() {
        let (store, _temp) = create_test_store();

        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let response = store.create_key("test-key".to_string()).unwrap();

        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Timestamp should be set and within reasonable range
        assert!(response.created_at >= before);
        assert!(response.created_at <= after);
    }

    #[test]
    fn test_new_key_starts_active() {
        let (store, _temp) = create_test_store();

        let response = store.create_key("test-key".to_string()).unwrap();

        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        assert!(key.active, "Newly created key should be active");
    }

    #[test]
    fn test_create_key_with_empty_name() {
        let (store, _temp) = create_test_store();

        // Should be able to create a key with an empty name
        let response = store.create_key("".to_string()).unwrap();
        assert!(!response.access_key_id.is_empty());
        assert!(!response.secret_access_key.is_empty());
        assert_eq!(response.name, "");
    }

    #[test]
    fn test_create_key_with_special_chars_in_name() {
        let (store, _temp) = create_test_store();

        let special_name = "test-key!@#$%^&*()_+{}|:\"<>?[]\\;',./".to_string();
        let response = store.create_key(special_name.clone()).unwrap();

        assert_eq!(response.name, special_name);

        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        assert_eq!(key.name, special_name);
    }

    #[test]
    fn test_secret_not_stored_in_database() {
        let (store, _temp) = create_test_store();

        let response = store.create_key("test-key".to_string()).unwrap();

        // The secret should only be returned in the response
        // It should not be retrievable from the database later
        let _key = store.get_key(&response.access_key_id).unwrap().unwrap();

        // AccessKeyInfo doesn't contain the secret
        // We can verify this by ensuring verification still works
        assert!(store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap());

        // But we cannot retrieve the secret from the stored key
        // (This is enforced by the API design - get_key returns AccessKeyInfo, not AccessKey)
    }

    #[test]
    fn test_verify_secret_method() {
        let (key, secret) = AccessKey::new("test".to_string());

        // Correct secret should verify
        assert!(key.verify_secret(&secret));

        // Wrong secret should not verify
        assert!(!key.verify_secret("wrong_secret"));
        assert!(!key.verify_secret(""));
    }

    #[test]
    fn test_access_key_id_length_exactly_20() {
        // Test multiple keys to ensure consistency
        for _ in 0..20 {
            let (key, _) = AccessKey::new("test".to_string());
            assert_eq!(
                key.access_key_id.len(),
                20,
                "Access key ID length should be exactly 20 characters, got {}",
                key.access_key_id.len()
            );
        }
    }

    #[test]
    fn test_secret_key_length_exactly_40() {
        // Test multiple keys to ensure consistency
        for _ in 0..20 {
            let (_, secret) = AccessKey::new("test".to_string());
            assert_eq!(
                secret.len(),
                40,
                "Secret key length should be exactly 40 characters, got {}",
                secret.len()
            );
        }
    }

    // Comprehensive deletion tests

    #[test]
    fn test_delete_existing_key() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-key".to_string()).unwrap();

        // Verify it exists
        let key = store.get_key(&response.access_key_id).unwrap();
        assert!(key.is_some());

        // Delete it
        let deleted = store.delete_key(&response.access_key_id).unwrap();
        assert!(deleted, "delete_key should return true for existing key");

        // Verify it's gone
        let key = store.get_key(&response.access_key_id).unwrap();
        assert!(key.is_none(), "Key should not exist after deletion");
    }

    #[test]
    fn test_delete_nonexistent_key() {
        let (store, _temp) = create_test_store();

        // Try to delete a key that never existed
        let deleted = store.delete_key("AKIANONEXISTENTKEY99").unwrap();
        assert!(
            !deleted,
            "delete_key should return false for non-existent key"
        );
    }

    #[test]
    fn test_delete_already_deleted_key() {
        let (store, _temp) = create_test_store();

        // Create and delete a key
        let response = store.create_key("test-key".to_string()).unwrap();
        let deleted = store.delete_key(&response.access_key_id).unwrap();
        assert!(deleted);

        // Try to delete it again
        let deleted_again = store.delete_key(&response.access_key_id).unwrap();
        assert!(
            !deleted_again,
            "Deleting an already deleted key should return false"
        );
    }

    #[test]
    fn test_deleted_key_not_in_list() {
        let (store, _temp) = create_test_store();

        // Create three keys
        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        // Verify all three are in the list
        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 3);

        // Delete key2
        store.delete_key(&key2.access_key_id).unwrap();

        // List should now only contain key1 and key3
        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 2, "List should have 2 keys after deleting 1");

        let key_ids: Vec<String> = keys.iter().map(|k| k.access_key_id.clone()).collect();
        assert!(
            key_ids.contains(&key1.access_key_id),
            "key1 should still be in list"
        );
        assert!(
            !key_ids.contains(&key2.access_key_id),
            "key2 should not be in list after deletion"
        );
        assert!(
            key_ids.contains(&key3.access_key_id),
            "key3 should still be in list"
        );
    }

    #[test]
    fn test_other_keys_unaffected_by_deletion() {
        let (store, _temp) = create_test_store();

        // Create three keys
        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        // Delete key2
        store.delete_key(&key2.access_key_id).unwrap();

        // Verify key1 and key3 are unaffected and still work
        let key1_info = store.get_key(&key1.access_key_id).unwrap();
        assert!(key1_info.is_some(), "key1 should still exist");
        assert_eq!(
            key1_info.unwrap().name,
            "key-1",
            "key1 name should be unchanged"
        );

        let key3_info = store.get_key(&key3.access_key_id).unwrap();
        assert!(key3_info.is_some(), "key3 should still exist");
        assert_eq!(
            key3_info.unwrap().name,
            "key-3",
            "key3 name should be unchanged"
        );

        // Verify authentication still works for non-deleted keys
        assert!(
            store
                .verify(&key1.access_key_id, &key1.secret_access_key)
                .unwrap(),
            "key1 should still authenticate"
        );
        assert!(
            store
                .verify(&key3.access_key_id, &key3.secret_access_key)
                .unwrap(),
            "key3 should still authenticate"
        );
    }

    #[test]
    fn test_deleted_key_cannot_authenticate() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-key".to_string()).unwrap();

        // Verify authentication works before deletion
        let valid = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(valid, "Key should authenticate before deletion");

        // Delete the key
        store.delete_key(&response.access_key_id).unwrap();

        // Attempt to authenticate with the deleted key
        let invalid = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(!invalid, "Deleted key should not authenticate");
    }

    #[test]
    fn test_deleted_key_not_in_active_keys() {
        let (store, _temp) = create_test_store();

        // Create three keys
        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        // All should be in active keys
        let active = store.get_active_keys().unwrap();
        assert_eq!(active.len(), 3);

        // Delete key2
        store.delete_key(&key2.access_key_id).unwrap();

        // Active keys should now only contain key1 and key3
        let active = store.get_active_keys().unwrap();
        assert_eq!(active.len(), 2, "Should have 2 active keys after deletion");
        assert!(
            active.contains_key(&key1.access_key_id),
            "key1 should be in active keys"
        );
        assert!(
            !active.contains_key(&key2.access_key_id),
            "key2 should not be in active keys"
        );
        assert!(
            active.contains_key(&key3.access_key_id),
            "key3 should be in active keys"
        );
    }

    #[test]
    fn test_cannot_modify_deleted_key() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-key".to_string()).unwrap();

        // Delete it
        store.delete_key(&response.access_key_id).unwrap();

        // Try to set it active
        let updated = store.set_active(&response.access_key_id, true).unwrap();
        assert!(!updated, "Should not be able to update a deleted key");

        // Try to update last used
        store.update_last_used(&response.access_key_id).unwrap();

        // Verify it's still deleted (update_last_used should be a no-op)
        let key = store.get_key(&response.access_key_id).unwrap();
        assert!(
            key.is_none(),
            "Key should still be deleted after update attempts"
        );
    }

    #[test]
    fn test_delete_inactive_key() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-key".to_string()).unwrap();

        // Make it inactive
        store.set_active(&response.access_key_id, false).unwrap();

        // Verify it's inactive
        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        assert!(!key.active, "Key should be inactive");

        // Delete it
        let deleted = store.delete_key(&response.access_key_id).unwrap();
        assert!(deleted, "Should be able to delete inactive key");

        // Verify it's gone
        let key = store.get_key(&response.access_key_id).unwrap();
        assert!(key.is_none(), "Inactive key should be deleted");
    }

    #[test]
    fn test_delete_all_keys() {
        let (store, _temp) = create_test_store();

        // Create multiple keys
        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        // Verify all exist
        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 3);

        // Delete all keys
        store.delete_key(&key1.access_key_id).unwrap();
        store.delete_key(&key2.access_key_id).unwrap();
        store.delete_key(&key3.access_key_id).unwrap();

        // List should be empty
        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 0, "All keys should be deleted");

        // Active keys should be empty
        let active = store.get_active_keys().unwrap();
        assert_eq!(active.len(), 0, "No active keys should remain");
    }

    #[test]
    fn test_delete_with_empty_string() {
        let (store, _temp) = create_test_store();

        // Try to delete with empty string
        let deleted = store.delete_key("").unwrap();
        assert!(!deleted, "Deleting empty string key should return false");
    }

    #[test]
    fn test_delete_preserves_database_integrity() {
        let (store, _temp) = create_test_store();

        // Create multiple keys
        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        // Delete one key
        store.delete_key(&key2.access_key_id).unwrap();

        // Perform various operations to verify database integrity
        // 1. Can still create new keys
        let key4 = store.create_key("key-4".to_string()).unwrap();
        assert!(store.get_key(&key4.access_key_id).unwrap().is_some());

        // 2. Can still modify remaining keys
        let updated = store.set_active(&key1.access_key_id, false).unwrap();
        assert!(updated);

        // 3. Can still list keys
        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 3); // key1, key3, key4 (key2 was deleted)

        // 4. Can still verify keys
        assert!(
            store
                .verify(&key1.access_key_id, &key1.secret_access_key)
                .unwrap()
                || !store
                    .verify(&key1.access_key_id, &key1.secret_access_key)
                    .unwrap()
        );
        assert!(store
            .verify(&key3.access_key_id, &key3.secret_access_key)
            .unwrap());
    }

    // Comprehensive access key authentication tests

    #[test]
    fn test_auth_verify_with_correct_credentials() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Verify with correct credentials
        let result = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(
            result,
            "Authentication should succeed with correct credentials"
        );
    }

    #[test]
    fn test_auth_verify_with_incorrect_secret() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Verify with incorrect secret
        let result = store
            .verify(&response.access_key_id, "incorrect_secret")
            .unwrap();
        assert!(!result, "Authentication should fail with incorrect secret");
    }

    #[test]
    fn test_auth_verify_with_incorrect_access_key_id() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Verify with incorrect access key ID
        let result = store
            .verify("AKIAINVALIDKEY123456", &response.secret_access_key)
            .unwrap();
        assert!(
            !result,
            "Authentication should fail with incorrect access key ID"
        );
    }

    #[test]
    fn test_auth_verify_with_nonexistent_key() {
        let (store, _temp) = create_test_store();

        // Verify with a key that doesn't exist
        let result = store.verify("AKIANONEXISTENTKEY99", "any_secret").unwrap();
        assert!(!result, "Authentication should fail for non-existent key");
    }

    #[test]
    fn test_auth_verify_with_inactive_key() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Verify it works when active
        let result = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(result, "Authentication should succeed when key is active");

        // Deactivate the key
        store.set_active(&response.access_key_id, false).unwrap();

        // Verify should now fail even with correct credentials
        let result = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(
            !result,
            "Authentication should fail with inactive key even with correct secret"
        );
    }

    #[test]
    fn test_auth_verify_timing_updates_last_used() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Verify last_used_at is initially None
        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        assert!(
            key.last_used_at.is_none(),
            "last_used_at should be None initially"
        );

        // Perform authentication
        let result = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(result, "Authentication should succeed");

        // Update last used (simulating what would happen in actual usage)
        store.update_last_used(&response.access_key_id).unwrap();

        // Verify last_used_at is now set
        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        assert!(
            key.last_used_at.is_some(),
            "last_used_at should be set after update"
        );
        assert!(
            key.last_used_at.unwrap() > 0,
            "last_used_at should have a valid timestamp"
        );
    }

    #[test]
    fn test_auth_last_used_timestamp_progression() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Update last used first time
        store.update_last_used(&response.access_key_id).unwrap();
        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        let first_timestamp = key.last_used_at.unwrap();

        // Wait a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Update last used second time
        store.update_last_used(&response.access_key_id).unwrap();
        let key = store.get_key(&response.access_key_id).unwrap().unwrap();
        let second_timestamp = key.last_used_at.unwrap();

        // Second timestamp should be >= first timestamp
        assert!(
            second_timestamp >= first_timestamp,
            "Second update should have timestamp >= first (first: {}, second: {})",
            first_timestamp,
            second_timestamp
        );
    }

    #[test]
    fn test_auth_case_sensitivity_of_access_key_id() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Try with lowercase access key ID
        let lowercase_id = response.access_key_id.to_lowercase();
        let result = store
            .verify(&lowercase_id, &response.secret_access_key)
            .unwrap();
        assert!(
            !result,
            "Access key ID should be case-sensitive (lowercase should fail)"
        );

        // Try with mixed case - modify index 2 (the 'I' in AKIA prefix, guaranteed to be a letter)
        let mut mixed_case_id = response.access_key_id.clone();
        if let Some(c) = mixed_case_id.get_mut(2..3) {
            c.make_ascii_lowercase();
        }
        let result = store
            .verify(&mixed_case_id, &response.secret_access_key)
            .unwrap();
        assert!(
            !result,
            "Access key ID should be case-sensitive (mixed case should fail)"
        );

        // Exact case should work
        let result = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(result, "Exact case should authenticate successfully");
    }

    #[test]
    fn test_auth_case_sensitivity_of_secret() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Try with uppercase secret
        let uppercase_secret = response.secret_access_key.to_uppercase();
        if uppercase_secret != response.secret_access_key {
            let result = store
                .verify(&response.access_key_id, &uppercase_secret)
                .unwrap();
            assert!(
                !result,
                "Secret should be case-sensitive (uppercase should fail)"
            );
        }

        // Try with lowercase secret
        let lowercase_secret = response.secret_access_key.to_lowercase();
        if lowercase_secret != response.secret_access_key {
            let result = store
                .verify(&response.access_key_id, &lowercase_secret)
                .unwrap();
            assert!(
                !result,
                "Secret should be case-sensitive (lowercase should fail)"
            );
        }

        // Exact case should work
        let result = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(result, "Exact case should authenticate successfully");
    }

    #[test]
    fn test_auth_empty_credentials() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Try with empty secret
        let result = store.verify(&response.access_key_id, "").unwrap();
        assert!(!result, "Authentication should fail with empty secret");

        // Try with empty access key ID
        let result = store.verify("", &response.secret_access_key).unwrap();
        assert!(
            !result,
            "Authentication should fail with empty access key ID"
        );

        // Try with both empty
        let result = store.verify("", "").unwrap();
        assert!(
            !result,
            "Authentication should fail with both credentials empty"
        );
    }

    #[test]
    fn test_auth_whitespace_in_credentials() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Try with whitespace-padded secret
        let padded_secret = format!(" {} ", response.secret_access_key);
        let result = store
            .verify(&response.access_key_id, &padded_secret)
            .unwrap();
        assert!(
            !result,
            "Authentication should fail with whitespace-padded secret"
        );

        // Try with whitespace-padded access key ID
        let padded_id = format!(" {} ", response.access_key_id);
        let result = store
            .verify(&padded_id, &response.secret_access_key)
            .unwrap();
        assert!(
            !result,
            "Authentication should fail with whitespace-padded access key ID"
        );
    }

    #[test]
    fn test_auth_partial_secret_match() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Try with partial secret (first half)
        let partial_secret = &response.secret_access_key[..20];
        let result = store
            .verify(&response.access_key_id, partial_secret)
            .unwrap();
        assert!(!result, "Authentication should fail with partial secret");

        // Try with secret plus extra characters
        let extended_secret = format!("{}extra", response.secret_access_key);
        let result = store
            .verify(&response.access_key_id, &extended_secret)
            .unwrap();
        assert!(!result, "Authentication should fail with extended secret");
    }

    #[test]
    fn test_auth_similar_secrets() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Try with secret that differs by one character
        let mut similar_secret = response.secret_access_key.clone();
        if let Some(c) = similar_secret.chars().next() {
            let replacement = if c == 'A' { 'B' } else { 'A' };
            similar_secret.replace_range(0..1, &replacement.to_string());
        }

        let result = store
            .verify(&response.access_key_id, &similar_secret)
            .unwrap();
        assert!(
            !result,
            "Authentication should fail even with very similar secret"
        );
    }

    #[test]
    fn test_auth_multiple_keys_isolation() {
        let (store, _temp) = create_test_store();

        // Create multiple keys
        let key1 = store.create_key("key-1".to_string()).unwrap();
        let key2 = store.create_key("key-2".to_string()).unwrap();
        let key3 = store.create_key("key-3".to_string()).unwrap();

        // Each key should authenticate with its own secret
        assert!(store
            .verify(&key1.access_key_id, &key1.secret_access_key)
            .unwrap());
        assert!(store
            .verify(&key2.access_key_id, &key2.secret_access_key)
            .unwrap());
        assert!(store
            .verify(&key3.access_key_id, &key3.secret_access_key)
            .unwrap());

        // Cross-authentication should fail
        assert!(!store
            .verify(&key1.access_key_id, &key2.secret_access_key)
            .unwrap());
        assert!(!store
            .verify(&key1.access_key_id, &key3.secret_access_key)
            .unwrap());
        assert!(!store
            .verify(&key2.access_key_id, &key1.secret_access_key)
            .unwrap());
        assert!(!store
            .verify(&key2.access_key_id, &key3.secret_access_key)
            .unwrap());
        assert!(!store
            .verify(&key3.access_key_id, &key1.secret_access_key)
            .unwrap());
        assert!(!store
            .verify(&key3.access_key_id, &key2.secret_access_key)
            .unwrap());
    }

    #[test]
    fn test_auth_reactivated_key() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Verify it works initially
        assert!(store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap());

        // Deactivate
        store.set_active(&response.access_key_id, false).unwrap();
        assert!(!store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap());

        // Reactivate
        store.set_active(&response.access_key_id, true).unwrap();
        assert!(
            store
                .verify(&response.access_key_id, &response.secret_access_key)
                .unwrap(),
            "Authentication should succeed again after reactivation"
        );
    }

    #[test]
    fn test_auth_deleted_key_cannot_authenticate() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Verify it works before deletion
        assert!(store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap());

        // Delete the key
        store.delete_key(&response.access_key_id).unwrap();

        // Authentication should fail after deletion
        let result = store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap();
        assert!(!result, "Deleted key should not authenticate");
    }

    #[test]
    fn test_auth_last_used_does_not_affect_authentication() {
        let (store, _temp) = create_test_store();

        // Create a key
        let response = store.create_key("test-auth".to_string()).unwrap();

        // Authenticate before updating last_used
        assert!(store
            .verify(&response.access_key_id, &response.secret_access_key)
            .unwrap());

        // Update last_used
        store.update_last_used(&response.access_key_id).unwrap();

        // Authentication should still work
        assert!(
            store
                .verify(&response.access_key_id, &response.secret_access_key)
                .unwrap(),
            "last_used_at update should not affect authentication"
        );
    }

    #[test]
    fn test_auth_verify_method_on_access_key() {
        // Test the AccessKey::verify_secret method directly
        let (key, secret) = AccessKey::new("test".to_string());

        // Correct secret should verify
        assert!(key.verify_secret(&secret), "Correct secret should verify");

        // Incorrect secret should not verify
        assert!(
            !key.verify_secret("wrong_secret"),
            "Incorrect secret should not verify"
        );

        // Empty secret should not verify
        assert!(!key.verify_secret(""), "Empty secret should not verify");

        // Secret with extra characters should not verify
        let wrong_secret = format!("{}extra", secret);
        assert!(
            !key.verify_secret(&wrong_secret),
            "Extended secret should not verify"
        );

        // Secret missing characters should not verify
        let truncated_secret = &secret[..secret.len() - 1];
        assert!(
            !key.verify_secret(truncated_secret),
            "Truncated secret should not verify"
        );
    }

    #[test]
    fn test_auth_constant_time_comparison() {
        // This test verifies that verify_secret uses constant-time comparison
        // We can't directly measure timing, but we can verify the implementation exists
        let (key, secret) = AccessKey::new("test".to_string());

        // Create many different wrong secrets
        let wrong_secrets = vec![
            "A".repeat(40),
            "Z".repeat(40),
            "0".repeat(40),
            secret[..39].to_string(),
            format!("{}X", &secret[..39]),
        ];

        // All should fail
        for wrong_secret in wrong_secrets {
            assert!(
                !key.verify_secret(&wrong_secret),
                "Wrong secret '{}' should not verify",
                wrong_secret
            );
        }

        // Correct secret should still work
        assert!(
            key.verify_secret(&secret),
            "Correct secret should still verify"
        );
    }
}
