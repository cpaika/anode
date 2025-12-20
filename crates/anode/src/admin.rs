use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

use crate::cluster::{ClusterManager, ClusterStatus};
use crate::health::{DetailedHealth, HealthChecker, HealthStatus};
use crate::metrics::Metrics;

use anode_raft::AnodeRaft;

/// Shared state for admin API handlers
#[derive(Clone)]
pub struct AdminState {
    pub health_checker: Arc<HealthChecker>,
    pub cluster_manager: Arc<ClusterManager>,
    pub metrics: Arc<Metrics>,
    pub raft: Arc<AnodeRaft>,
}

/// Node status information
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeStatus {
    pub node_id: u64,
    pub is_leader: bool,
    pub current_term: u64,
    pub current_leader: Option<u64>,
    pub uptime_seconds: f64,
}

/// Create the admin API router
pub fn admin_router(state: AdminState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
        .route("/health/detailed", get(detailed_health))
        .route("/metrics", get(metrics))
        .route("/cluster/status", get(cluster_status))
        .route("/node/status", get(node_status))
        .with_state(state)
}

/// Health check endpoint
async fn health(State(state): State<AdminState>) -> Json<HealthStatus> {
    debug!("Health check requested");
    let status = state.health_checker.readiness().await;
    Json(status)
}

/// Liveness check endpoint (Kubernetes-style)
async fn liveness(State(state): State<AdminState>) -> Json<HealthStatus> {
    debug!("Liveness check requested");
    let status = state.health_checker.liveness().await;
    Json(status)
}

/// Readiness check endpoint (Kubernetes-style)
async fn readiness(State(state): State<AdminState>) -> Json<HealthStatus> {
    debug!("Readiness check requested");
    let status = state.health_checker.readiness().await;
    Json(status)
}

/// Detailed health check endpoint
async fn detailed_health(State(state): State<AdminState>) -> Json<DetailedHealth> {
    debug!("Detailed health check requested");
    let health = state.health_checker.detailed().await;
    Json(health)
}

/// Prometheus metrics endpoint
async fn metrics(State(state): State<AdminState>) -> Response {
    debug!("Metrics requested");
    let metrics_text = state.metrics.export();

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain; version=0.0.4")
        .body(metrics_text.into())
        .unwrap()
}

/// Cluster status endpoint
async fn cluster_status(State(state): State<AdminState>) -> Json<ClusterStatus> {
    debug!("Cluster status requested");
    let status = state.cluster_manager.get_status().await;
    Json(status)
}

/// Node status endpoint
async fn node_status(State(state): State<AdminState>) -> Json<NodeStatus> {
    debug!("Node status requested");

    let raft_metrics = state.raft.metrics().borrow().clone();

    let status = NodeStatus {
        node_id: raft_metrics.id,
        is_leader: raft_metrics.current_leader == Some(raft_metrics.id),
        current_term: raft_metrics.current_term,
        current_leader: raft_metrics.current_leader,
        uptime_seconds: state.metrics.uptime_seconds.get(),
    };

    Json(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn create_test_state() -> AdminState {
        // This would require setting up test instances
        // Skipping actual implementation for now
        todo!("Implement test state creation")
    }

    #[tokio::test]
    #[ignore] // Ignore until we have proper test setup
    async fn test_health_endpoint() {
        let state = create_test_state().await;
        let app = admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
