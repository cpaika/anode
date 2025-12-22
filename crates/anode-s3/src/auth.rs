//! S3 authentication and signature verification.
//!
//! This module handles AWS Signature Version 4 authentication for S3 API requests.
//! It provides signature verification to ensure that requests are properly authenticated
//! before being processed.

use crate::error::{S3Error, S3Result};
use axum::http::{HeaderMap, Method, Uri};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

/// AWS Signature V4 authentication handler.
///
/// Provides verification of AWS Signature Version 4 signed requests,
/// which is the standard authentication method for AWS S3 API.
pub struct AwsSignatureV4 {
    access_key_id: String,
    secret_access_key: String,
}

impl AwsSignatureV4 {
    /// Creates a new AWS Signature V4 handler.
    ///
    /// # Arguments
    ///
    /// * `access_key` - The access key ID
    /// * `secret_key` - The secret access key
    pub fn new(access_key: String, secret_key: String) -> Self {
        Self {
            access_key_id: access_key,
            secret_access_key: secret_key,
        }
    }

    /// Verifies an incoming request's signature.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method of the request
    /// * `uri` - URI of the request
    /// * `headers` - HTTP headers
    /// * `payload_hash` - SHA256 hash of the request payload
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the signature is valid, or an error if authentication fails.
    pub fn verify_request(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        payload_hash: &str,
    ) -> S3Result<()> {
        // Extract authorization header
        let auth_header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                S3Error::AuthenticationFailed("Missing authorization header".to_string())
            })?;

        // Parse authorization header
        let (credential, signed_headers, signature) = Self::parse_auth_header(auth_header)?;

        // Extract date and region from credential
        let parts: Vec<&str> = credential.split('/').collect();
        if parts.len() < 4 {
            return Err(S3Error::AuthenticationFailed(
                "Invalid credential format".to_string(),
            ));
        }

        let access_key = parts[0];
        let date = parts[1];
        let region = parts[2];
        let service = parts[3];

        // Verify access key
        if access_key != self.access_key_id {
            return Err(S3Error::AuthenticationFailed(
                "Invalid access key".to_string(),
            ));
        }

        // Get datetime from headers
        let datetime = headers
            .get("x-amz-date")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                S3Error::AuthenticationFailed("Missing x-amz-date header".to_string())
            })?;

        // Build canonical request
        let canonical_request =
            Self::canonical_request(method, uri, headers, &signed_headers, payload_hash);

        // Build string to sign
        let string_to_sign = Self::string_to_sign(datetime, region, &canonical_request);

        // Calculate signing key
        let signing_key = Self::signing_key(&self.secret_access_key, date, region, service);

        // Calculate expected signature
        let expected_signature = Self::calculate_signature(&signing_key, &string_to_sign);

        // Compare signatures
        if signature != expected_signature {
            return Err(S3Error::SignatureDoesNotMatch);
        }

        Ok(())
    }

    /// Parses the authorization header.
    ///
    /// Extracts credential, signed headers, and signature from the AWS Signature V4
    /// authorization header format.
    fn parse_auth_header(auth_header: &str) -> S3Result<(String, Vec<String>, String)> {
        // Format: AWS4-HMAC-SHA256 Credential=..., SignedHeaders=..., Signature=...
        if !auth_header.starts_with("AWS4-HMAC-SHA256") {
            return Err(S3Error::AuthenticationFailed(
                "Invalid signature version".to_string(),
            ));
        }

        let parts: Vec<&str> = auth_header.split(',').collect();
        if parts.len() != 3 {
            return Err(S3Error::AuthenticationFailed(
                "Invalid authorization format".to_string(),
            ));
        }

        // Extract credential
        let credential = parts[0]
            .split('=')
            .nth(1)
            .ok_or_else(|| S3Error::AuthenticationFailed("Missing credential".to_string()))?
            .trim()
            .to_string();

        // Extract signed headers
        let signed_headers = parts[1]
            .split('=')
            .nth(1)
            .ok_or_else(|| S3Error::AuthenticationFailed("Missing signed headers".to_string()))?
            .trim()
            .split(';')
            .map(|s| s.to_string())
            .collect();

        // Extract signature
        let signature = parts[2]
            .split('=')
            .nth(1)
            .ok_or_else(|| S3Error::AuthenticationFailed("Missing signature".to_string()))?
            .trim()
            .to_string();

        Ok((credential, signed_headers, signature))
    }

    /// Builds a canonical request.
    ///
    /// Creates the canonical request string used in AWS Signature V4 calculation.
    pub fn canonical_request(
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        signed_headers: &[String],
        payload_hash: &str,
    ) -> String {
        // Canonical URI
        let canonical_uri = uri.path();

        // Canonical query string
        let canonical_query = uri.query().unwrap_or("");

        // Canonical headers
        let mut canonical_headers = String::new();
        for header_name in signed_headers {
            if let Some(value) = headers.get(header_name) {
                if let Ok(value_str) = value.to_str() {
                    canonical_headers.push_str(&format!(
                        "{}:{}\n",
                        header_name.to_lowercase(),
                        value_str.trim()
                    ));
                }
            }
        }

        // Signed headers string
        let signed_headers_str = signed_headers.join(";");

        format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.as_str(),
            canonical_uri,
            canonical_query,
            canonical_headers,
            signed_headers_str,
            payload_hash
        )
    }

    /// Builds the string to sign.
    ///
    /// Creates the string-to-sign used in AWS Signature V4 calculation.
    pub fn string_to_sign(datetime: &str, region: &str, canonical_request: &str) -> String {
        let date = &datetime[0..8];
        let scope = format!("{}/{}/s3/aws4_request", date, region);
        let hashed_request = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            datetime, scope, hashed_request
        )
    }

    /// Calculates the signing key.
    ///
    /// Derives the signing key used in AWS Signature V4 calculation.
    pub fn signing_key(secret_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
        let k_secret = format!("AWS4{}", secret_key);
        let k_date = hmac_sha256(k_secret.as_bytes(), date.as_bytes());
        let k_region = hmac_sha256(&k_date, region.as_bytes());
        let k_service = hmac_sha256(&k_region, service.as_bytes());
        hmac_sha256(&k_service, b"aws4_request")
    }

    /// Calculates the final signature.
    ///
    /// Computes the final signature by signing the string-to-sign with the signing key.
    pub fn calculate_signature(signing_key: &[u8], string_to_sign: &str) -> String {
        hex::encode(hmac_sha256(signing_key, string_to_sign.as_bytes()))
    }
}

/// HMAC-SHA256 helper.
///
/// Computes HMAC-SHA256 of data with the given key.
fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Authentication configuration.
///
/// Controls whether authentication is enabled and stores the valid access keys.
#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// Whether authentication is enabled.
    pub enabled: bool,
    /// Map of access key ID to secret access key.
    pub access_keys: HashMap<String, String>,
}

impl AuthConfig {
    /// Creates a new auth config with authentication disabled.
    ///
    /// All requests will be allowed without signature verification.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            access_keys: HashMap::new(),
        }
    }

    /// Creates a new auth config with authentication enabled.
    ///
    /// # Arguments
    ///
    /// * `access_keys` - Map of access key IDs to their corresponding secret keys
    pub fn enabled(access_keys: HashMap<String, String>) -> Self {
        Self {
            enabled: true,
            access_keys,
        }
    }

    /// Verifies a request using the configured access keys.
    ///
    /// If authentication is disabled, this returns immediately without verification.
    /// Otherwise, it validates the AWS Signature V4 signature.
    pub fn verify_request(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        payload_hash: &str,
    ) -> S3Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Extract access key from authorization header
        let auth_header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                S3Error::AuthenticationFailed("Missing authorization header".to_string())
            })?;

        let access_key = extract_access_key(auth_header)?;

        // Find the secret key for this access key
        let secret_key = self
            .access_keys
            .get(&access_key)
            .ok_or_else(|| S3Error::AuthenticationFailed("Invalid access key".to_string()))?;

        // Verify signature
        let verifier = AwsSignatureV4::new(access_key, secret_key.clone());
        verifier.verify_request(method, uri, headers, payload_hash)
    }
}

/// Extracts the access key ID from an authorization header.
///
/// Parses the AWS Signature V4 authorization header and returns the access key ID.
fn extract_access_key(auth_header: &str) -> S3Result<String> {
    // Format: AWS4-HMAC-SHA256 Credential=ACCESS_KEY/date/region/service/aws4_request, ...
    let credential_part = auth_header
        .split(',')
        .next()
        .ok_or_else(|| S3Error::AuthenticationFailed("Invalid authorization format".to_string()))?;

    let credential = credential_part
        .split('=')
        .nth(1)
        .ok_or_else(|| S3Error::AuthenticationFailed("Missing credential".to_string()))?
        .trim();

    let access_key = credential
        .split('/')
        .next()
        .ok_or_else(|| S3Error::AuthenticationFailed("Invalid credential format".to_string()))?
        .to_string();

    Ok(access_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signing_key() {
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let date = "20150830";
        let region = "us-east-1";
        let service = "s3";

        let key = AwsSignatureV4::signing_key(secret, date, region, service);
        assert!(!key.is_empty());
    }

    #[test]
    fn test_hmac_sha256() {
        let key = b"secret";
        let data = b"message";
        let result = hmac_sha256(key, data);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_auth_config_disabled() {
        let config = AuthConfig::disabled();
        assert!(!config.enabled);
        assert!(config.access_keys.is_empty());
    }

    #[test]
    fn test_auth_config_enabled() {
        let mut keys = HashMap::new();
        keys.insert("AKIAIOSFODNN7EXAMPLE".to_string(), "secret".to_string());

        let config = AuthConfig::enabled(keys);
        assert!(config.enabled);
        assert_eq!(config.access_keys.len(), 1);
    }

    #[test]
    fn test_extract_access_key() {
        let auth = "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20230101/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-date, Signature=abc123";
        let access_key = extract_access_key(auth).unwrap();
        assert_eq!(access_key, "AKIAIOSFODNN7EXAMPLE");
    }

    #[test]
    fn test_calculate_signature() {
        let key = b"test_key";
        let string_to_sign =
            "AWS4-HMAC-SHA256\n20230101T120000Z\n20230101/us-east-1/s3/aws4_request\nhash";
        let signature = AwsSignatureV4::calculate_signature(key, string_to_sign);
        assert!(!signature.is_empty());
        assert_eq!(signature.len(), 64); // SHA256 hex is 64 chars
    }

    #[test]
    fn test_verify_valid_signature() {
        use axum::http::{HeaderMap, Method, Uri};

        // Set up test credentials
        let access_key = "AKIAIOSFODNN7EXAMPLE";
        let secret_key = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let verifier = AwsSignatureV4::new(access_key.to_string(), secret_key.to_string());

        // Create test request parameters
        let method = Method::GET;
        let uri: Uri = "/test-bucket/test-object".parse().unwrap();
        let datetime = "20230101T120000Z";
        let date = "20230101";
        let region = "us-east-1";
        let service = "s3";

        // Payload hash for empty body (standard for GET requests)
        let payload_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

        // Set up headers
        let mut headers = HeaderMap::new();
        headers.insert("host", "localhost:8080".parse().unwrap());
        headers.insert("x-amz-date", datetime.parse().unwrap());

        // Build the canonical request
        let signed_headers = vec!["host".to_string(), "x-amz-date".to_string()];
        let canonical_request = AwsSignatureV4::canonical_request(
            &method,
            &uri,
            &headers,
            &signed_headers,
            payload_hash,
        );

        // Build string to sign
        let string_to_sign = AwsSignatureV4::string_to_sign(datetime, region, &canonical_request);

        // Calculate signing key
        let signing_key = AwsSignatureV4::signing_key(secret_key, date, region, service);

        // Calculate signature
        let signature = AwsSignatureV4::calculate_signature(&signing_key, &string_to_sign);

        // Build authorization header
        let credential = format!(
            "{}/{}/{}/{}/aws4_request",
            access_key, date, region, service
        );
        let auth_header = format!(
            "AWS4-HMAC-SHA256 Credential={}, SignedHeaders=host;x-amz-date, Signature={}",
            credential, signature
        );
        headers.insert("authorization", auth_header.parse().unwrap());

        // Verify the signature - this should succeed
        let result = verifier.verify_request(&method, &uri, &headers, payload_hash);
        assert!(result.is_ok(), "Valid signature should be accepted");
    }

    #[test]
    fn test_verify_invalid_signature() {
        use axum::http::{HeaderMap, Method, Uri};

        // Set up test credentials
        let access_key = "AKIAIOSFODNN7EXAMPLE";
        let secret_key = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let verifier = AwsSignatureV4::new(access_key.to_string(), secret_key.to_string());

        // Create test request parameters
        let method = Method::GET;
        let uri: Uri = "/test-bucket/test-object".parse().unwrap();
        let datetime = "20230101T120000Z";
        let date = "20230101";
        let region = "us-east-1";

        // Payload hash for empty body (standard for GET requests)
        let payload_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

        // Set up headers
        let mut headers = HeaderMap::new();
        headers.insert("host", "localhost:8080".parse().unwrap());
        headers.insert("x-amz-date", datetime.parse().unwrap());

        // Build authorization header with an invalid/wrong signature
        let credential = format!("{}/{}/{}/s3/aws4_request", access_key, date, region);
        let invalid_signature = "0000000000000000000000000000000000000000000000000000000000000000";
        let auth_header = format!(
            "AWS4-HMAC-SHA256 Credential={}, SignedHeaders=host;x-amz-date, Signature={}",
            credential, invalid_signature
        );
        headers.insert("authorization", auth_header.parse().unwrap());

        // Verify the signature - this should fail with SignatureDoesNotMatch
        let result = verifier.verify_request(&method, &uri, &headers, payload_hash);
        assert!(result.is_err(), "Invalid signature should be rejected");

        match result {
            Err(S3Error::SignatureDoesNotMatch) => {
                // Expected error type
            },
            _ => panic!("Expected SignatureDoesNotMatch error"),
        }
    }
}
