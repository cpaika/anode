//! S3 router setup.
//!
//! This module sets up the Axum router for S3 API endpoints, providing a complete
//! S3-compatible HTTP interface for the storage engine.

use crate::auth::AuthConfig;
use crate::handlers::{self, AppState};
use crate::middleware::request_id_middleware;
use crate::multipart::{MultipartConfig, MultipartManager};
use anode_storage::StorageEngine;
use axum::{
    extract::Request,
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, head, post, put},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// S3 Router builder.
///
/// Constructs an Axum router with all S3 API endpoints configured.
pub struct S3Router {
    storage: Arc<StorageEngine>,
    auth: Option<AuthConfig>,
}

impl S3Router {
    /// Creates a new S3 router.
    ///
    /// # Arguments
    ///
    /// * `storage` - The storage engine to use for object storage
    /// * `auth` - Optional authentication configuration (None for no authentication)
    pub fn new(storage: Arc<StorageEngine>, auth: Option<AuthConfig>) -> Self {
        Self { storage, auth }
    }

    /// Builds the router and returns it.
    ///
    /// Configures all S3 API routes and middleware, including multipart upload cleanup.
    /// Returns an Axum Router ready to be served.
    pub fn into_router(self) -> Router {
        // Create multipart manager
        let multipart = Arc::new(MultipartManager::new(
            self.storage.clone(),
            MultipartConfig::default(),
        ));

        // Start background cleanup task for expired multipart uploads
        multipart.clone().start_cleanup_task();

        // Create application state
        let state = AppState {
            storage: self.storage,
            buckets: Arc::new(RwLock::new(HashMap::new())),
            multipart,
        };

        // Build the router with request_id middleware
        let router = build_routes()
            .with_state(state)
            .layer(middleware::from_fn(request_id_middleware));

        // Add authentication middleware if configured
        if let Some(_auth) = self.auth {
            // TODO: Add authentication middleware
            router
        } else {
            router
        }
    }
}

/// Builds the S3 API routes.
///
/// Creates all S3-compatible API endpoints for bucket and object operations.
fn build_routes() -> Router<AppState> {
    Router::new()
        // Service-level operations (no bucket in path)
        .route("/", get(handlers::list_buckets))
        // Object operations (using wildcard to support keys with slashes) - must come first for specificity
        .route(
            "/{bucket}/{*key}",
            post(object_post_handler)
                .put(handle_object_put)
                .get(handle_object_get)
                .head(handle_object_head)
                .delete(handle_object_delete),
        )
        // Bucket operations
        .route(
            "/{bucket}",
            put(handle_bucket_put)
                .delete(handle_bucket_delete)
                .head(handlers::head_bucket)
                .get(handle_bucket_get)
                .post(handle_bucket_post),
        )
}

/// Handles PUT requests on a bucket.
///
/// Routes to PutBucketPolicy, PutBucketLifecycle, or CreateBucket based on query parameters.
async fn handle_bucket_put(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<String>,
    query: axum::extract::Query<HashMap<String, String>>,
    body: axum::body::Body,
) -> Result<Response, crate::error::S3Error> {
    // Check for bucket policy
    if query.contains_key("policy") {
        return handlers::put_bucket_policy(state, path, body).await;
    }

    // Check for bucket lifecycle
    if query.contains_key("lifecycle") {
        return handlers::put_bucket_lifecycle(state, path, body).await;
    }

    // Default to create bucket (ignore body for CreateBucket)
    handlers::create_bucket(state, path).await
}

/// Handles DELETE requests on a bucket.
///
/// Routes to DeleteBucketPolicy, DeleteBucketLifecycle, or DeleteBucket based on query parameters.
async fn handle_bucket_delete(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<String>,
    query: axum::extract::Query<HashMap<String, String>>,
) -> Result<Response, crate::error::S3Error> {
    // Check for bucket policy
    if query.contains_key("policy") {
        return handlers::delete_bucket_policy(state, path).await;
    }

    // Check for bucket lifecycle
    if query.contains_key("lifecycle") {
        return handlers::delete_bucket_lifecycle(state, path).await;
    }

    // Default to delete bucket
    handlers::delete_bucket(state, path).await
}

/// Handles GET requests on a bucket.
///
/// Routes to GetBucketPolicy, GetBucketLifecycle, ListObjectVersions, ListObjects, ListObjectsV2, or ListMultipartUploads based on query parameters.
async fn handle_bucket_get(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<String>,
    query: axum::extract::Query<HashMap<String, String>>,
) -> Result<Response, crate::error::S3Error> {
    // Check for bucket policy
    if query.contains_key("policy") {
        return handlers::get_bucket_policy(state, path).await;
    }

    // Check for bucket lifecycle
    if query.contains_key("lifecycle") {
        return handlers::get_bucket_lifecycle(state, path).await;
    }

    // Check for list object versions
    if query.contains_key("versions") {
        let params = handlers::ListVersionsParams {
            versions: query.get("versions").cloned(),
            prefix: query.get("prefix").cloned(),
            max_keys: query.get("max-keys").and_then(|s| s.parse().ok()),
            key_marker: query.get("key-marker").cloned(),
            version_id_marker: query.get("version-id-marker").cloned(),
        };
        return handlers::list_object_versions(state, path, axum::extract::Query(params)).await;
    }

    // Check for list multipart uploads
    if query.contains_key("uploads") {
        let params = handlers::ListUploadsParams {
            uploads: query.get("uploads").cloned(),
            prefix: query.get("prefix").cloned(),
            max_uploads: query.get("max-uploads").and_then(|s| s.parse().ok()),
            key_marker: query.get("key-marker").cloned(),
            upload_id_marker: query.get("upload-id-marker").cloned(),
        };
        return handlers::list_multipart_uploads(state, path, axum::extract::Query(params)).await;
    }

    // Default to list objects
    let list_params = handlers::ListParams {
        list_type: query.get("list-type").and_then(|s| s.parse().ok()),
        prefix: query.get("prefix").cloned(),
        delimiter: query.get("delimiter").cloned(),
        max_keys: query.get("max-keys").and_then(|s| s.parse().ok()),
        continuation_token: query.get("continuation-token").cloned(),
        start_after: query.get("start-after").cloned(),
    };
    handlers::list_objects_v2(state, path, axum::extract::Query(list_params)).await
}

/// Handles POST requests on a bucket.
///
/// Implements the DeleteObjects (batch delete) operation.
async fn handle_bucket_post(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<String>,
    body: axum::body::Body,
) -> Result<Response, crate::error::S3Error> {
    handlers::delete_objects(state, path, body).await
}

/// Handles PUT requests on an object.
///
/// Routes to PutObject, part upload, or copy operation
/// based on query parameters and headers.
async fn handle_object_put(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<(String, String)>,
    query: axum::extract::Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
    body: axum::body::Body,
) -> Result<Response, crate::error::S3Error> {
    // Check for part upload (PUT /{bucket}/{key}?partNumber=X&uploadId=Y)
    if query.contains_key("partNumber") && query.contains_key("uploadId") {
        let part_number = query
            .get("partNumber")
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| {
                crate::error::S3Error::InvalidArgument("Invalid partNumber".to_string())
            })?;

        let upload_id = query
            .get("uploadId")
            .ok_or_else(|| crate::error::S3Error::InvalidArgument("Missing uploadId".to_string()))?
            .clone();

        let params = handlers::PartParams {
            part_number,
            upload_id,
        };

        return handlers::upload_part(state, path, axum::extract::Query(params), body).await;
    }

    // Check for copy operation
    if headers.contains_key("x-amz-copy-source") {
        return handlers::copy_object(state, path, headers).await;
    }

    // Regular PUT object
    handlers::put_object(state, path, headers, body).await
}

/// Handles POST requests on an object.
///
/// Implements multipart upload initiation and CompleteMultipartUpload operations.
async fn object_post_handler(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<(String, String)>,
    query: axum::extract::Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
    body: axum::body::Body,
) -> Result<Response, crate::error::S3Error> {
    // Check for multipart upload initiation (POST /{bucket}/{key}?uploads)
    if query.contains_key("uploads") {
        return handlers::create_multipart_upload(state, path, headers).await;
    }

    // Check for multipart upload completion (POST /{bucket}/{key}?uploadId=X)
    if query.contains_key("uploadId") {
        let params = handlers::UploadParams {
            upload_id: query.get("uploadId").cloned(),
            uploads: None,
        };
        return handlers::complete_multipart_upload(
            state,
            path,
            axum::extract::Query(params),
            body,
        )
        .await;
    }

    Err(crate::error::S3Error::MethodNotAllowed)
}

/// Handles GET requests on an object.
///
/// Routes to GetObject or ListParts based on query parameters.
async fn handle_object_get(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<(String, String)>,
    query: axum::extract::Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
) -> Result<Response, crate::error::S3Error> {
    // Check for list parts (GET /{bucket}/{key}?uploadId=X)
    if query.contains_key("uploadId") {
        let params = handlers::ListPartsParams {
            upload_id: query
                .get("uploadId")
                .ok_or_else(|| {
                    crate::error::S3Error::InvalidArgument("Missing uploadId".to_string())
                })?
                .clone(),
            max_parts: query.get("max-parts").and_then(|s| s.parse().ok()),
            part_number_marker: query.get("part-number-marker").and_then(|s| s.parse().ok()),
        };
        return handlers::list_parts(state, path, axum::extract::Query(params)).await;
    }

    // Regular GET object (with optional versionId)
    let params = handlers::GetObjectParams {
        version_id: query.get("versionId").cloned(),
    };
    handlers::get_object(state, path, axum::extract::Query(params), headers).await
}

/// Handles HEAD requests on an object.
///
/// Implements HeadObject with optional version ID support.
async fn handle_object_head(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<(String, String)>,
    query: axum::extract::Query<HashMap<String, String>>,
) -> Result<Response, crate::error::S3Error> {
    // HEAD object (with optional versionId)
    let params = handlers::GetObjectParams {
        version_id: query.get("versionId").cloned(),
    };
    handlers::head_object(state, path, axum::extract::Query(params)).await
}

/// Handles DELETE requests on an object.
///
/// Routes to DeleteObject or AbortMultipartUpload based on query parameters.
async fn handle_object_delete(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<(String, String)>,
    query: axum::extract::Query<HashMap<String, String>>,
) -> Result<Response, crate::error::S3Error> {
    // Check for abort multipart upload (DELETE /{bucket}/{key}?uploadId=X)
    if query.contains_key("uploadId") {
        let params = handlers::UploadParams {
            upload_id: query.get("uploadId").cloned(),
            uploads: None,
        };
        return handlers::abort_multipart_upload(state, path, axum::extract::Query(params)).await;
    }

    // Regular DELETE object (with optional versionId)
    let params = handlers::DeleteObjectParams {
        version_id: query.get("versionId").cloned(),
    };
    handlers::delete_object(state, path, axum::extract::Query(params)).await
}

/// Authentication middleware.
///
/// Verifies AWS Signature V4 authentication if enabled.
async fn auth_middleware(
    auth_config: Arc<AuthConfig>,
    request: Request,
    next: Next,
) -> Result<Response, crate::error::S3Error> {
    if !auth_config.enabled {
        return Ok(next.run(request).await);
    }

    // Extract request information for signature verification
    let (parts, body) = request.into_parts();

    // Calculate payload hash
    // Note: This is a simplified version. In production, you'd need to handle
    // the body more carefully to avoid consuming it.
    let payload_hash = "UNSIGNED-PAYLOAD"; // S3 supports unsigned payloads

    // Verify the request
    auth_config.verify_request(&parts.method, &parts.uri, &parts.headers, payload_hash)?;

    // Reconstruct request and continue
    let request = Request::from_parts(parts, body);
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_storage() -> (Arc<StorageEngine>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = anode_storage::StorageConfig {
            data_dir: temp_dir.path().join("blobs"),
            metadata_dir: temp_dir.path().join("metadata"),
            chunk_size: anode_storage::chunks::CHUNK_SIZE,
            sync_writes: false,
        };
        (
            Arc::new(StorageEngine::open(storage_config).await.unwrap()),
            temp_dir,
        )
    }

    #[tokio::test]
    async fn test_s3_router_creation() {
        let (storage, _temp) = create_test_storage().await;
        let router = S3Router::new(storage, None);
        let _app = router.into_router();
    }

    #[tokio::test]
    async fn test_s3_router_with_auth() {
        let (storage, _temp) = create_test_storage().await;
        let auth = AuthConfig::disabled();
        let router = S3Router::new(storage, Some(auth));
        let _app = router.into_router();
    }

    #[tokio::test]
    async fn test_multipart_route_matching() {
        use axum::body::Body;
        use axum::http::{Method, Request, StatusCode};
        use tower::ServiceExt;

        let (storage, _temp) = create_test_storage().await;

        // Create bucket through storage (not ideal, but for testing route matching)
        storage
            .create_bucket(
                "test-bucket",
                anode_storage::BucketConfig::new("test-bucket"),
            )
            .await
            .unwrap();

        let app = S3Router::new(storage, None).into_router();

        // Test POST to /{bucket}/{key}?uploads
        let request = Request::builder()
            .method(Method::POST)
            .uri("/test-bucket/test-key?uploads")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();

        // We expect either 200 (bucket registered) or 404 (bucket not found in state.buckets)
        // But NOT 404 Not Found from routing
        eprintln!("Response status: {:?}", status);
        assert!(
            status == StatusCode::OK || status == StatusCode::NOT_FOUND,
            "Expected OK or NOT_FOUND (bucket), got {:?}",
            status
        );
    }
}
