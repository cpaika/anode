use crate::error::{ClientError, Result};
use std::time::Duration;

/// Configuration for the Anode client.
///
/// `ClientConfig` provides all the settings needed to configure an [`AnodeClient`](crate::AnodeClient),
/// including endpoint URL, credentials, timeout, retry behavior, and S3 compatibility options.
///
/// # Example
///
/// ```
/// use anode_client::ClientConfig;
/// use std::time::Duration;
///
/// let config = ClientConfig::new("http://localhost:8080")
///     .with_credentials("my_access_key", "my_secret_key")
///     .with_region("us-west-2")
///     .with_timeout(Duration::from_secs(60))
///     .with_retries(5);
/// ```
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The endpoint URL for the Anode server (e.g., "http://localhost:8080")
    pub endpoint: String,
    /// Optional AWS access key ID for authentication
    pub access_key_id: Option<String>,
    /// Optional AWS secret access key for authentication
    pub secret_access_key: Option<String>,
    /// AWS region name (default: "us-east-1")
    pub region: String,
    /// Request timeout duration (default: 300 seconds)
    pub timeout: Duration,
    /// Maximum number of retry attempts for failed requests (default: 3)
    pub max_retries: u32,
    /// Whether to verify MD5 checksums on uploads (default: true)
    pub verify_checksums: bool,
    /// Whether to use path-style URLs (true) or virtual-hosted-style URLs (false)
    pub path_style: bool,
}

impl ClientConfig {
    /// Creates a new client configuration with default settings.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The base URL of the Anode server (e.g., "http://localhost:8080")
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientConfig;
    ///
    /// let config = ClientConfig::new("http://localhost:8080");
    /// ```
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            access_key_id: None,
            secret_access_key: None,
            region: "us-east-1".to_string(),
            timeout: Duration::from_secs(300),
            max_retries: 3,
            verify_checksums: true,
            path_style: true,
        }
    }

    /// Sets AWS credentials for authentication (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `access_key` - AWS access key ID
    /// * `secret_key` - AWS secret access key
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientConfig;
    ///
    /// let config = ClientConfig::new("http://localhost:8080")
    ///     .with_credentials("AKIAIOSFODNN7EXAMPLE", "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
    /// ```
    pub fn with_credentials(
        mut self,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(access_key.into());
        self.secret_access_key = Some(secret_key.into());
        self
    }

    /// Sets the request timeout duration (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum duration to wait for a request to complete
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientConfig;
    /// use std::time::Duration;
    ///
    /// let config = ClientConfig::new("http://localhost:8080")
    ///     .with_timeout(Duration::from_secs(60));
    /// ```
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the maximum number of retry attempts for failed requests (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `max_retries` - Maximum number of retries (0 means no retries)
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientConfig;
    ///
    /// let config = ClientConfig::new("http://localhost:8080")
    ///     .with_retries(5);
    /// ```
    pub fn with_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Sets the AWS region (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `region` - AWS region name (e.g., "us-west-2")
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientConfig;
    ///
    /// let config = ClientConfig::new("http://localhost:8080")
    ///     .with_region("us-west-2");
    /// ```
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Enables or disables MD5 checksum verification for uploads (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `verify` - Whether to verify checksums (default: true)
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientConfig;
    ///
    /// let config = ClientConfig::new("http://localhost:8080")
    ///     .with_checksum_verification(false);
    /// ```
    pub fn with_checksum_verification(mut self, verify: bool) -> Self {
        self.verify_checksums = verify;
        self
    }

    /// Sets whether to use path-style URLs or virtual-hosted-style URLs (builder pattern).
    ///
    /// Path-style: `http://localhost:8080/bucket/key`
    /// Virtual-hosted-style: `http://bucket.localhost:8080/key`
    ///
    /// # Arguments
    ///
    /// * `path_style` - Whether to use path-style URLs (default: true)
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientConfig;
    ///
    /// let config = ClientConfig::new("http://localhost:8080")
    ///     .with_path_style(false);
    /// ```
    pub fn with_path_style(mut self, path_style: bool) -> Self {
        self.path_style = path_style;
        self
    }

    /// Validates the configuration settings.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The endpoint is empty or not a valid URL
    /// - The region is empty
    /// - The timeout is zero
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientConfig;
    ///
    /// let config = ClientConfig::new("http://localhost:8080");
    /// assert!(config.validate().is_ok());
    ///
    /// let invalid_config = ClientConfig::new("");
    /// assert!(invalid_config.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<()> {
        if self.endpoint.is_empty() {
            return Err(ClientError::InvalidConfig(
                "endpoint cannot be empty".to_string(),
            ));
        }

        url::Url::parse(&self.endpoint)
            .map_err(|e| ClientError::InvalidConfig(format!("invalid endpoint URL: {}", e)))?;

        if self.region.is_empty() {
            return Err(ClientError::InvalidConfig(
                "region cannot be empty".to_string(),
            ));
        }

        if self.timeout.as_secs() == 0 {
            return Err(ClientError::InvalidConfig(
                "timeout must be greater than zero".to_string(),
            ));
        }

        Ok(())
    }

    /// Creates a configuration from environment variables.
    ///
    /// Reads the following environment variables:
    /// - `ANODE_ENDPOINT` - Server endpoint (default: "http://localhost:8080")
    /// - `ANODE_ACCESS_KEY_ID` - Access key ID (optional)
    /// - `ANODE_SECRET_ACCESS_KEY` - Secret access key (required if access key ID is set)
    /// - `ANODE_REGION` - AWS region (optional, default: "us-east-1")
    /// - `ANODE_TIMEOUT` - Timeout in seconds (optional, default: 300)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `ANODE_ACCESS_KEY_ID` is set but `ANODE_SECRET_ACCESS_KEY` is not
    /// - `ANODE_TIMEOUT` is not a valid number
    /// - Configuration validation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anode_client::ClientConfig;
    ///
    /// // Set environment variables first
    /// std::env::set_var("ANODE_ENDPOINT", "http://localhost:8080");
    /// std::env::set_var("ANODE_ACCESS_KEY_ID", "my_key");
    /// std::env::set_var("ANODE_SECRET_ACCESS_KEY", "my_secret");
    ///
    /// let config = ClientConfig::from_env()?;
    /// # Ok::<(), anode_client::ClientError>(())
    /// ```
    pub fn from_env() -> Result<Self> {
        let endpoint =
            std::env::var("ANODE_ENDPOINT").unwrap_or_else(|_| "http://localhost:8080".to_string());

        let mut config = Self::new(endpoint);

        if let Ok(access_key) = std::env::var("ANODE_ACCESS_KEY_ID") {
            let secret_key = std::env::var("ANODE_SECRET_ACCESS_KEY").map_err(|_| {
                ClientError::InvalidConfig(
                    "ANODE_SECRET_ACCESS_KEY must be set when ANODE_ACCESS_KEY_ID is set"
                        .to_string(),
                )
            })?;
            config = config.with_credentials(access_key, secret_key);
        }

        if let Ok(region) = std::env::var("ANODE_REGION") {
            config = config.with_region(region);
        }

        if let Ok(timeout_str) = std::env::var("ANODE_TIMEOUT") {
            let timeout_secs: u64 = timeout_str.parse().map_err(|_| {
                ClientError::InvalidConfig(format!("invalid ANODE_TIMEOUT value: {}", timeout_str))
            })?;
            config = config.with_timeout(Duration::from_secs(timeout_secs));
        }

        config.validate()?;
        Ok(config)
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8080".to_string(),
            access_key_id: None,
            secret_access_key: None,
            region: "us-east-1".to_string(),
            timeout: Duration::from_secs(300),
            max_retries: 3,
            verify_checksums: true,
            path_style: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClientConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8080");
        assert_eq!(config.region, "us-east-1");
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert_eq!(config.max_retries, 3);
        assert!(config.verify_checksums);
        assert!(config.path_style);
    }

    #[test]
    fn test_builder_pattern() {
        let config = ClientConfig::new("http://example.com")
            .with_credentials("key123", "secret456")
            .with_region("us-west-2")
            .with_timeout(Duration::from_secs(60))
            .with_retries(5);

        assert_eq!(config.endpoint, "http://example.com");
        assert_eq!(config.access_key_id, Some("key123".to_string()));
        assert_eq!(config.secret_access_key, Some("secret456".to_string()));
        assert_eq!(config.region, "us-west-2");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_validate() {
        let config = ClientConfig::default();
        assert!(config.validate().is_ok());

        let invalid_config = ClientConfig::new("");
        assert!(invalid_config.validate().is_err());
    }
}
