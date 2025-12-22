//! S3 API request handlers.
//!
//! This module contains the actual handler functions for all S3 API operations,
//! including bucket operations (create, delete, list) and object operations
//! (put, get, delete, copy, multipart uploads).

use crate::error::{S3Error, S3Result};
use crate::multipart::MultipartManager;
use crate::xml::{self, BucketInfo, CompleteMultipartResult, ListObjectsResult, ObjectInfo};
use anode_storage::StorageEngine;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tracing::{debug, info};

/// Application state shared across handlers.
///
/// Contains the storage engine, bucket metadata, and multipart upload manager.
#[derive(Clone)]
pub struct AppState {
    /// The underlying storage engine.
    pub storage: Arc<StorageEngine>,
    /// In-memory bucket metadata (name, creation date).
    pub buckets: Arc<tokio::sync::RwLock<HashMap<String, BucketMetadata>>>,
    /// Multipart upload manager.
    pub multipart: Arc<MultipartManager>,
}

/// Metadata about a bucket.
///
/// Stores bucket name, creation timestamp, optional bucket policy, and lifecycle configuration.
#[derive(Clone, Debug)]
pub struct BucketMetadata {
    /// Bucket name.
    pub name: String,
    /// Unix timestamp of bucket creation.
    pub creation_date: u64,
    /// Optional bucket policy as JSON string.
    pub policy: Option<String>,
    /// Optional lifecycle configuration as XML string.
    pub lifecycle: Option<String>,
}

/// Query parameters for ListObjectsV2.
///
/// Supports pagination and filtering of object listings.
#[derive(Deserialize, Debug)]
pub struct ListParams {
    #[serde(rename = "list-type")]
    pub list_type: Option<u32>,
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    #[serde(rename = "max-keys")]
    pub max_keys: Option<u32>,
    #[serde(rename = "continuation-token")]
    pub continuation_token: Option<String>,
    #[serde(rename = "start-after")]
    pub start_after: Option<String>,
}

/// Query parameters for multipart upload part operations.
#[derive(Deserialize, Debug)]
pub struct PartParams {
    #[serde(rename = "partNumber")]
    pub part_number: u32,
    #[serde(rename = "uploadId")]
    pub upload_id: String,
}

/// Query parameters for upload operations.
#[derive(Deserialize, Debug)]
pub struct UploadParams {
    #[serde(rename = "uploadId")]
    pub upload_id: Option<String>,
    pub uploads: Option<String>,
}

/// Query parameters for list parts.
#[derive(Deserialize, Debug)]
pub struct ListPartsParams {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
    #[serde(rename = "max-parts")]
    pub max_parts: Option<u32>,
    #[serde(rename = "part-number-marker")]
    pub part_number_marker: Option<u32>,
}

/// Query parameters for list multipart uploads.
#[derive(Deserialize, Debug)]
pub struct ListUploadsParams {
    pub uploads: Option<String>,
    pub prefix: Option<String>,
    #[serde(rename = "max-uploads")]
    pub max_uploads: Option<u32>,
    #[serde(rename = "key-marker")]
    pub key_marker: Option<String>,
    #[serde(rename = "upload-id-marker")]
    pub upload_id_marker: Option<String>,
}

/// Query parameters for ListObjectVersions.
#[derive(Deserialize, Debug)]
pub struct ListVersionsParams {
    pub versions: Option<String>,
    pub prefix: Option<String>,
    #[serde(rename = "max-keys")]
    pub max_keys: Option<u32>,
    #[serde(rename = "key-marker")]
    pub key_marker: Option<String>,
    #[serde(rename = "version-id-marker")]
    pub version_id_marker: Option<String>,
}

/// Query parameters for GET object operations.
#[derive(Deserialize, Debug)]
pub struct GetObjectParams {
    #[serde(rename = "versionId")]
    pub version_id: Option<String>,
}

// ============================================================================
// Bucket Operations
// ============================================================================

/// Creates a new bucket.
///
/// Implements the S3 CreateBucket operation.
pub async fn create_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> S3Result<Response> {
    info!("Creating bucket: {}", bucket);

    // Validate bucket name
    validate_bucket_name(&bucket)?;

    let mut buckets = state.buckets.write().await;

    // Check if bucket already exists
    if buckets.contains_key(&bucket) {
        return Err(S3Error::BucketAlreadyExists(bucket));
    }

    // Create bucket metadata
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Create bucket in storage engine
    let bucket_config = anode_storage::BucketConfig {
        name: bucket.clone(),
        created_at: now,
        versioning_enabled: false,
        metadata: std::collections::BTreeMap::new(),
        quota_bytes: None,
    };
    state.storage.create_bucket(&bucket, bucket_config).await?;

    buckets.insert(
        bucket.clone(),
        BucketMetadata {
            name: bucket,
            creation_date: now,
            policy: None,
            lifecycle: None,
        },
    );

    Ok((StatusCode::OK, [("Content-Length", "0")]).into_response())
}

/// Deletes a bucket.
///
/// Implements the S3 DeleteBucket operation. The bucket must be empty.
pub async fn delete_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> S3Result<Response> {
    let mut buckets = state.buckets.write().await;

    // Check if bucket exists
    if !buckets.contains_key(&bucket) {
        return Err(S3Error::NoSuchBucket(bucket));
    }

    // Check if bucket is empty
    let result = state
        .storage
        .list_objects(&bucket, "", 1, None)
        .await
        .map_err(|_| S3Error::InternalError("Failed to list objects".to_string()))?;

    if !result.objects.is_empty() {
        return Err(S3Error::BucketNotEmpty(bucket));
    }

    buckets.remove(&bucket);

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Checks if a bucket exists (HEAD bucket).
///
/// Implements the S3 HeadBucket operation.
pub async fn head_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> S3Result<Response> {
    let buckets = state.buckets.read().await;

    if !buckets.contains_key(&bucket) {
        return Err(S3Error::NoSuchBucket(bucket));
    }

    Ok(StatusCode::OK.into_response())
}

/// Lists all buckets.
///
/// Implements the S3 ListBuckets operation.
pub async fn list_buckets(State(state): State<AppState>) -> S3Result<Response> {
    debug!("Listing buckets");

    let buckets = state.buckets.read().await;

    let bucket_list: Vec<BucketInfo> = buckets
        .values()
        .map(|b| BucketInfo {
            name: b.name.clone(),
            creation_date: xml::format_timestamp(b.creation_date),
        })
        .collect();

    let xml = xml::list_buckets_xml(&bucket_list);

    Ok((StatusCode::OK, [("Content-Type", "application/xml")], xml).into_response())
}

// ============================================================================
// Bucket Policy Operations
// ============================================================================

/// Gets the bucket policy (GET /{bucket}?policy).
///
/// Implements the S3 GetBucketPolicy operation.
pub async fn get_bucket_policy(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> S3Result<Response> {
    let buckets = state.buckets.read().await;

    // Check if bucket exists
    let bucket_metadata = buckets
        .get(&bucket)
        .ok_or_else(|| S3Error::NoSuchBucket(bucket.clone()))?;

    // Check if policy exists
    let policy_json = bucket_metadata
        .policy
        .as_ref()
        .ok_or_else(|| S3Error::NoSuchBucketPolicy(bucket.clone()))?;

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/json")],
        policy_json.clone(),
    )
        .into_response())
}

/// Sets the bucket policy (PUT /{bucket}?policy).
///
/// Implements the S3 PutBucketPolicy operation.
pub async fn put_bucket_policy(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    body: Body,
) -> S3Result<Response> {
    // Read the body
    let policy_json = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| S3Error::InternalError(format!("Failed to read body: {}", e)))?;

    let policy_str = String::from_utf8(policy_json.to_vec())
        .map_err(|e| S3Error::InvalidRequest(format!("Invalid UTF-8: {}", e)))?;

    // Parse and validate the policy
    let policy = crate::policy::BucketPolicy::from_json(&policy_str)
        .map_err(|e| S3Error::InvalidRequest(format!("Invalid policy JSON: {}", e)))?;

    policy
        .validate()
        .map_err(|e| S3Error::InvalidRequest(format!("Policy validation failed: {}", e)))?;

    // Update bucket metadata
    let mut buckets = state.buckets.write().await;

    let bucket_metadata = buckets
        .get_mut(&bucket)
        .ok_or_else(|| S3Error::NoSuchBucket(bucket.clone()))?;

    bucket_metadata.policy = Some(policy_str);

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Deletes the bucket policy (DELETE /{bucket}?policy).
///
/// Implements the S3 DeleteBucketPolicy operation.
pub async fn delete_bucket_policy(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> S3Result<Response> {
    let mut buckets = state.buckets.write().await;

    let bucket_metadata = buckets
        .get_mut(&bucket)
        .ok_or_else(|| S3Error::NoSuchBucket(bucket.clone()))?;

    bucket_metadata.policy = None;

    Ok(StatusCode::NO_CONTENT.into_response())
}

// ============================================================================
// Bucket Lifecycle Operations
// ============================================================================

/// Gets the bucket lifecycle configuration (GET /{bucket}?lifecycle).
///
/// Implements the S3 GetBucketLifecycleConfiguration operation.
pub async fn get_bucket_lifecycle(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> S3Result<Response> {
    let buckets = state.buckets.read().await;

    // Check if bucket exists
    let bucket_metadata = buckets
        .get(&bucket)
        .ok_or_else(|| S3Error::NoSuchBucket(bucket.clone()))?;

    // Check if lifecycle configuration exists
    let lifecycle_xml = bucket_metadata
        .lifecycle
        .as_ref()
        .ok_or_else(|| S3Error::NoSuchLifecycleConfiguration(bucket.clone()))?;

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/xml")],
        lifecycle_xml.clone(),
    )
        .into_response())
}

/// Sets the bucket lifecycle configuration (PUT /{bucket}?lifecycle).
///
/// Implements the S3 PutBucketLifecycleConfiguration operation.
pub async fn put_bucket_lifecycle(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    body: Body,
) -> S3Result<Response> {
    // Read the body
    let lifecycle_xml = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| S3Error::InternalError(format!("Failed to read body: {}", e)))?;

    let lifecycle_str = String::from_utf8(lifecycle_xml.to_vec())
        .map_err(|e| S3Error::InvalidRequest(format!("Invalid UTF-8: {}", e)))?;

    // Parse and validate the lifecycle configuration
    let lifecycle_config = xml::parse_lifecycle_configuration(&lifecycle_str)
        .map_err(|e| S3Error::InvalidRequest(format!("Invalid lifecycle configuration: {}", e)))?;

    // Validate the lifecycle configuration
    lifecycle_config
        .validate()
        .map_err(|e| S3Error::InvalidRequest(format!("Lifecycle validation failed: {}", e)))?;

    // Update bucket metadata
    let mut buckets = state.buckets.write().await;

    let bucket_metadata = buckets
        .get_mut(&bucket)
        .ok_or_else(|| S3Error::NoSuchBucket(bucket.clone()))?;

    bucket_metadata.lifecycle = Some(lifecycle_str);

    Ok(StatusCode::OK.into_response())
}

/// Deletes the bucket lifecycle configuration (DELETE /{bucket}?lifecycle).
///
/// Implements the S3 DeleteBucketLifecycle operation.
pub async fn delete_bucket_lifecycle(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> S3Result<Response> {
    let mut buckets = state.buckets.write().await;

    let bucket_metadata = buckets
        .get_mut(&bucket)
        .ok_or_else(|| S3Error::NoSuchBucket(bucket.clone()))?;

    bucket_metadata.lifecycle = None;

    Ok(StatusCode::NO_CONTENT.into_response())
}

// ============================================================================
// Object Operations
// ============================================================================

/// Uploads an object (PUT object).
///
/// Implements the S3 PutObject operation. Supports custom metadata via x-amz-meta-* headers.
pub async fn put_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> S3Result<Response> {
    info!("Uploading object: bucket={}, key={}", bucket, key);

    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket));
        }
    }

    // Read the body
    let data = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| S3Error::InternalError(format!("Failed to read body: {}", e)))?;

    // Extract metadata from headers
    let mut metadata = BTreeMap::new();
    for (name, value) in headers.iter() {
        if let Some(meta_key) = name.as_str().strip_prefix("x-amz-meta-") {
            if let Ok(value_str) = value.to_str() {
                metadata.insert(meta_key.to_string(), value_str.to_string());
            }
        }
    }

    // Store Content-Type header if present
    if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        if let Ok(content_type_str) = content_type.to_str() {
            metadata.insert("content-type".to_string(), content_type_str.to_string());
        }
    }

    // Store the object
    let manifest = state
        .storage
        .put_object(&bucket, &key, &data, metadata)
        .await?;

    // Return ETag and version ID
    let etag = format!("\"{}\"", hex::encode(manifest.checksum));
    Ok((
        StatusCode::OK,
        [
            ("ETag", etag.as_str()),
            ("Content-Length", "0"),
            ("x-amz-version-id", manifest.version_id.as_str()),
        ],
    )
        .into_response())
}

/// Downloads an object (GET object).
///
/// Implements the S3 GetObject operation with optional version ID support.
pub async fn get_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(params): Query<GetObjectParams>,
    _headers: HeaderMap,
) -> S3Result<Response> {
    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket));
        }
    }

    // Validate versionId if provided
    if let Some(ref version_id) = params.version_id {
        if !crate::versioning::VersionId::is_valid(version_id) {
            return Err(S3Error::InvalidArgument(format!(
                "Invalid versionId: {}",
                version_id
            )));
        }
    }

    // Retrieve the object (with optional version ID)
    let result = if let Some(ref version_id) = params.version_id {
        state
            .storage
            .get_object_version(&bucket, &key, version_id)
            .await
            .map_err(|e| {
                if matches!(e, anode_storage::StorageError::NotFound(_)) {
                    S3Error::NoSuchKey(key.clone())
                } else {
                    S3Error::StorageError(e)
                }
            })?
    } else {
        state.storage.get_object(&bucket, &key).await.map_err(|e| {
            if matches!(e, anode_storage::StorageError::NotFound(_)) {
                S3Error::NoSuchKey(key.clone())
            } else {
                S3Error::StorageError(e)
            }
        })?
    };

    let (manifest, data) = match result {
        Some((manifest, data)) => (manifest, data),
        None => return Err(S3Error::NoSuchKey(key)),
    };

    // If the current object is a delete marker and no version was specified, return 404
    if manifest.is_delete_marker && params.version_id.is_none() {
        let mut response = StatusCode::NOT_FOUND.into_response();
        let headers = response.headers_mut();
        headers.insert("x-amz-delete-marker", "true".parse().unwrap());
        headers.insert("x-amz-version-id", manifest.version_id.parse().unwrap());
        return Ok(response);
    }

    // Get Content-Type from metadata, default to application/octet-stream
    let content_type = manifest
        .metadata
        .get("content-type")
        .map(|s| s.as_str())
        .unwrap_or("application/octet-stream");

    // Build response with custom metadata headers
    let mut response = (
        StatusCode::OK,
        [
            ("Content-Type", content_type.to_string()),
            ("Content-Length", data.len().to_string()),
            ("x-amz-version-id", manifest.version_id.clone()),
        ],
        data,
    )
        .into_response();

    // Add x-amz-meta-* headers from metadata
    let headers = response.headers_mut();
    for (key, value) in &manifest.metadata {
        if key != "content-type" {
            let header_name = format!("x-amz-meta-{}", key);
            if let Ok(header_key) = axum::http::HeaderName::from_bytes(header_name.as_bytes()) {
                if let Ok(header_value) = axum::http::HeaderValue::from_str(value) {
                    headers.insert(header_key, header_value);
                }
            }
        }
    }

    // TODO: Handle range requests, conditional requests, etc.

    Ok(response)
}

/// Checks if an object exists (HEAD object).
///
/// Implements the S3 HeadObject operation with optional version ID support.
/// Returns metadata without the object body.
pub async fn head_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(params): Query<GetObjectParams>,
) -> S3Result<Response> {
    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket));
        }
    }

    // Validate versionId if provided
    if let Some(ref version_id) = params.version_id {
        if !crate::versioning::VersionId::is_valid(version_id) {
            return Err(S3Error::InvalidArgument(format!(
                "Invalid versionId: {}",
                version_id
            )));
        }
    }

    // Check if object exists (with optional version ID)
    let result = if let Some(ref version_id) = params.version_id {
        state
            .storage
            .get_object_version(&bucket, &key, version_id)
            .await
            .map_err(|e| {
                if matches!(e, anode_storage::StorageError::NotFound(_)) {
                    S3Error::NoSuchKey(key.clone())
                } else {
                    S3Error::StorageError(e)
                }
            })?
    } else {
        state.storage.get_object(&bucket, &key).await.map_err(|e| {
            if matches!(e, anode_storage::StorageError::NotFound(_)) {
                S3Error::NoSuchKey(key.clone())
            } else {
                S3Error::StorageError(e)
            }
        })?
    };

    let (manifest, data) = match result {
        Some((manifest, data)) => (manifest, data),
        None => return Err(S3Error::NoSuchKey(key)),
    };

    // If the current object is a delete marker and no version was specified, return 404
    if manifest.is_delete_marker && params.version_id.is_none() {
        let mut response = StatusCode::NOT_FOUND.into_response();
        let headers = response.headers_mut();
        headers.insert("x-amz-delete-marker", "true".parse().unwrap());
        headers.insert("x-amz-version-id", manifest.version_id.parse().unwrap());
        return Ok(response);
    }

    // Get Content-Type from metadata, default to application/octet-stream
    let content_type = manifest
        .metadata
        .get("content-type")
        .map(|s| s.as_str())
        .unwrap_or("application/octet-stream");

    // Calculate ETag from manifest checksum
    let etag = format!("\"{}\"", hex::encode(manifest.checksum));

    // Build response with custom metadata headers
    let mut response = (
        StatusCode::OK,
        [
            ("Content-Type", content_type.to_string()),
            ("Content-Length", data.len().to_string()),
            ("ETag", etag),
            ("x-amz-version-id", manifest.version_id.clone()),
        ],
    )
        .into_response();

    // Add x-amz-meta-* headers from metadata
    let headers = response.headers_mut();
    for (key, value) in &manifest.metadata {
        if key != "content-type" {
            let header_name = format!("x-amz-meta-{}", key);
            if let Ok(header_key) = axum::http::HeaderName::from_bytes(header_name.as_bytes()) {
                if let Ok(header_value) = axum::http::HeaderValue::from_str(value) {
                    headers.insert(header_key, header_value);
                }
            }
        }
    }

    Ok(response)
}

/// Query parameters for DELETE object operations.
#[derive(Deserialize, Debug)]
pub struct DeleteObjectParams {
    #[serde(rename = "versionId")]
    pub version_id: Option<String>,
}

/// Deletes an object or creates a delete marker.
///
/// Implements the S3 DeleteObject operation with support for versioning.
/// - Without versionId: Creates a delete marker for versioned buckets, or permanently deletes for non-versioned buckets
/// - With versionId: Permanently deletes that specific version
pub async fn delete_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(params): Query<DeleteObjectParams>,
) -> S3Result<Response> {
    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket));
        }
    }

    // If version_id is specified, permanently delete that version
    if let Some(ref version_id) = params.version_id {
        // Validate version ID
        if !crate::versioning::VersionId::is_valid(version_id) {
            return Err(S3Error::InvalidArgument(format!(
                "Invalid versionId: {}",
                version_id
            )));
        }

        // Permanently delete the specific version
        match state
            .storage
            .delete_object_version(&bucket, &key, version_id)
            .await
        {
            Ok(Some(manifest)) => {
                // Build response headers
                let mut response = (
                    StatusCode::NO_CONTENT,
                    [("x-amz-version-id", version_id.as_str())],
                )
                    .into_response();

                // If we deleted a delete marker, set the x-amz-delete-marker header
                if manifest.is_delete_marker {
                    let headers = response.headers_mut();
                    headers.insert("x-amz-delete-marker", "true".parse().unwrap());
                }

                return Ok(response);
            },
            Ok(None) => {
                // Version not found - S3 returns 204 anyway
                return Ok((
                    StatusCode::NO_CONTENT,
                    [("x-amz-version-id", version_id.as_str())],
                )
                    .into_response());
            },
            Err(e) => return Err(S3Error::StorageError(e)),
        }
    }

    // No version ID specified - delete the object (creates delete marker for versioned buckets)
    let _ = state.storage.delete_object(&bucket, &key).await;

    // Get the current object to check if it's a delete marker
    match state.storage.get_object(&bucket, &key).await {
        Ok(Some((manifest, _))) if manifest.is_delete_marker => {
            // We just created a delete marker
            Ok((
                StatusCode::NO_CONTENT,
                [
                    ("x-amz-version-id", manifest.version_id.as_str()),
                    ("x-amz-delete-marker", "true"),
                ],
            )
                .into_response())
        },
        _ => {
            // Non-versioned deletion or error - return standard 204
            Ok(StatusCode::NO_CONTENT.into_response())
        },
    }
}

/// Copies an object.
///
/// Implements the S3 CopyObject operation for server-side copies.
pub async fn copy_object(
    State(state): State<AppState>,
    Path((dest_bucket, dest_key)): Path<(String, String)>,
    headers: HeaderMap,
) -> S3Result<Response> {
    // Extract source bucket and key from x-amz-copy-source header
    let copy_source = headers
        .get("x-amz-copy-source")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| S3Error::InvalidRequest("Missing x-amz-copy-source header".to_string()))?;

    // Parse source (format: /bucket/key or bucket/key)
    let source = copy_source.trim_start_matches('/');
    let (src_bucket, src_key) = source
        .split_once('/')
        .ok_or_else(|| S3Error::InvalidRequest("Invalid copy source format".to_string()))?;

    // Check if both buckets exist
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(src_bucket) {
            return Err(S3Error::NoSuchBucket(src_bucket.to_string()));
        }
        if !buckets.contains_key(&dest_bucket) {
            return Err(S3Error::NoSuchBucket(dest_bucket));
        }
    }

    // Get source object
    let result = state
        .storage
        .get_object(src_bucket, src_key)
        .await
        .map_err(|e| {
            if matches!(e, anode_storage::StorageError::NotFound(_)) {
                S3Error::NoSuchKey(src_key.to_string())
            } else {
                S3Error::StorageError(e)
            }
        })?;

    let data = match result {
        Some((_, data)) => data,
        None => return Err(S3Error::NoSuchKey(src_key.to_string())),
    };

    // Store the copy
    let manifest = state
        .storage
        .put_object(&dest_bucket, &dest_key, &data, BTreeMap::new())
        .await?;

    let etag = format!("\"{}\"", hex::encode(manifest.checksum));
    let last_modified = xml::format_timestamp(manifest.modified_at as u64);
    let xml = xml::copy_object_xml(&etag, &last_modified);

    Ok((StatusCode::OK, [("Content-Type", "application/xml")], xml).into_response())
}

/// Lists objects in a bucket (ListObjectsV2).
///
/// Implements the S3 ListObjectsV2 operation with pagination support.
pub async fn list_objects_v2(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    Query(params): Query<ListParams>,
) -> S3Result<Response> {
    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket));
        }
    }

    // Get objects from storage
    let max_keys = params.max_keys.unwrap_or(1000).min(1000);
    let result = state
        .storage
        .list_objects(
            &bucket,
            params.prefix.as_deref().unwrap_or(""),
            max_keys as usize + 1,
            None,
        )
        .await?;

    // Convert to ObjectInfo
    let mut objects: Vec<ObjectInfo> = result
        .objects
        .iter()
        .map(xml::object_manifest_to_info)
        .collect();

    // Apply max-keys limit
    let max_keys = params.max_keys.unwrap_or(1000).min(1000);
    let is_truncated = objects.len() > max_keys as usize;
    objects.truncate(max_keys as usize);

    let result = ListObjectsResult {
        name: bucket,
        prefix: params.prefix,
        max_keys,
        is_truncated,
        key_count: objects.len() as u32,
        contents: objects,
        continuation_token: params.continuation_token,
        next_continuation_token: if is_truncated {
            Some("next-token".to_string()) // TODO: Implement proper pagination
        } else {
            None
        },
    };

    let xml = xml::list_objects_v2_xml(&result);

    Ok((StatusCode::OK, [("Content-Type", "application/xml")], xml).into_response())
}

// ============================================================================
// Multipart Upload Operations
// ============================================================================

/// Initiates a multipart upload.
///
/// Implements the S3 CreateMultipartUpload operation.
pub async fn create_multipart_upload(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    headers: HeaderMap,
) -> S3Result<Response> {
    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket.clone()));
        }
    }

    // Extract metadata from headers
    let mut metadata = BTreeMap::new();
    for (name, value) in headers.iter() {
        if let Some(meta_key) = name.as_str().strip_prefix("x-amz-meta-") {
            if let Ok(value_str) = value.to_str() {
                metadata.insert(meta_key.to_string(), value_str.to_string());
            }
        }
    }

    // Create multipart upload
    let upload_id = state
        .multipart
        .create_upload(&bucket, &key, metadata)
        .await?;

    let xml = xml::initiate_multipart_xml(&bucket, &key, &upload_id);

    Ok((StatusCode::OK, [("Content-Type", "application/xml")], xml).into_response())
}

/// Uploads a part for a multipart upload.
///
/// Implements the S3 UploadPart operation.
pub async fn upload_part(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(params): Query<PartParams>,
    body: Body,
) -> S3Result<Response> {
    // Read the body
    let data = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| S3Error::InternalError(format!("Failed to read body: {}", e)))?;

    // Upload the part
    let part = state
        .multipart
        .upload_part(&params.upload_id, params.part_number, data)
        .await?;

    let etag = format!("\"{}\"", part.etag);

    Ok((StatusCode::OK, [("ETag", etag.as_str())]).into_response())
}

/// Completes a multipart upload.
///
/// Implements the S3 CompleteMultipartUpload operation.
pub async fn complete_multipart_upload(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(params): Query<UploadParams>,
    body: Body,
) -> S3Result<Response> {
    let upload_id = params
        .upload_id
        .ok_or_else(|| S3Error::InvalidRequest("Missing uploadId parameter".to_string()))?;

    // Parse the XML body to get the parts list
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| S3Error::InternalError(format!("Failed to read body: {}", e)))?;

    let parts = xml::parse_complete_multipart_upload(&body_bytes)?;

    // Complete the multipart upload
    let manifest = state.multipart.complete_upload(&upload_id, parts).await?;

    let etag = format!("\"{}\"", hex::encode(manifest.checksum));
    let location = format!("/{}/{}", bucket, key);

    let result = CompleteMultipartResult {
        bucket: bucket.clone(),
        key: key.clone(),
        etag: etag.clone(),
        location,
    };

    let xml = xml::complete_multipart_xml(&result);

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/xml"), ("ETag", &etag)],
        xml,
    )
        .into_response())
}

/// Aborts a multipart upload.
///
/// Implements the S3 AbortMultipartUpload operation.
pub async fn abort_multipart_upload(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(params): Query<UploadParams>,
) -> S3Result<Response> {
    let upload_id = params
        .upload_id
        .ok_or_else(|| S3Error::InvalidRequest("Missing uploadId parameter".to_string()))?;

    // Abort the multipart upload
    state.multipart.abort_upload(&upload_id).await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Lists parts of a multipart upload.
///
/// Implements the S3 ListParts operation.
pub async fn list_parts(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(params): Query<ListPartsParams>,
) -> S3Result<Response> {
    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket.clone()));
        }
    }

    // List parts
    let result = state
        .multipart
        .list_parts(
            &params.upload_id,
            params.max_parts,
            params.part_number_marker,
        )
        .await?;

    let xml = xml::list_parts_xml(&result);

    Ok((StatusCode::OK, [("Content-Type", "application/xml")], xml).into_response())
}

/// Lists multipart uploads for a bucket.
///
/// Implements the S3 ListMultipartUploads operation.
pub async fn list_multipart_uploads(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    Query(params): Query<ListUploadsParams>,
) -> S3Result<Response> {
    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket.clone()));
        }
    }

    // List uploads
    let result = state
        .multipart
        .list_uploads(
            &bucket,
            params.prefix.as_deref(),
            params.max_uploads,
            params.key_marker.as_deref(),
            params.upload_id_marker.as_deref(),
        )
        .await?;

    let xml = xml::list_multipart_uploads_xml(&result);

    Ok((StatusCode::OK, [("Content-Type", "application/xml")], xml).into_response())
}

/// Deletes multiple objects (POST with XML body).
///
/// Implements the S3 DeleteObjects (batch delete) operation.
pub async fn delete_objects(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    body: Body,
) -> S3Result<Response> {
    use crate::batch::{BatchHandler, ObjectIdentifier};

    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket.clone()));
        }
    }

    // Read and parse XML body
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| S3Error::InternalError(format!("Failed to read body: {}", e)))?;

    let body_str = String::from_utf8(body_bytes.to_vec())
        .map_err(|e| S3Error::InvalidRequest(format!("Invalid UTF-8: {}", e)))?;

    let request = xml::parse_delete_objects_request(&body_str)
        .map_err(|e| S3Error::InvalidRequest(format!("Invalid XML: {}", e)))?;

    // Convert to ObjectIdentifier
    let objects: Vec<ObjectIdentifier> = request
        .objects
        .into_iter()
        .map(|obj| {
            if let Some(version_id) = obj.version_id {
                ObjectIdentifier::with_version(obj.key, version_id)
            } else {
                ObjectIdentifier::new(obj.key)
            }
        })
        .collect();

    // Perform batch delete
    let handler = BatchHandler::new(state.storage.clone());
    let result = handler
        .delete_objects(&bucket, objects, request.quiet)
        .await?;

    // Convert to XML types
    let deleted: Vec<xml::BatchDeletedObject> = result
        .deleted
        .into_iter()
        .map(|d| xml::BatchDeletedObject {
            key: d.key,
            version_id: d.version_id,
            delete_marker: d.delete_marker,
            delete_marker_version_id: d.delete_marker_version_id,
        })
        .collect();

    let errors: Vec<xml::BatchDeleteError> = result
        .errors
        .into_iter()
        .map(|e| xml::BatchDeleteError {
            key: e.key,
            version_id: e.version_id,
            code: e.code,
            message: e.message,
        })
        .collect();

    let xml = xml::batch_delete_xml(&deleted, &errors);

    Ok((StatusCode::OK, [("Content-Type", "application/xml")], xml).into_response())
}

/// Lists object versions in a bucket (ListObjectVersions).
///
/// Implements the S3 ListObjectVersions operation with pagination support.
pub async fn list_object_versions(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    Query(params): Query<ListVersionsParams>,
) -> S3Result<Response> {
    // Check if bucket exists
    {
        let buckets = state.buckets.read().await;
        if !buckets.contains_key(&bucket) {
            return Err(S3Error::NoSuchBucket(bucket));
        }
    }

    // Get versions from storage
    let max_keys = params.max_keys.unwrap_or(1000).min(1000);
    let result = state
        .storage
        .list_object_versions(
            &bucket,
            params.prefix.as_deref().unwrap_or(""),
            max_keys as usize,
            params.key_marker.as_deref(),
            params.version_id_marker.as_deref(),
        )
        .await?;

    // Convert manifests to version info
    let versions: Vec<xml::VersionInfo> = result
        .objects
        .iter()
        .map(xml::object_manifest_to_version_info)
        .collect();

    // Determine next markers if truncated
    let (next_key_marker, next_version_id_marker) = if result.is_truncated {
        versions
            .last()
            .map(|v| (Some(v.key.clone()), Some(v.version_id.clone())))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    let list_result = xml::ListVersionsResult {
        name: bucket,
        prefix: params.prefix,
        max_keys,
        is_truncated: result.is_truncated,
        versions,
        delete_markers: vec![], // Already included in versions
        key_marker: params.key_marker,
        version_id_marker: params.version_id_marker,
        next_key_marker,
        next_version_id_marker,
    };

    let xml = xml::list_versions_xml(&list_result);

    Ok((StatusCode::OK, [("Content-Type", "application/xml")], xml).into_response())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Validates a bucket name according to S3 rules.
///
/// Ensures the bucket name follows S3 naming conventions:
/// - 3-63 characters long
/// - Only lowercase letters, numbers, hyphens, and periods
/// - Cannot start or end with a hyphen
fn validate_bucket_name(name: &str) -> S3Result<()> {
    if name.len() < 3 || name.len() > 63 {
        return Err(S3Error::InvalidBucketName(
            "Bucket name must be between 3 and 63 characters".to_string(),
        ));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
    {
        return Err(S3Error::InvalidBucketName(
            "Bucket name can only contain lowercase letters, numbers, hyphens, and periods"
                .to_string(),
        ));
    }

    if name.starts_with('-') || name.ends_with('-') {
        return Err(S3Error::InvalidBucketName(
            "Bucket name cannot start or end with a hyphen".to_string(),
        ));
    }

    Ok(())
}

/// Represents a byte range for partial object retrieval.
///
/// Used for implementing HTTP Range requests.
#[derive(Debug, PartialEq, Eq)]
pub enum ByteRange {
    /// Range from start to end (inclusive), e.g., "bytes=0-100".
    FromTo { start: u64, end: Option<u64> },
    /// Suffix range: last N bytes, e.g., "bytes=-100".
    Suffix { last: u64 },
}

/// Parses the Range header value.
///
/// Supports formats like "bytes=0-100", "bytes=100-", or "bytes=-100".
/// Returns None if the header is invalid or not present.
fn parse_range_header(range_value: &str) -> Option<ByteRange> {
    // Range header format: "bytes=start-end", "bytes=start-", or "bytes=-last"
    let range_value = range_value.trim();

    // Must start with "bytes="
    if !range_value.starts_with("bytes=") {
        return None;
    }

    // Extract the range part after "bytes="
    let range_part = &range_value[6..];

    // Split by '-' to get start and end
    let parts: Vec<&str> = range_part.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    let start_str = parts[0].trim();
    let end_str = parts[1].trim();

    // Check for suffix range format: "bytes=-100" (last N bytes)
    if start_str.is_empty() && !end_str.is_empty() {
        let last = end_str.parse::<u64>().ok()?;
        return Some(ByteRange::Suffix { last });
    }

    // Parse start (required for FromTo ranges)
    let start = start_str.parse::<u64>().ok()?;

    // Parse end (optional for open-ended ranges like "bytes=100-")
    let end = if end_str.is_empty() {
        None
    } else {
        let end_value = end_str.parse::<u64>().ok()?;
        // Validate that start <= end when end is specified
        if start > end_value {
            return None;
        }
        Some(end_value)
    };

    Some(ByteRange::FromTo { start, end })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn test_bucket_policy_workflow() {
        // Test that BucketMetadata can store and retrieve policies
        let policy_json = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": "*",
                    "Action": "s3:GetObject",
                    "Resource": "arn:aws:s3:::mybucket/*"
                }
            ]
        }"#;

        let mut metadata = BucketMetadata {
            name: "test-bucket".to_string(),
            creation_date: 123456,
            policy: None,
            lifecycle: None,
        };

        // Initially no policy
        assert!(metadata.policy.is_none());

        // Set policy
        metadata.policy = Some(policy_json.to_string());
        assert!(metadata.policy.is_some());

        // Verify policy can be retrieved
        let retrieved = metadata.policy.as_ref().unwrap();
        assert!(retrieved.contains("2012-10-17"));
        assert!(retrieved.contains("s3:GetObject"));

        // Clear policy
        metadata.policy = None;
        assert!(metadata.policy.is_none());
    }

    #[test]
    fn test_validate_bucket_name() {
        assert!(validate_bucket_name("my-bucket").is_ok());
        assert!(validate_bucket_name("my.bucket.123").is_ok());

        assert!(validate_bucket_name("ab").is_err()); // Too short
        assert!(validate_bucket_name(&"a".repeat(64)).is_err()); // Too long
        assert!(validate_bucket_name("MyBucket").is_err()); // Uppercase
        assert!(validate_bucket_name("-bucket").is_err()); // Starts with hyphen
        assert!(validate_bucket_name("bucket-").is_err()); // Ends with hyphen
    }

    #[test]
    fn test_etag_computation_single_part() {
        // Test data
        let data = b"Hello, World!";

        // Compute expected ETag using SHA-256 (what the storage engine uses)
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        let expected_etag = hex::encode(hash);

        // Verify the expected format matches what put_object would return
        // The ETag should be the hex-encoded SHA-256 hash
        assert_eq!(expected_etag.len(), 64); // SHA-256 produces 32 bytes = 64 hex chars

        // Test with another known value
        let data2 = b"";
        let mut hasher2 = Sha256::new();
        hasher2.update(data2);
        let hash2 = hasher2.finalize();
        let etag2 = hex::encode(hash2);

        // SHA-256 of empty string
        assert_eq!(
            etag2,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // Test with a longer string
        let data3 = b"The quick brown fox jumps over the lazy dog";
        let mut hasher3 = Sha256::new();
        hasher3.update(data3);
        let hash3 = hasher3.finalize();
        let etag3 = hex::encode(hash3);

        // Known SHA-256 hash for this phrase
        assert_eq!(
            etag3,
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }

    #[test]
    fn test_parse_range_header_basic() {
        // Test basic range parsing: "bytes=0-100"
        let range = parse_range_header("bytes=0-100");
        assert_eq!(
            range,
            Some(ByteRange::FromTo {
                start: 0,
                end: Some(100)
            })
        );
    }

    #[test]
    fn test_parse_range_header_various_formats() {
        // Test with whitespace
        assert_eq!(
            parse_range_header("bytes=0-100"),
            Some(ByteRange::FromTo {
                start: 0,
                end: Some(100)
            })
        );

        // Test with larger values
        assert_eq!(
            parse_range_header("bytes=1024-2048"),
            Some(ByteRange::FromTo {
                start: 1024,
                end: Some(2048)
            })
        );

        // Test with same start and end
        assert_eq!(
            parse_range_header("bytes=50-50"),
            Some(ByteRange::FromTo {
                start: 50,
                end: Some(50)
            })
        );

        // Test with large numbers
        assert_eq!(
            parse_range_header("bytes=1000000-2000000"),
            Some(ByteRange::FromTo {
                start: 1000000,
                end: Some(2000000)
            })
        );
    }

    #[test]
    fn test_parse_range_header_invalid() {
        // Test invalid formats
        assert_eq!(parse_range_header(""), None);
        assert_eq!(parse_range_header("invalid"), None);
        assert_eq!(parse_range_header("bytes="), None);
        assert_eq!(parse_range_header("bytes=abc-def"), None);
        assert_eq!(parse_range_header("bytes=100"), None);

        // Test where start > end (invalid)
        assert_eq!(parse_range_header("bytes=100-50"), None);

        // Test wrong prefix
        assert_eq!(parse_range_header("chunks=0-100"), None);
    }

    #[test]
    fn test_parse_range_header_open_ended() {
        // Test open-ended range: "bytes=100-" (from byte 100 to end)
        let range = parse_range_header("bytes=100-");
        assert_eq!(
            range,
            Some(ByteRange::FromTo {
                start: 100,
                end: None
            })
        );

        // Test open-ended range from byte 0
        assert_eq!(
            parse_range_header("bytes=0-"),
            Some(ByteRange::FromTo {
                start: 0,
                end: None
            })
        );

        // Test open-ended range with larger start value
        assert_eq!(
            parse_range_header("bytes=1048576-"),
            Some(ByteRange::FromTo {
                start: 1048576,
                end: None
            })
        );

        // Test open-ended range with whitespace
        assert_eq!(
            parse_range_header("bytes=500- "),
            Some(ByteRange::FromTo {
                start: 500,
                end: None
            })
        );
    }

    #[test]
    fn test_parse_range_header_suffix() {
        // Test suffix range: "bytes=-100" (last 100 bytes)
        let range = parse_range_header("bytes=-100");
        assert_eq!(range, Some(ByteRange::Suffix { last: 100 }));

        // Test suffix range with different values
        assert_eq!(
            parse_range_header("bytes=-1"),
            Some(ByteRange::Suffix { last: 1 })
        );

        assert_eq!(
            parse_range_header("bytes=-500"),
            Some(ByteRange::Suffix { last: 500 })
        );

        assert_eq!(
            parse_range_header("bytes=-1048576"),
            Some(ByteRange::Suffix { last: 1048576 })
        );

        // Test suffix range with whitespace
        assert_eq!(
            parse_range_header("bytes=-100 "),
            Some(ByteRange::Suffix { last: 100 })
        );

        // Test that suffix with non-numeric value is invalid
        assert_eq!(parse_range_header("bytes=-abc"), None);
    }
}
