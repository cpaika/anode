use crate::error::{ClientError, Result};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub endpoint: String,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub region: String,
    pub timeout: Duration,
    pub max_retries: u32,
    pub verify_checksums: bool,
    pub path_style: bool,
}

impl ClientConfig {
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

    pub fn with_credentials(
        mut self,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(access_key.into());
        self.secret_access_key = Some(secret_key.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    pub fn with_checksum_verification(mut self, verify: bool) -> Self {
        self.verify_checksums = verify;
        self
    }

    pub fn with_path_style(mut self, path_style: bool) -> Self {
        self.path_style = path_style;
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.endpoint.is_empty() {
            return Err(ClientError::InvalidConfig(
                "endpoint cannot be empty".to_string(),
            ));
        }

        url::Url::parse(&self.endpoint).map_err(|e| {
            ClientError::InvalidConfig(format!("invalid endpoint URL: {}", e))
        })?;

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

    pub fn from_env() -> Result<Self> {
        let endpoint = std::env::var("ANODE_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());

        let mut config = Self::new(endpoint);

        if let Ok(access_key) = std::env::var("ANODE_ACCESS_KEY_ID") {
            let secret_key = std::env::var("ANODE_SECRET_ACCESS_KEY")
                .map_err(|_| {
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
                ClientError::InvalidConfig(format!(
                    "invalid ANODE_TIMEOUT value: {}",
                    timeout_str
                ))
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
