// Request middleware for adding request_id to all logs
//
// This module provides middleware that generates a unique request ID for each
// incoming request and adds it as a tracing span field, making it available
// in all log entries within that request's context.

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::Instrument;
use uuid::Uuid;

/// Header name for the request ID
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Middleware that generates a unique request ID and adds it to the tracing span
///
/// This middleware:
/// 1. Checks if the request already has an x-request-id header
/// 2. If not, generates a new UUID
/// 3. Creates a tracing span with the request_id field
/// 4. Processes the request within that span
/// 5. Adds the request_id to the response headers
pub async fn request_id_middleware(request: Request, next: Next) -> Response {
    // Extract or generate request ID
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Create a span with the request_id field
    let span = tracing::info_span!(
        "request",
        request_id = %request_id,
        method = %request.method(),
        uri = %request.uri(),
    );

    // Process the request within the span
    let response = async move {
        tracing::debug!("Processing request");
        next.run(request).await
    }
    .instrument(span)
    .await;

    // Add request ID to response headers
    let (mut parts, body) = response.into_parts();
    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        parts.headers.insert(REQUEST_ID_HEADER, header_value);
    }

    Response::from_parts(parts, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        response::IntoResponse,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn test_handler() -> impl IntoResponse {
        tracing::info!("Handler executed");
        StatusCode::OK
    }

    #[tokio::test]
    async fn test_request_id_generation() {
        let app = Router::new()
            .route("/", get(test_handler))
            .layer(middleware::from_fn(request_id_middleware));

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Check that response has request ID header
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));
    }

    #[tokio::test]
    async fn test_request_id_preservation() {
        let app = Router::new()
            .route("/", get(test_handler))
            .layer(middleware::from_fn(request_id_middleware));

        let request_id = "test-request-id-123";
        let request = Request::builder()
            .uri("/")
            .header(REQUEST_ID_HEADER, request_id)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Check that the same request ID is returned
        let response_id = response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok());

        assert_eq!(response_id, Some(request_id));
    }
}
