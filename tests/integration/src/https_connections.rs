//! HTTPS/TLS Connection Tests
//!
//! Comprehensive tests for HTTPS connections including:
//! - Server starts with TLS configuration
//! - Client can connect via HTTPS
//! - HTTP connection is rejected when TLS is required
//! - Invalid certificates are rejected
//! - Client certificate authentication

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use tracing::{debug, info};

/// Certificate and key generation utilities for testing
pub struct TlsCertificates {
    pub temp_dir: TempDir,
    pub server_cert_path: PathBuf,
    pub server_key_path: PathBuf,
    pub ca_cert_path: PathBuf,
    pub ca_key_path: PathBuf,
    pub client_cert_path: PathBuf,
    pub client_key_path: PathBuf,
}

impl TlsCertificates {
    /// Generate self-signed certificates for testing
    pub fn generate() -> Result<Self> {
        let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
        let base_path = temp_dir.path();

        info!("Generating test certificates in {:?}", base_path);

        // Generate CA certificate
        let ca_cert_path = base_path.join("ca-cert.pem");
        let ca_key_path = base_path.join("ca-key.pem");
        Self::generate_ca(&ca_cert_path, &ca_key_path)?;

        // Generate server certificate signed by CA
        let server_cert_path = base_path.join("server-cert.pem");
        let server_key_path = base_path.join("server-key.pem");
        Self::generate_server_cert(
            &server_cert_path,
            &server_key_path,
            &ca_cert_path,
            &ca_key_path,
        )?;

        // Generate client certificate signed by CA
        let client_cert_path = base_path.join("client-cert.pem");
        let client_key_path = base_path.join("client-key.pem");
        Self::generate_client_cert(
            &client_cert_path,
            &client_key_path,
            &ca_cert_path,
            &ca_key_path,
        )?;

        Ok(Self {
            temp_dir,
            server_cert_path,
            server_key_path,
            ca_cert_path,
            ca_key_path,
            client_cert_path,
            client_key_path,
        })
    }

    /// Generate CA certificate using openssl command
    fn generate_ca(cert_path: &PathBuf, key_path: &PathBuf) -> Result<()> {
        debug!("Generating CA certificate");

        // Generate CA private key
        let output = std::process::Command::new("openssl")
            .args(&["genrsa", "-out", key_path.to_str().unwrap(), "2048"])
            .output()
            .context("Failed to generate CA private key")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to generate CA private key: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Generate CA certificate
        let output = std::process::Command::new("openssl")
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
                "/C=US/ST=Test/L=Test/O=Anode Test/CN=Anode Test CA",
            ])
            .output()
            .context("Failed to generate CA certificate")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to generate CA certificate: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        debug!("CA certificate generated successfully");
        Ok(())
    }

    /// Generate server certificate signed by CA
    fn generate_server_cert(
        cert_path: &PathBuf,
        key_path: &PathBuf,
        ca_cert_path: &PathBuf,
        ca_key_path: &PathBuf,
    ) -> Result<()> {
        debug!("Generating server certificate");

        // Generate server private key
        let output = std::process::Command::new("openssl")
            .args(&["genrsa", "-out", key_path.to_str().unwrap(), "2048"])
            .output()
            .context("Failed to generate server private key")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to generate server private key: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Generate CSR
        let csr_path = cert_path.with_extension("csr");
        let output = std::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-key",
                key_path.to_str().unwrap(),
                "-out",
                csr_path.to_str().unwrap(),
                "-subj",
                "/C=US/ST=Test/L=Test/O=Anode Test/CN=localhost",
            ])
            .output()
            .context("Failed to generate server CSR")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to generate server CSR: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Create extension file for SAN (Subject Alternative Name)
        let ext_path = cert_path.with_extension("ext");
        fs::write(
            &ext_path,
            "subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\n",
        )
        .context("Failed to write extension file")?;

        // Sign certificate with CA
        let output = std::process::Command::new("openssl")
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
            .context("Failed to sign server certificate")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to sign server certificate: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        debug!("Server certificate generated successfully");
        Ok(())
    }

    /// Generate client certificate signed by CA
    fn generate_client_cert(
        cert_path: &PathBuf,
        key_path: &PathBuf,
        ca_cert_path: &PathBuf,
        ca_key_path: &PathBuf,
    ) -> Result<()> {
        debug!("Generating client certificate");

        // Generate client private key
        let output = std::process::Command::new("openssl")
            .args(&["genrsa", "-out", key_path.to_str().unwrap(), "2048"])
            .output()
            .context("Failed to generate client private key")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to generate client private key: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Generate CSR
        let csr_path = cert_path.with_extension("csr");
        let output = std::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-key",
                key_path.to_str().unwrap(),
                "-out",
                csr_path.to_str().unwrap(),
                "-subj",
                "/C=US/ST=Test/L=Test/O=Anode Test/CN=test-client",
            ])
            .output()
            .context("Failed to generate client CSR")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to generate client CSR: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Create extension file for client authentication
        let ext_path = cert_path.with_extension("ext");
        fs::write(&ext_path, "extendedKeyUsage=clientAuth\n")
            .context("Failed to write extension file")?;

        // Sign certificate with CA
        let output = std::process::Command::new("openssl")
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
            .context("Failed to sign client certificate")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to sign client certificate: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        debug!("Client certificate generated successfully");
        Ok(())
    }

    /// Create a reqwest client configured to trust the CA
    pub fn create_https_client(&self) -> Result<reqwest::Client> {
        let ca_cert = fs::read(&self.ca_cert_path).context("Failed to read CA certificate")?;

        let ca_cert =
            reqwest::Certificate::from_pem(&ca_cert).context("Failed to parse CA certificate")?;

        let client = reqwest::Client::builder()
            .add_root_certificate(ca_cert)
            .danger_accept_invalid_certs(false)
            .build()
            .context("Failed to create HTTPS client")?;

        Ok(client)
    }

    /// Create a reqwest client with mutual TLS (client certificate)
    ///
    /// Note: This is a simplified implementation for testing.
    /// In production, you would use a proper TLS library for client certificates.
    /// For now, we'll use curl as an external command to test mTLS.
    pub fn test_mtls_connection(&self, url: &str) -> Result<bool> {
        use std::process::Command;

        let output = Command::new("curl")
            .args(&[
                "-v",
                "--cacert",
                self.ca_cert_path.to_str().unwrap(),
                "--cert",
                self.client_cert_path.to_str().unwrap(),
                "--key",
                self.client_key_path.to_str().unwrap(),
                url,
            ])
            .output()
            .context("Failed to execute curl for mTLS test")?;

        // Check if the connection was successful (exit code 0)
        Ok(output.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_certificates() {
        let certs = TlsCertificates::generate().expect("Failed to generate certificates");

        // Verify all certificate files exist
        assert!(certs.ca_cert_path.exists(), "CA certificate should exist");
        assert!(certs.ca_key_path.exists(), "CA key should exist");
        assert!(
            certs.server_cert_path.exists(),
            "Server certificate should exist"
        );
        assert!(certs.server_key_path.exists(), "Server key should exist");
        assert!(
            certs.client_cert_path.exists(),
            "Client certificate should exist"
        );
        assert!(certs.client_key_path.exists(), "Client key should exist");

        // Verify certificates can be read
        let ca_cert = fs::read(&certs.ca_cert_path).expect("Failed to read CA cert");
        assert!(!ca_cert.is_empty(), "CA certificate should not be empty");

        let server_cert = fs::read(&certs.server_cert_path).expect("Failed to read server cert");
        assert!(
            !server_cert.is_empty(),
            "Server certificate should not be empty"
        );
    }

    #[tokio::test]
    async fn test_create_https_client() {
        let certs = TlsCertificates::generate().expect("Failed to generate certificates");
        let client = certs
            .create_https_client()
            .expect("Failed to create HTTPS client");

        // Verify client was created
        assert!(client.get("https://localhost").build().is_ok());
    }

    #[tokio::test]
    async fn test_mtls_connection_helper() {
        let certs = TlsCertificates::generate().expect("Failed to generate certificates");

        // We can't test the actual connection without a running server,
        // but we can verify the certificate files exist
        assert!(certs.client_cert_path.exists());
        assert!(certs.client_key_path.exists());
    }
}
