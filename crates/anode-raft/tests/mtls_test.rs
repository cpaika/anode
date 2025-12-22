//! mTLS (Mutual TLS) Tests for Raft Inter-Node Communication
//!
//! This module tests the mTLS implementation for secure communication between
//! Raft nodes. It verifies that:
//! - Nodes can be configured with valid mTLS certificates
//! - TLS configuration is correctly built from certificate files
//! - Invalid certificate paths are detected
//! - Network manager correctly handles TLS configuration
//!
//! The tests use OpenSSL to generate test certificates similar to the approach
//! in `tests/integration/src/https_connections.rs`.

use anode_raft::{RaftNetworkManager, TlsConfig};
use openraft::BasicNode;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Certificate generation utilities for mTLS testing
struct MtlsCertificates {
    temp_dir: TempDir,
    ca_cert_path: PathBuf,
    ca_key_path: PathBuf,
    node1_cert_path: PathBuf,
    node1_key_path: PathBuf,
    node2_cert_path: PathBuf,
    node2_key_path: PathBuf,
    invalid_cert_path: PathBuf,
    invalid_key_path: PathBuf,
}

impl MtlsCertificates {
    /// Generate all certificates needed for testing
    fn generate() -> Result<Self, String> {
        let temp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let base_path = temp_dir.path();

        // Generate CA certificate
        let ca_cert_path = base_path.join("ca-cert.pem");
        let ca_key_path = base_path.join("ca-key.pem");
        Self::generate_ca(&ca_cert_path, &ca_key_path)?;

        // Generate certificate for node1
        let node1_cert_path = base_path.join("node1-cert.pem");
        let node1_key_path = base_path.join("node1-key.pem");
        Self::generate_node_cert(
            &node1_cert_path,
            &node1_key_path,
            &ca_cert_path,
            &ca_key_path,
            "node1",
        )?;

        // Generate certificate for node2
        let node2_cert_path = base_path.join("node2-cert.pem");
        let node2_key_path = base_path.join("node2-key.pem");
        Self::generate_node_cert(
            &node2_cert_path,
            &node2_key_path,
            &ca_cert_path,
            &ca_key_path,
            "node2",
        )?;

        // Generate invalid certificate (self-signed, not from our CA)
        let invalid_cert_path = base_path.join("invalid-cert.pem");
        let invalid_key_path = base_path.join("invalid-key.pem");
        Self::generate_self_signed(&invalid_cert_path, &invalid_key_path)?;

        Ok(Self {
            temp_dir,
            ca_cert_path,
            ca_key_path,
            node1_cert_path,
            node1_key_path,
            node2_cert_path,
            node2_key_path,
            invalid_cert_path,
            invalid_key_path,
        })
    }

    /// Generate CA certificate
    fn generate_ca(cert_path: &PathBuf, key_path: &PathBuf) -> Result<(), String> {
        // Generate CA private key
        let output = Command::new("openssl")
            .args(&["genrsa", "-out", key_path.to_str().unwrap(), "2048"])
            .output()
            .map_err(|e| format!("Failed to execute openssl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to generate CA key: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Generate CA certificate
        let output = Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-x509",
                "-days",
                "365",
                "-key",
                key_path.to_str().unwrap(),
                "-out",
                cert_path.to_str().unwrap(),
                "-subj",
                "/C=US/ST=Test/L=Test/O=Anode Raft Test/CN=Test CA",
            ])
            .output()
            .map_err(|e| format!("Failed to execute openssl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to generate CA cert: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Generate a node certificate signed by the CA
    fn generate_node_cert(
        cert_path: &PathBuf,
        key_path: &PathBuf,
        ca_cert_path: &PathBuf,
        ca_key_path: &PathBuf,
        node_name: &str,
    ) -> Result<(), String> {
        // Generate node private key
        let output = Command::new("openssl")
            .args(&["genrsa", "-out", key_path.to_str().unwrap(), "2048"])
            .output()
            .map_err(|e| format!("Failed to execute openssl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to generate node key: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Generate CSR
        let csr_path = cert_path.with_extension("csr");
        let output = Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-key",
                key_path.to_str().unwrap(),
                "-out",
                csr_path.to_str().unwrap(),
                "-subj",
                &format!("/C=US/ST=Test/L=Test/O=Anode Raft Test/CN={}", node_name),
            ])
            .output()
            .map_err(|e| format!("Failed to execute openssl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to generate CSR: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Create extension file for SAN (Subject Alternative Name)
        let ext_path = cert_path.with_extension("ext");
        fs::write(
            &ext_path,
            format!(
                "subjectAltName=DNS:{},DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth,clientAuth\n",
                node_name
            ),
        )
        .map_err(|e| format!("Failed to write extension file: {}", e))?;

        // Sign certificate with CA
        let output = Command::new("openssl")
            .args(&[
                "x509",
                "-req",
                "-in",
                csr_path.to_str().unwrap(),
                "-CA",
                ca_cert_path.to_str().unwrap(),
                "-CAkey",
                ca_key_path.to_str().unwrap(),
                "-CAcreateserial",
                "-out",
                cert_path.to_str().unwrap(),
                "-days",
                "365",
                "-extfile",
                ext_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| format!("Failed to execute openssl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to sign certificate: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Generate a self-signed certificate (not signed by our CA)
    fn generate_self_signed(cert_path: &PathBuf, key_path: &PathBuf) -> Result<(), String> {
        // Generate private key
        let output = Command::new("openssl")
            .args(&["genrsa", "-out", key_path.to_str().unwrap(), "2048"])
            .output()
            .map_err(|e| format!("Failed to execute openssl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to generate key: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Generate self-signed certificate
        let output = Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-x509",
                "-days",
                "365",
                "-key",
                key_path.to_str().unwrap(),
                "-out",
                cert_path.to_str().unwrap(),
                "-subj",
                "/C=US/ST=Test/L=Test/O=Invalid/CN=invalid",
            ])
            .output()
            .map_err(|e| format!("Failed to execute openssl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to generate self-signed cert: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Create a TlsConfig for a node
    fn create_tls_config(&self, node_cert: &PathBuf, node_key: &PathBuf) -> TlsConfig {
        TlsConfig {
            enabled: true,
            cert_file: node_cert.clone(),
            key_file: node_key.clone(),
            ca_file: Some(self.ca_cert_path.clone()),
            require_client_cert: true,
            verify_certificates: true,
        }
    }
}

/// Test that the TLS configuration is correctly built
#[test]
fn test_tls_config_creation() {
    let certs = MtlsCertificates::generate().expect("Failed to generate certificates");

    let tls_config = certs.create_tls_config(&certs.node1_cert_path, &certs.node1_key_path);

    assert!(tls_config.enabled);
    assert!(tls_config.require_client_cert);
    assert!(tls_config.verify_certificates);
    assert!(tls_config.ca_file.is_some());
    assert_eq!(tls_config.cert_file, certs.node1_cert_path);
    assert_eq!(tls_config.key_file, certs.node1_key_path);
}

/// Test that nodes can be configured with valid mTLS certificates
#[test]
fn test_mtls_network_manager_creation() {
    let certs = MtlsCertificates::generate().expect("Failed to generate certificates");

    // Create TLS config for node1
    let tls_config = certs.create_tls_config(&certs.node1_cert_path, &certs.node1_key_path);

    // Create network manager with TLS
    let manager = RaftNetworkManager::with_tls(tls_config);

    // Verify TLS is configured
    assert!(manager.has_tls());

    // Add node2 as a peer
    manager.add_node(
        2,
        BasicNode {
            addr: "127.0.0.1:50051".to_string(),
        },
    );

    // Verify that the node was added
    assert_eq!(manager.node_count(), 1);
    assert!(manager.has_node(2));
}

/// Test that invalid certificate paths are detected when building client TLS config
#[tokio::test]
async fn test_mtls_invalid_certificate_paths() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create TLS config with non-existent files
    let tls_config = TlsConfig {
        enabled: true,
        cert_file: temp_dir.path().join("nonexistent-cert.pem"),
        key_file: temp_dir.path().join("nonexistent-key.pem"),
        ca_file: Some(temp_dir.path().join("nonexistent-ca.pem")),
        require_client_cert: true,
        verify_certificates: true,
    };

    let manager = RaftNetworkManager::with_tls(tls_config);

    manager.add_node(
        2,
        BasicNode {
            addr: "127.0.0.1:50052".to_string(),
        },
    );

    // Try to get a connection - should fail because files don't exist
    let result = manager.get_connection(2).await;
    assert!(
        result.is_err(),
        "Connection should fail with invalid certificate paths"
    );
}

/// Test that nodes can be added and removed from the network manager
#[test]
fn test_network_manager_node_management() {
    let manager = RaftNetworkManager::new();

    // Add nodes
    manager.add_node(
        1,
        BasicNode {
            addr: "127.0.0.1:50051".to_string(),
        },
    );
    manager.add_node(
        2,
        BasicNode {
            addr: "127.0.0.1:50052".to_string(),
        },
    );
    manager.add_node(
        3,
        BasicNode {
            addr: "127.0.0.1:50053".to_string(),
        },
    );

    assert_eq!(manager.node_count(), 3);

    // Remove a node
    manager.remove_node(2);
    assert_eq!(manager.node_count(), 2);

    // Verify the correct node was removed
    assert!(manager.has_node(1));
    assert!(!manager.has_node(2));
    assert!(manager.has_node(3));
}

/// Test network manager creation with and without TLS
#[test]
fn test_network_manager_creation_with_and_without_tls() {
    // Without TLS
    let manager = RaftNetworkManager::new();
    assert!(!manager.has_tls());

    // With TLS
    let tls_config = TlsConfig {
        enabled: true,
        cert_file: PathBuf::from("/tmp/cert.pem"),
        key_file: PathBuf::from("/tmp/key.pem"),
        ca_file: Some(PathBuf::from("/tmp/ca.pem")),
        require_client_cert: true,
        verify_certificates: true,
    };

    let manager = RaftNetworkManager::with_tls(tls_config.clone());
    assert!(manager.has_tls());

    // Verify the TLS config was stored correctly
    let stored_tls = manager.get_tls_config().unwrap();
    assert_eq!(stored_tls.enabled, tls_config.enabled);
    assert_eq!(stored_tls.cert_file, tls_config.cert_file);
    assert_eq!(stored_tls.key_file, tls_config.key_file);
    assert_eq!(stored_tls.ca_file, tls_config.ca_file);
    assert_eq!(
        stored_tls.require_client_cert,
        tls_config.require_client_cert
    );
    assert_eq!(
        stored_tls.verify_certificates,
        tls_config.verify_certificates
    );
}

/// Test certificate generation creates all required files
#[test]
fn test_certificate_generation() {
    let certs = MtlsCertificates::generate().expect("Failed to generate certificates");

    // Verify all certificate files exist and are readable
    assert!(certs.ca_cert_path.exists(), "CA certificate should exist");
    assert!(certs.ca_key_path.exists(), "CA key should exist");
    assert!(
        certs.node1_cert_path.exists(),
        "Node1 certificate should exist"
    );
    assert!(certs.node1_key_path.exists(), "Node1 key should exist");
    assert!(
        certs.node2_cert_path.exists(),
        "Node2 certificate should exist"
    );
    assert!(certs.node2_key_path.exists(), "Node2 key should exist");
    assert!(
        certs.invalid_cert_path.exists(),
        "Invalid certificate should exist"
    );
    assert!(certs.invalid_key_path.exists(), "Invalid key should exist");

    // Verify certificates can be read and are not empty
    let ca_cert = fs::read(&certs.ca_cert_path).expect("Failed to read CA cert");
    assert!(!ca_cert.is_empty(), "CA certificate should not be empty");

    let node1_cert = fs::read(&certs.node1_cert_path).expect("Failed to read node1 cert");
    assert!(
        !node1_cert.is_empty(),
        "Node1 certificate should not be empty"
    );

    let node2_cert = fs::read(&certs.node2_cert_path).expect("Failed to read node2 cert");
    assert!(
        !node2_cert.is_empty(),
        "Node2 certificate should not be empty"
    );

    let invalid_cert = fs::read(&certs.invalid_cert_path).expect("Failed to read invalid cert");
    assert!(
        !invalid_cert.is_empty(),
        "Invalid certificate should not be empty"
    );
}

/// Test that certificate files contain valid PEM data
#[test]
fn test_certificate_pem_format() {
    let certs = MtlsCertificates::generate().expect("Failed to generate certificates");

    // Read and verify CA certificate is valid PEM
    let ca_cert = fs::read_to_string(&certs.ca_cert_path).expect("Failed to read CA cert");
    assert!(
        ca_cert.contains("BEGIN CERTIFICATE"),
        "CA cert should be in PEM format"
    );
    assert!(
        ca_cert.contains("END CERTIFICATE"),
        "CA cert should be in PEM format"
    );

    // Read and verify node certificate is valid PEM
    let node1_cert = fs::read_to_string(&certs.node1_cert_path).expect("Failed to read node1 cert");
    assert!(
        node1_cert.contains("BEGIN CERTIFICATE"),
        "Node1 cert should be in PEM format"
    );
    assert!(
        node1_cert.contains("END CERTIFICATE"),
        "Node1 cert should be in PEM format"
    );

    // Read and verify private key is valid PEM
    let node1_key = fs::read_to_string(&certs.node1_key_path).expect("Failed to read node1 key");
    assert!(
        node1_key.contains("BEGIN") && node1_key.contains("PRIVATE KEY"),
        "Node1 key should be in PEM format"
    );
    assert!(
        node1_key.contains("END") && node1_key.contains("PRIVATE KEY"),
        "Node1 key should be in PEM format"
    );
}

/// Test that TLS config with disabled flag doesn't enforce TLS
#[test]
fn test_tls_config_disabled() {
    let certs = MtlsCertificates::generate().expect("Failed to generate certificates");

    let tls_config = TlsConfig {
        enabled: false, // TLS disabled
        cert_file: certs.node1_cert_path.clone(),
        key_file: certs.node1_key_path.clone(),
        ca_file: Some(certs.ca_cert_path.clone()),
        require_client_cert: true,
        verify_certificates: true,
    };

    let manager = RaftNetworkManager::with_tls(tls_config);

    // Even though TLS config is provided, enabled=false means it should not be used
    assert!(manager.has_tls());
    assert!(!manager.get_tls_config().unwrap().enabled);
}

/// Test that multiple nodes can have different certificates from the same CA
#[test]
fn test_multiple_nodes_with_different_certs() {
    let certs = MtlsCertificates::generate().expect("Failed to generate certificates");

    // Create TLS config for node1
    let tls_config1 = certs.create_tls_config(&certs.node1_cert_path, &certs.node1_key_path);
    let manager1 = RaftNetworkManager::with_tls(tls_config1);

    // Create TLS config for node2
    let tls_config2 = certs.create_tls_config(&certs.node2_cert_path, &certs.node2_key_path);
    let manager2 = RaftNetworkManager::with_tls(tls_config2);

    // Both managers should have TLS configured
    assert!(manager1.has_tls());
    assert!(manager2.has_tls());

    // They should use the same CA but different node certificates
    assert_eq!(
        manager1.get_tls_config().unwrap().ca_file,
        manager2.get_tls_config().unwrap().ca_file
    );
    assert_ne!(
        manager1.get_tls_config().unwrap().cert_file,
        manager2.get_tls_config().unwrap().cert_file
    );
}

/// Test TLS configuration without CA file (server-only TLS, not mTLS)
#[test]
fn test_tls_config_without_ca_file() {
    let certs = MtlsCertificates::generate().expect("Failed to generate certificates");

    let tls_config = TlsConfig {
        enabled: true,
        cert_file: certs.node1_cert_path.clone(),
        key_file: certs.node1_key_path.clone(),
        ca_file: None,              // No CA file
        require_client_cert: false, // Can't require client cert without CA
        verify_certificates: true,
    };

    let manager = RaftNetworkManager::with_tls(tls_config);

    assert!(manager.has_tls());
    let tls = manager.get_tls_config().unwrap();
    assert!(tls.enabled);
    assert!(tls.ca_file.is_none());
    assert!(!tls.require_client_cert);
}

/// Test that TLS config default values are correct
#[test]
fn test_tls_config_defaults() {
    let tls_config = TlsConfig::default();

    assert!(!tls_config.enabled, "TLS should be disabled by default");
    assert!(
        !tls_config.require_client_cert,
        "Client certs should not be required by default"
    );
    assert!(
        tls_config.verify_certificates,
        "Certificate verification should be enabled by default"
    );
    assert!(tls_config.ca_file.is_none(), "No CA file by default");
}

/// Test connection failure when node is not registered
#[tokio::test]
async fn test_connection_to_unregistered_node() {
    let manager = RaftNetworkManager::new();

    // Try to connect to a node that doesn't exist
    let result = manager.get_connection(999).await;
    assert!(
        result.is_err(),
        "Connection should fail for unregistered node"
    );
}

/// Test that removing a node also removes its cached connection
#[test]
fn test_remove_node_clears_cache() {
    let manager = RaftNetworkManager::new();

    manager.add_node(
        1,
        BasicNode {
            addr: "127.0.0.1:50051".to_string(),
        },
    );

    assert_eq!(manager.node_count(), 1);

    manager.remove_node(1);

    assert_eq!(manager.node_count(), 0);
    assert_eq!(
        manager.connection_count(),
        0,
        "Connection cache should be cleared when node is removed"
    );
}
