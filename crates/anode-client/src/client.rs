use crate::config::ClientConfig;
use crate::error::{ClientError, Result};
use crate::multipart::{CompletedPart, CompleteMultipartOutput, MultipartUpload};
use aws_credential_types::Credentials;
use aws_sigv4::http_request::{sign, SignableBody, SignableRequest, SigningSettings};
use aws_sigv4::sign::v4;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tracing::{debug, warn};

#[derive(Clone)]
pub struct AnodeClient {
    http: Client,
    config: ClientConfig,
}

impl AnodeClient {
    pub fn new(config: ClientConfig) -> Result<Self> {
        config.validate()?;

        let http = Client::builder()
            .timeout(config.timeout)
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(ClientError::Network)?;

        Ok(Self { http, config })
    }

    fn build_url(&self, bucket: Option<&str>, key: Option<&str>) -> String {
        let mut url = self.config.endpoint.clone();

        if let Some(bucket) = bucket {
            if self.config.path_style {
                url = format!("{}/{}", url, bucket);
            } else {
                url = url.replace("://", &format!("://{}.", bucket));
            }
        }

        if let Some(key) = key {
            url = format!("{}/{}", url, key);
        }

        url
    }

    async fn sign_request(
        &self,
        mut request: reqwest::Request,
    ) -> Result<reqwest::Request> {
        if let (Some(access_key), Some(secret_key)) =
            (&self.config.access_key_id, &self.config.secret_access_key)
        {
            let credentials = Credentials::new(
                access_key,
                secret_key,
                None,
                None,
                "anode-client",
            );

            let identity = credentials.into();
            let signing_settings = SigningSettings::default();
            let signing_params = v4::SigningParams::builder()
                .identity(&identity)
                .region(&self.config.region)
                .name("s3")
                .time(SystemTime::now())
                .settings(signing_settings)
                .build()
                .map_err(|e| {
                    ClientError::AuthenticationFailed(format!(
                        "Failed to build signing params: {}",
                        e
                    ))
                })?;

            let uri = request.url().as_str();
            let method = request.method().as_str();
            let headers = request
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str(), std::str::from_utf8(v.as_bytes()).unwrap_or("")))
                .collect::<Vec<_>>();

            let signable_request = SignableRequest::new(
                method,
                uri,
                headers.iter().map(|(k, v)| (*k, *v)),
                SignableBody::Bytes(&[]),
            )
            .map_err(|e| {
                ClientError::AuthenticationFailed(format!(
                    "Failed to create signable request: {}",
                    e
                ))
            })?;

            let (signing_instructions, _signature) =
                sign(signable_request, &signing_params.into())
                    .map_err(|e| {
                        ClientError::AuthenticationFailed(format!(
                            "Failed to sign request: {}",
                            e
                        ))
                    })?
                    .into_parts();

            for (name, value) in signing_instructions.headers() {
                let header_name: reqwest::header::HeaderName = name.parse().map_err(|e| {
                    ClientError::AuthenticationFailed(format!("Invalid header name: {}", e))
                })?;
                let header_value: reqwest::header::HeaderValue = value.parse().map_err(|e| {
                    ClientError::AuthenticationFailed(format!("Invalid header value: {}", e))
                })?;
                request.headers_mut().insert(header_name, header_value);
            }
        }

        Ok(request)
    }

    async fn execute_with_retry<F, Fut>(
        &self,
        operation: F,
    ) -> Result<reqwest::Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<reqwest::Response>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match operation().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }

                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();

                    let error = ClientError::from_status_code(status, body);

                    if !error.is_retriable() || attempt == self.config.max_retries {
                        return Err(error);
                    }

                    warn!("Request failed with retriable error: {:?}, attempt {}/{}", error, attempt + 1, self.config.max_retries);
                    last_error = Some(error);
                }
                Err(e) => {
                    if !e.is_retriable() || attempt == self.config.max_retries {
                        return Err(e);
                    }

                    warn!("Request failed with retriable error: {:?}, attempt {}/{}", e, attempt + 1, self.config.max_retries);
                    last_error = Some(e);
                }
            }

            // Exponential backoff
            let delay = Duration::from_millis(100 * 2_u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }

        Err(last_error.unwrap_or_else(|| {
            ClientError::InvalidConfig("Max retries exceeded".to_string())
        }))
    }

    // Bucket operations

    pub async fn create_bucket(&self, bucket: &str) -> Result<()> {
        debug!("Creating bucket: {}", bucket);

        let url = self.build_url(Some(bucket), None);
        let request = self.http.put(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        self.execute_with_retry(|| async {
            Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
        })
        .await?;

        Ok(())
    }

    pub async fn delete_bucket(&self, bucket: &str) -> Result<()> {
        debug!("Deleting bucket: {}", bucket);

        let url = self.build_url(Some(bucket), None);
        let request = self.http.delete(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        self.execute_with_retry(|| async {
            Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
        })
        .await?;

        Ok(())
    }

    pub async fn head_bucket(&self, bucket: &str) -> Result<bool> {
        debug!("Checking if bucket exists: {}", bucket);

        let url = self.build_url(Some(bucket), None);
        let request = self.http.head(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        match self.http.execute(signed_request).await {
            Ok(response) if response.status().is_success() => Ok(true),
            Ok(response) if response.status() == reqwest::StatusCode::NOT_FOUND => Ok(false),
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Err(ClientError::from_status_code(status, body))
            }
            Err(e) => Err(ClientError::Network(e)),
        }
    }

    pub async fn list_buckets(&self) -> Result<Vec<BucketInfo>> {
        debug!("Listing buckets");

        let url = self.config.endpoint.clone();
        let request = self.http.get(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let body = response.text().await?;
        let list_result: ListAllMyBucketsResult = quick_xml::de::from_str(&body)?;

        Ok(list_result
            .buckets
            .bucket
            .into_iter()
            .map(|b| BucketInfo {
                name: b.name,
                creation_date: b.creation_date,
            })
            .collect())
    }

    // Object operations

    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: impl Into<Bytes>,
    ) -> Result<PutObjectOutput> {
        let data = data.into();
        debug!("Putting object: {}/{} ({} bytes)", bucket, key, data.len());

        let url = self.build_url(Some(bucket), Some(key));

        let md5_hash = if self.config.verify_checksums {
            Some(format!("{:x}", md5::compute(&data)))
        } else {
            None
        };

        let mut request_builder = self.http.put(&url).body(data);

        if let Some(hash) = &md5_hash {
            request_builder = request_builder.header("Content-MD5", hash);
        }

        let request = request_builder.build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let etag = response
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        Ok(PutObjectOutput { etag })
    }

    pub async fn get_object(&self, bucket: &str, key: &str) -> Result<GetObjectOutput> {
        debug!("Getting object: {}/{}", bucket, key);

        let url = self.build_url(Some(bucket), Some(key));
        let request = self.http.get(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let metadata = self.extract_metadata(key, &response)?;
        let body = response.bytes().await?;

        Ok(GetObjectOutput { metadata, body })
    }

    pub async fn head_object(&self, bucket: &str, key: &str) -> Result<ObjectMetadata> {
        debug!("Getting object metadata: {}/{}", bucket, key);

        let url = self.build_url(Some(bucket), Some(key));
        let request = self.http.head(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        self.extract_metadata(key, &response)
    }

    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<()> {
        debug!("Deleting object: {}/{}", bucket, key);

        let url = self.build_url(Some(bucket), Some(key));
        let request = self.http.delete(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        self.execute_with_retry(|| async {
            Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
        })
        .await?;

        Ok(())
    }

    pub async fn copy_object(
        &self,
        src_bucket: &str,
        src_key: &str,
        dst_bucket: &str,
        dst_key: &str,
    ) -> Result<CopyObjectOutput> {
        debug!(
            "Copying object: {}/{} -> {}/{}",
            src_bucket, src_key, dst_bucket, dst_key
        );

        let url = self.build_url(Some(dst_bucket), Some(dst_key));
        let copy_source = format!("{}/{}", src_bucket, src_key);

        let request = self
            .http
            .put(&url)
            .header("x-amz-copy-source", copy_source)
            .build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let body = response.text().await?;
        let result: CopyObjectResult = quick_xml::de::from_str(&body)?;

        Ok(CopyObjectOutput {
            etag: result.etag,
            last_modified: result.last_modified,
        })
    }

    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        max_keys: Option<i32>,
    ) -> Result<ListObjectsOutput> {
        debug!("Listing objects in bucket: {}", bucket);

        let mut url = self.build_url(Some(bucket), None);
        url.push_str("?list-type=2");

        if let Some(prefix) = prefix {
            url.push_str(&format!("&prefix={}", urlencoding::encode(prefix)));
        }

        if let Some(max_keys) = max_keys {
            url.push_str(&format!("&max-keys={}", max_keys));
        }

        let request = self.http.get(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let body = response.text().await?;
        let list_result: ListBucketResult = quick_xml::de::from_str(&body)?;

        let objects = list_result
            .contents
            .into_iter()
            .map(|obj| ObjectMetadata {
                key: obj.key,
                size: obj.size,
                etag: obj.etag.trim_matches('"').to_string(),
                last_modified: DateTime::parse_from_rfc3339(&obj.last_modified)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now),
                content_type: None,
                metadata: HashMap::new(),
            })
            .collect();

        let common_prefixes = list_result
            .common_prefixes
            .into_iter()
            .map(|cp| cp.prefix)
            .collect();

        Ok(ListObjectsOutput {
            objects,
            common_prefixes,
            is_truncated: list_result.is_truncated,
            next_continuation_token: list_result.next_continuation_token,
        })
    }

    // Streaming operations

    pub async fn put_object_stream<S>(
        &self,
        bucket: &str,
        key: &str,
        stream: S,
        content_length: u64,
    ) -> Result<PutObjectOutput>
    where
        S: Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + 'static,
    {
        debug!(
            "Putting object stream: {}/{} ({} bytes)",
            bucket, key, content_length
        );

        let url = self.build_url(Some(bucket), Some(key));

        let body = reqwest::Body::wrap_stream(stream);
        let request = self
            .http
            .put(&url)
            .header("Content-Length", content_length)
            .body(body)
            .build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let etag = response
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        Ok(PutObjectOutput { etag })
    }

    pub async fn get_object_stream(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<impl Stream<Item = Result<Bytes>>> {
        debug!("Getting object stream: {}/{}", bucket, key);

        let url = self.build_url(Some(bucket), Some(key));
        let request = self.http.get(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self.http.execute(signed_request).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ClientError::from_status_code(status, body));
        }

        Ok(response
            .bytes_stream()
            .map(|result: std::result::Result<Bytes, reqwest::Error>| {
                result.map_err(ClientError::Network)
            }))
    }

    // Range requests

    pub async fn get_object_range(
        &self,
        bucket: &str,
        key: &str,
        start: u64,
        end: u64,
    ) -> Result<Bytes> {
        debug!(
            "Getting object range: {}/{} (bytes {}-{})",
            bucket, key, start, end
        );

        if start > end {
            return Err(ClientError::InvalidRange(format!(
                "start ({}) must be <= end ({})",
                start, end
            )));
        }

        let url = self.build_url(Some(bucket), Some(key));
        let range_header = format!("bytes={}-{}", start, end);

        let request = self.http.get(&url).header("Range", range_header).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        Ok(response.bytes().await?)
    }

    // Multipart upload

    pub fn multipart_upload<'a>(&'a self, bucket: &str, key: &str) -> MultipartUpload<'a> {
        MultipartUpload::new(self, bucket.to_string(), key.to_string())
    }

    // Internal multipart operations (used by MultipartUpload)

    pub(crate) async fn initiate_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<String> {
        debug!("Initiating multipart upload: {}/{}", bucket, key);

        let url = format!("{}?uploads", self.build_url(Some(bucket), Some(key)));
        let request = self.http.post(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let body = response.text().await?;
        let result: InitiateMultipartUploadResult = quick_xml::de::from_str(&body)?;

        Ok(result.upload_id)
    }

    pub(crate) async fn upload_part(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: i32,
        data: Bytes,
    ) -> Result<CompletedPart> {
        debug!(
            "Uploading part {} for {}/{} ({} bytes)",
            part_number,
            bucket,
            key,
            data.len()
        );

        if part_number < 1 || part_number > 10000 {
            return Err(ClientError::InvalidPartNumber(part_number));
        }

        let url = format!(
            "{}?partNumber={}&uploadId={}",
            self.build_url(Some(bucket), Some(key)),
            part_number,
            upload_id
        );

        let request = self.http.put(&url).body(data).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let etag = response
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ClientError::InvalidETag("Missing ETag header".to_string()))?
            .trim_matches('"')
            .to_string();

        Ok(CompletedPart { part_number, etag })
    }

    pub(crate) async fn complete_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: Vec<CompletedPart>,
    ) -> Result<CompleteMultipartOutput> {
        debug!("Completing multipart upload: {}/{}", bucket, key);

        let url = format!(
            "{}?uploadId={}",
            self.build_url(Some(bucket), Some(key)),
            upload_id
        );

        let parts_xml = parts
            .iter()
            .map(|p| {
                format!(
                    "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                    p.part_number, p.etag
                )
            })
            .collect::<Vec<_>>()
            .join("");

        let body = format!(
            "<CompleteMultipartUpload>{}</CompleteMultipartUpload>",
            parts_xml
        );

        let request = self.http.post(&url).body(body).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
            })
            .await?;

        let response_body = response.text().await?;
        let result: CompleteMultipartUploadResult = quick_xml::de::from_str(&response_body)?;

        Ok(CompleteMultipartOutput {
            location: result.location,
            bucket: result.bucket,
            key: result.key,
            etag: result.etag.trim_matches('"').to_string(),
        })
    }

    pub(crate) async fn abort_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<()> {
        debug!("Aborting multipart upload: {}/{}", bucket, key);

        let url = format!(
            "{}?uploadId={}",
            self.build_url(Some(bucket), Some(key)),
            upload_id
        );

        let request = self.http.delete(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        self.execute_with_retry(|| async {
            Ok(self.http.execute(signed_request.try_clone().unwrap()).await?)
        })
        .await?;

        Ok(())
    }

    // Presigned URLs

    pub fn presign_get(
        &self,
        bucket: &str,
        key: &str,
        expires_in: Duration,
    ) -> Result<String> {
        self.presign_url(bucket, key, "GET", expires_in)
    }

    pub fn presign_put(
        &self,
        bucket: &str,
        key: &str,
        expires_in: Duration,
    ) -> Result<String> {
        self.presign_url(bucket, key, "PUT", expires_in)
    }

    fn presign_url(
        &self,
        bucket: &str,
        key: &str,
        _method: &str,
        expires_in: Duration,
    ) -> Result<String> {
        let (_access_key, _secret_key) = match (
            &self.config.access_key_id,
            &self.config.secret_access_key,
        ) {
            (Some(ak), Some(sk)) => (ak, sk),
            _ => {
                return Err(ClientError::AuthenticationFailed(
                    "Credentials required for presigned URLs".to_string(),
                ))
            }
        };

        let url = self.build_url(Some(bucket), Some(key));
        let _timestamp = SystemTime::now();
        let expires = expires_in.as_secs();

        // This is a simplified presigned URL implementation
        // In production, you would use the full AWS SigV4 presigning logic
        let presigned_url = format!(
            "{}?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Expires={}",
            url, expires
        );

        Ok(presigned_url)
    }

    // Helper methods

    fn extract_metadata(
        &self,
        key: &str,
        response: &reqwest::Response,
    ) -> Result<ObjectMetadata> {
        let headers = response.headers();

        let size = headers
            .get("Content-Length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let etag = headers
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        let last_modified = headers
            .get("Last-Modified")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| DateTime::parse_from_rfc2822(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let content_type = headers
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let mut metadata = HashMap::new();
        for (name, value) in headers.iter() {
            if let Some(meta_key) = name.as_str().strip_prefix("x-amz-meta-") {
                if let Ok(value_str) = value.to_str() {
                    metadata.insert(meta_key.to_string(), value_str.to_string());
                }
            }
        }

        Ok(ObjectMetadata {
            key: key.to_string(),
            size,
            etag,
            last_modified,
            content_type,
            metadata,
        })
    }
}

// Data structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    pub name: String,
    #[serde(rename = "CreationDate")]
    pub creation_date: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    pub key: String,
    pub size: u64,
    pub etag: String,
    pub last_modified: DateTime<Utc>,
    pub content_type: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug)]
pub struct GetObjectOutput {
    pub metadata: ObjectMetadata,
    pub body: Bytes,
}

#[derive(Debug)]
pub struct PutObjectOutput {
    pub etag: String,
}

#[derive(Debug)]
pub struct CopyObjectOutput {
    pub etag: String,
    pub last_modified: String,
}

#[derive(Debug)]
pub struct ListObjectsOutput {
    pub objects: Vec<ObjectMetadata>,
    pub common_prefixes: Vec<String>,
    pub is_truncated: bool,
    pub next_continuation_token: Option<String>,
}

// XML response structures

#[derive(Debug, Deserialize)]
struct ListAllMyBucketsResult {
    #[serde(rename = "Buckets")]
    buckets: Buckets,
}

#[derive(Debug, Deserialize)]
struct Buckets {
    #[serde(rename = "Bucket", default)]
    bucket: Vec<BucketInfo>,
}

#[derive(Debug, Deserialize)]
struct ListBucketResult {
    #[serde(rename = "Contents", default)]
    contents: Vec<S3Object>,
    #[serde(rename = "CommonPrefixes", default)]
    common_prefixes: Vec<CommonPrefix>,
    #[serde(rename = "IsTruncated")]
    is_truncated: bool,
    #[serde(rename = "NextContinuationToken")]
    next_continuation_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct S3Object {
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "ETag")]
    etag: String,
    #[serde(rename = "LastModified")]
    last_modified: String,
}

#[derive(Debug, Deserialize)]
struct CommonPrefix {
    #[serde(rename = "Prefix")]
    prefix: String,
}

#[derive(Debug, Deserialize)]
struct CopyObjectResult {
    #[serde(rename = "ETag")]
    etag: String,
    #[serde(rename = "LastModified")]
    last_modified: String,
}

#[derive(Debug, Deserialize)]
struct InitiateMultipartUploadResult {
    #[serde(rename = "UploadId")]
    upload_id: String,
}

#[derive(Debug, Deserialize)]
struct CompleteMultipartUploadResult {
    #[serde(rename = "Location")]
    location: String,
    #[serde(rename = "Bucket")]
    bucket: String,
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "ETag")]
    etag: String,
}
