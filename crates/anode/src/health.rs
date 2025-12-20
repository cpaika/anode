use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

use anode_raft::AnodeRaft;
use anode_storage::StorageEngine;

use crate::cluster::ClusterManager;

/// Simple health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: String,
}

/// Detailed health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedHealth {
    pub storage: ComponentHealth,
    pub raft: ComponentHealth,
    pub cluster: ComponentHealth,
    pub overall: HealthStatus,
}

/// Health status of a single component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub healthy: bool,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Health checker for monitoring system health
pub struct HealthChecker {
    storage: Arc<StorageEngine>,
    raft: Arc<AnodeRaft>,
    cluster_manager: Arc<ClusterManager>,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(
        storage: Arc<StorageEngine>,
        raft: Arc<AnodeRaft>,
        cluster_manager: Arc<ClusterManager>,
    ) -> Self {
        Self {
            storage,
            raft,
            cluster_manager,
        }
    }

    /// Check readiness (ready to serve requests)
    pub async fn readiness(&self) -> HealthStatus {
        debug!("Checking readiness");

        // Check if Raft has a leader
        let has_leader = self.raft.current_leader().await.is_some();

        if !has_leader {
            return HealthStatus {
                healthy: false,
                message: "No Raft leader elected".to_string(),
            };
        }

        // Check if storage is accessible
        // TODO: Add actual storage health check
        // For now, assume healthy if initialized

        HealthStatus {
            healthy: true,
            message: "Ready".to_string(),
        }
    }

    /// Check liveness (server is alive)
    pub async fn liveness(&self) -> HealthStatus {
        debug!("Checking liveness");

        // Basic liveness check - if we can run this code, we're alive
        HealthStatus {
            healthy: true,
            message: "Alive".to_string(),
        }
    }

    /// Get detailed health information
    pub async fn detailed(&self) -> DetailedHealth {
        debug!("Performing detailed health check");

        let storage_health = self.check_storage_health().await;
        let raft_health = self.check_raft_health().await;
        let cluster_health = self.check_cluster_health().await;

        let overall_healthy = storage_health.healthy
            && raft_health.healthy
            && cluster_health.healthy;

        let overall = HealthStatus {
            healthy: overall_healthy,
            message: if overall_healthy {
                "All systems operational".to_string()
            } else {
                "One or more systems degraded".to_string()
            },
        };

        DetailedHealth {
            storage: storage_health,
            raft: raft_health,
            cluster: cluster_health,
            overall,
        }
    }

    /// Check storage component health
    async fn check_storage_health(&self) -> ComponentHealth {
        // TODO: Implement actual storage health checks
        // - Check disk space
        // - Check I/O performance
        // - Check metadata database

        ComponentHealth {
            healthy: true,
            message: "Storage engine operational".to_string(),
            details: Some(serde_json::json!({
                "engine": "operational"
            })),
        }
    }

    /// Check Raft component health
    async fn check_raft_health(&self) -> ComponentHealth {
        let metrics = match self.raft.metrics().borrow().clone() {
            metrics => metrics,
        };

        let has_leader = metrics.current_leader.is_some();
        let is_leader = metrics.current_leader == Some(metrics.id);

        let details = serde_json::json!({
            "node_id": metrics.id,
            "current_term": metrics.current_term,
            "current_leader": metrics.current_leader,
            "is_leader": is_leader,
            "membership": format!("{:?}", metrics.membership_config),
        });

        ComponentHealth {
            healthy: has_leader,
            message: if has_leader {
                if is_leader {
                    "Leader".to_string()
                } else {
                    format!("Follower (leader: {:?})", metrics.current_leader)
                }
            } else {
                "No leader elected".to_string()
            },
            details: Some(details),
        }
    }

    /// Check cluster component health
    async fn check_cluster_health(&self) -> ComponentHealth {
        let status = self.cluster_manager.get_status().await;

        let healthy = status.leader_id.is_some() && status.total_nodes > 0;

        let details = serde_json::json!({
            "total_nodes": status.total_nodes,
            "healthy_nodes": status.healthy_nodes,
            "leader_id": status.leader_id,
            "placement_groups": status.placement_groups.len(),
        });

        ComponentHealth {
            healthy,
            message: format!(
                "{}/{} nodes healthy",
                status.healthy_nodes,
                status.total_nodes
            ),
            details: Some(details),
        }
    }
}

impl HealthStatus {
    /// Create a healthy status
    pub fn healthy(message: impl Into<String>) -> Self {
        Self {
            healthy: true,
            message: message.into(),
        }
    }

    /// Create an unhealthy status
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            healthy: false,
            message: message.into(),
        }
    }
}

impl ComponentHealth {
    /// Create a healthy component status
    pub fn healthy(message: impl Into<String>) -> Self {
        Self {
            healthy: true,
            message: message.into(),
            details: None,
        }
    }

    /// Create an unhealthy component status
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            healthy: false,
            message: message.into(),
            details: None,
        }
    }

    /// Add details to the health status
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_creation() {
        let healthy = HealthStatus::healthy("OK");
        assert!(healthy.healthy);
        assert_eq!(healthy.message, "OK");

        let unhealthy = HealthStatus::unhealthy("Error");
        assert!(!unhealthy.healthy);
        assert_eq!(unhealthy.message, "Error");
    }

    #[test]
    fn test_component_health_with_details() {
        let health = ComponentHealth::healthy("OK")
            .with_details(serde_json::json!({ "foo": "bar" }));

        assert!(health.healthy);
        assert!(health.details.is_some());
    }
}
