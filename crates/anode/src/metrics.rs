use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec,
    IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry, TextEncoder,
};
use std::sync::Arc;

/// Prometheus metrics for the Anode server
pub struct Metrics {
    registry: Registry,

    // Request metrics
    pub requests_total: IntCounterVec,
    pub request_count: IntCounterVec,
    pub request_duration_seconds: HistogramVec,
    pub request_latency_seconds: HistogramVec,
    pub request_size_bytes: HistogramVec,
    pub response_size_bytes: HistogramVec,
    pub active_requests: IntGauge,

    // Object storage metrics
    pub objects_total: IntGaugeVec,
    pub storage_objects_total: IntGauge,
    pub storage_bytes_total: IntGaugeVec,
    pub operations_total: IntCounterVec,
    pub operation_errors_total: IntCounterVec,
    pub operation_duration_seconds: HistogramVec,
    pub chunks_total: IntGaugeVec,

    // Raft metrics
    pub raft_term: IntGauge,
    pub raft_is_leader: IntGauge,
    pub raft_commit_index: IntGauge,
    pub raft_last_applied: IntGauge,
    pub raft_proposals_total: IntCounter,
    pub raft_proposal_failures_total: IntCounter,
    pub raft_leader_changes_total: IntCounter,
    pub raft_snapshot_creating: IntGauge,
    pub raft_snapshot_last_index: IntGauge,

    // Cluster metrics
    pub cluster_nodes_total: IntGauge,
    pub cluster_healthy_nodes: IntGauge,
    pub cluster_replication_lag_seconds: GaugeVec,
    pub cluster_peer_rtt_seconds: GaugeVec,

    // Parquet metrics
    pub parquet_cache_hits_total: IntCounter,
    pub parquet_cache_misses_total: IntCounter,
    pub parquet_cache_size_bytes: IntGauge,
    pub parquet_cache_items: IntGauge,
    pub parquet_cache_evictions_total: IntCounter,
    pub parquet_queries_total: IntCounterVec,
    pub parquet_query_duration_seconds: HistogramVec,
    pub parquet_files_total: IntGauge,
    pub parquet_row_groups_scanned: IntCounterVec,

    // System metrics
    pub uptime_seconds: Gauge,
    pub memory_usage_bytes: Gauge,
    pub cpu_usage_percent: Gauge,
    pub disk_usage_bytes: GaugeVec,
    pub disk_available_bytes: GaugeVec,
    pub open_file_descriptors: IntGauge,

    // Network metrics
    pub network_bytes_sent_total: IntCounterVec,
    pub network_bytes_received_total: IntCounterVec,
    pub active_connections: IntGaugeVec,

    // Multipart upload metrics
    pub multipart_uploads_active: IntGauge,
    pub multipart_parts_total: IntCounterVec,
    pub multipart_upload_duration_seconds: HistogramVec,
}

impl Metrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        let registry = Registry::new();

        // Request metrics
        let requests_total = IntCounterVec::new(
            Opts::new("anode_requests_total", "Total number of requests").namespace("anode"),
            &["method", "endpoint", "status"],
        )
        .unwrap();

        let request_count = IntCounterVec::new(
            Opts::new(
                "anode_request_count",
                "Count of requests by method and status",
            )
            .namespace("anode"),
            &["method", "status"],
        )
        .unwrap();

        let request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "anode_request_duration_seconds",
                "Request duration in seconds",
            )
            .namespace("anode")
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
            &["method", "endpoint"],
        )
        .unwrap();

        let request_latency_seconds = HistogramVec::new(
            HistogramOpts::new(
                "anode_request_latency_seconds",
                "Request latency in seconds",
            )
            .namespace("anode")
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
            &["method"],
        )
        .unwrap();

        let request_size_bytes = HistogramVec::new(
            HistogramOpts::new("anode_request_size_bytes", "Request size in bytes")
                .namespace("anode")
                .buckets(prometheus::exponential_buckets(1024.0, 4.0, 10).unwrap()),
            &["method", "endpoint"],
        )
        .unwrap();

        let response_size_bytes = HistogramVec::new(
            HistogramOpts::new("anode_response_size_bytes", "Response size in bytes")
                .namespace("anode")
                .buckets(prometheus::exponential_buckets(1024.0, 4.0, 10).unwrap()),
            &["method", "endpoint"],
        )
        .unwrap();

        let active_requests = IntGauge::new(
            "anode_active_requests",
            "Number of currently active requests",
        )
        .unwrap();

        // Object storage metrics
        let objects_total = IntGaugeVec::new(
            Opts::new("anode_objects_total", "Total number of objects").namespace("anode"),
            &["bucket"],
        )
        .unwrap();

        let storage_objects_total = IntGauge::new(
            "anode_storage_objects_total",
            "Total number of objects in storage",
        )
        .unwrap();

        let storage_bytes_total = IntGaugeVec::new(
            Opts::new("anode_storage_bytes_total", "Total storage in bytes").namespace("anode"),
            &["bucket"],
        )
        .unwrap();

        let operations_total = IntCounterVec::new(
            Opts::new("anode_operations_total", "Total storage operations").namespace("anode"),
            &["operation"],
        )
        .unwrap();

        let operation_errors_total = IntCounterVec::new(
            Opts::new(
                "anode_operation_errors_total",
                "Total storage operation errors",
            )
            .namespace("anode"),
            &["operation", "error_type"],
        )
        .unwrap();

        let operation_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "anode_operation_duration_seconds",
                "Storage operation duration in seconds",
            )
            .namespace("anode")
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
            ]),
            &["operation"],
        )
        .unwrap();

        let chunks_total = IntGaugeVec::new(
            Opts::new("anode_chunks_total", "Total number of chunks").namespace("anode"),
            &["bucket"],
        )
        .unwrap();

        // Raft metrics
        let raft_term = IntGauge::new("anode_raft_term", "Current Raft term").unwrap();

        let raft_is_leader = IntGauge::new(
            "anode_raft_is_leader",
            "Whether this node is leader (1) or not (0)",
        )
        .unwrap();

        let raft_commit_index =
            IntGauge::new("anode_raft_commit_index", "Raft commit index").unwrap();

        let raft_proposals_total = IntCounter::new(
            "anode_raft_proposals_total",
            "Total Raft proposals submitted",
        )
        .unwrap();

        let raft_proposal_failures_total = IntCounter::new(
            "anode_raft_proposal_failures_total",
            "Total Raft proposal failures",
        )
        .unwrap();

        let raft_last_applied =
            IntGauge::new("anode_raft_last_applied", "Raft last applied index").unwrap();

        let raft_leader_changes_total = IntCounter::new(
            "anode_raft_leader_changes_total",
            "Total number of Raft leader changes",
        )
        .unwrap();

        let raft_snapshot_creating = IntGauge::new(
            "anode_raft_snapshot_creating",
            "Whether a Raft snapshot is being created (1) or not (0)",
        )
        .unwrap();

        let raft_snapshot_last_index = IntGauge::new(
            "anode_raft_snapshot_last_index",
            "Index of the last Raft snapshot",
        )
        .unwrap();

        // Cluster metrics
        let cluster_nodes_total = IntGauge::new(
            "anode_cluster_nodes_total",
            "Total number of nodes in cluster",
        )
        .unwrap();

        let cluster_healthy_nodes = IntGauge::new(
            "anode_cluster_healthy_nodes",
            "Number of healthy nodes in cluster",
        )
        .unwrap();

        let cluster_replication_lag_seconds = GaugeVec::new(
            Opts::new(
                "anode_cluster_replication_lag_seconds",
                "Replication lag in seconds per peer",
            )
            .namespace("anode"),
            &["peer_id"],
        )
        .unwrap();

        let cluster_peer_rtt_seconds = GaugeVec::new(
            Opts::new(
                "anode_cluster_peer_rtt_seconds",
                "Round-trip time to peers in seconds",
            )
            .namespace("anode"),
            &["peer_id"],
        )
        .unwrap();

        // Parquet metrics
        let parquet_cache_hits_total =
            IntCounter::new("anode_parquet_cache_hits_total", "Total Parquet cache hits").unwrap();

        let parquet_cache_misses_total = IntCounter::new(
            "anode_parquet_cache_misses_total",
            "Total Parquet cache misses",
        )
        .unwrap();

        let parquet_cache_size_bytes = IntGauge::new(
            "anode_parquet_cache_size_bytes",
            "Current Parquet cache size in bytes",
        )
        .unwrap();

        let parquet_queries_total = IntCounterVec::new(
            Opts::new("anode_parquet_queries_total", "Total Parquet queries").namespace("anode"),
            &["query_type"],
        )
        .unwrap();

        let parquet_cache_items = IntGauge::new(
            "anode_parquet_cache_items",
            "Number of items in Parquet cache",
        )
        .unwrap();

        let parquet_cache_evictions_total = IntCounter::new(
            "anode_parquet_cache_evictions_total",
            "Total Parquet cache evictions",
        )
        .unwrap();

        let parquet_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "anode_parquet_query_duration_seconds",
                "Parquet query duration in seconds",
            )
            .namespace("anode")
            .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["query_type"],
        )
        .unwrap();

        let parquet_files_total =
            IntGauge::new("anode_parquet_files_total", "Total number of Parquet files").unwrap();

        let parquet_row_groups_scanned = IntCounterVec::new(
            Opts::new(
                "anode_parquet_row_groups_scanned",
                "Total Parquet row groups scanned",
            )
            .namespace("anode"),
            &["file"],
        )
        .unwrap();

        // System metrics
        let uptime_seconds =
            Gauge::new("anode_uptime_seconds", "Server uptime in seconds").unwrap();

        let memory_usage_bytes =
            Gauge::new("anode_memory_usage_bytes", "Current memory usage in bytes").unwrap();

        let cpu_usage_percent =
            Gauge::new("anode_cpu_usage_percent", "CPU usage percentage").unwrap();

        let disk_usage_bytes = GaugeVec::new(
            Opts::new("anode_disk_usage_bytes", "Disk usage in bytes").namespace("anode"),
            &["path"],
        )
        .unwrap();

        let disk_available_bytes = GaugeVec::new(
            Opts::new(
                "anode_disk_available_bytes",
                "Available disk space in bytes",
            )
            .namespace("anode"),
            &["path"],
        )
        .unwrap();

        let open_file_descriptors = IntGauge::new(
            "anode_open_file_descriptors",
            "Number of open file descriptors",
        )
        .unwrap();

        // Network metrics
        let network_bytes_sent_total = IntCounterVec::new(
            Opts::new(
                "anode_network_bytes_sent_total",
                "Total bytes sent over network",
            )
            .namespace("anode"),
            &["protocol"],
        )
        .unwrap();

        let network_bytes_received_total = IntCounterVec::new(
            Opts::new(
                "anode_network_bytes_received_total",
                "Total bytes received over network",
            )
            .namespace("anode"),
            &["protocol"],
        )
        .unwrap();

        let active_connections = IntGaugeVec::new(
            Opts::new("anode_active_connections", "Number of active connections")
                .namespace("anode"),
            &["protocol"],
        )
        .unwrap();

        // Multipart upload metrics
        let multipart_uploads_active = IntGauge::new(
            "anode_multipart_uploads_active",
            "Number of active multipart uploads",
        )
        .unwrap();

        let multipart_parts_total = IntCounterVec::new(
            Opts::new(
                "anode_multipart_parts_total",
                "Total multipart upload parts",
            )
            .namespace("anode"),
            &["status"],
        )
        .unwrap();

        let multipart_upload_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "anode_multipart_upload_duration_seconds",
                "Multipart upload duration in seconds",
            )
            .namespace("anode")
            .buckets(vec![
                1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0,
            ]),
            &["status"],
        )
        .unwrap();

        // Register all metrics
        registry.register(Box::new(requests_total.clone())).unwrap();
        registry.register(Box::new(request_count.clone())).unwrap();
        registry
            .register(Box::new(request_duration_seconds.clone()))
            .unwrap();
        registry
            .register(Box::new(request_latency_seconds.clone()))
            .unwrap();
        registry
            .register(Box::new(request_size_bytes.clone()))
            .unwrap();
        registry
            .register(Box::new(response_size_bytes.clone()))
            .unwrap();
        registry
            .register(Box::new(active_requests.clone()))
            .unwrap();
        registry.register(Box::new(objects_total.clone())).unwrap();
        registry
            .register(Box::new(storage_objects_total.clone()))
            .unwrap();
        registry
            .register(Box::new(storage_bytes_total.clone()))
            .unwrap();
        registry
            .register(Box::new(operations_total.clone()))
            .unwrap();
        registry
            .register(Box::new(operation_errors_total.clone()))
            .unwrap();
        registry
            .register(Box::new(operation_duration_seconds.clone()))
            .unwrap();
        registry.register(Box::new(chunks_total.clone())).unwrap();
        registry.register(Box::new(raft_term.clone())).unwrap();
        registry.register(Box::new(raft_is_leader.clone())).unwrap();
        registry
            .register(Box::new(raft_commit_index.clone()))
            .unwrap();
        registry
            .register(Box::new(raft_last_applied.clone()))
            .unwrap();
        registry
            .register(Box::new(raft_proposals_total.clone()))
            .unwrap();
        registry
            .register(Box::new(raft_proposal_failures_total.clone()))
            .unwrap();
        registry
            .register(Box::new(raft_leader_changes_total.clone()))
            .unwrap();
        registry
            .register(Box::new(raft_snapshot_creating.clone()))
            .unwrap();
        registry
            .register(Box::new(raft_snapshot_last_index.clone()))
            .unwrap();
        registry
            .register(Box::new(cluster_nodes_total.clone()))
            .unwrap();
        registry
            .register(Box::new(cluster_healthy_nodes.clone()))
            .unwrap();
        registry
            .register(Box::new(cluster_replication_lag_seconds.clone()))
            .unwrap();
        registry
            .register(Box::new(cluster_peer_rtt_seconds.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_cache_hits_total.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_cache_misses_total.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_cache_size_bytes.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_cache_items.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_cache_evictions_total.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_queries_total.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_query_duration_seconds.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_files_total.clone()))
            .unwrap();
        registry
            .register(Box::new(parquet_row_groups_scanned.clone()))
            .unwrap();
        registry.register(Box::new(uptime_seconds.clone())).unwrap();
        registry
            .register(Box::new(memory_usage_bytes.clone()))
            .unwrap();
        registry
            .register(Box::new(cpu_usage_percent.clone()))
            .unwrap();
        registry
            .register(Box::new(disk_usage_bytes.clone()))
            .unwrap();
        registry
            .register(Box::new(disk_available_bytes.clone()))
            .unwrap();
        registry
            .register(Box::new(open_file_descriptors.clone()))
            .unwrap();
        registry
            .register(Box::new(network_bytes_sent_total.clone()))
            .unwrap();
        registry
            .register(Box::new(network_bytes_received_total.clone()))
            .unwrap();
        registry
            .register(Box::new(active_connections.clone()))
            .unwrap();
        registry
            .register(Box::new(multipart_uploads_active.clone()))
            .unwrap();
        registry
            .register(Box::new(multipart_parts_total.clone()))
            .unwrap();
        registry
            .register(Box::new(multipart_upload_duration_seconds.clone()))
            .unwrap();

        Self {
            registry,
            requests_total,
            request_count,
            request_duration_seconds,
            request_latency_seconds,
            request_size_bytes,
            response_size_bytes,
            active_requests,
            objects_total,
            storage_objects_total,
            storage_bytes_total,
            operations_total,
            operation_errors_total,
            operation_duration_seconds,
            chunks_total,
            raft_term,
            raft_is_leader,
            raft_commit_index,
            raft_last_applied,
            raft_proposals_total,
            raft_proposal_failures_total,
            raft_leader_changes_total,
            raft_snapshot_creating,
            raft_snapshot_last_index,
            cluster_nodes_total,
            cluster_healthy_nodes,
            cluster_replication_lag_seconds,
            cluster_peer_rtt_seconds,
            parquet_cache_hits_total,
            parquet_cache_misses_total,
            parquet_cache_size_bytes,
            parquet_cache_items,
            parquet_cache_evictions_total,
            parquet_queries_total,
            parquet_query_duration_seconds,
            parquet_files_total,
            parquet_row_groups_scanned,
            uptime_seconds,
            memory_usage_bytes,
            cpu_usage_percent,
            disk_usage_bytes,
            disk_available_bytes,
            open_file_descriptors,
            network_bytes_sent_total,
            network_bytes_received_total,
            active_connections,
            multipart_uploads_active,
            multipart_parts_total,
            multipart_upload_duration_seconds,
        }
    }

    /// Export metrics in Prometheus text format
    pub fn export(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();

        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        String::from_utf8(buffer).unwrap()
    }

    /// Get the registry
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        // Increment a counter to ensure it shows up in export
        metrics
            .requests_total
            .with_label_values(&["GET", "/test", "200"])
            .inc();
        assert!(metrics.export().contains("anode_requests_total"));
    }

    #[test]
    fn test_metrics_increment() {
        let metrics = Metrics::new();

        metrics
            .requests_total
            .with_label_values(&["GET", "/health", "200"])
            .inc();

        let output = metrics.export();
        assert!(output.contains("anode_requests_total"));
    }
}
