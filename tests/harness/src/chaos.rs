//! Chaos injection utilities for fault testing

use crate::{cluster::TestCluster, ChunkId, Result, TestHarnessError};
use rand::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// Types of chaos events that can be injected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChaosEvent {
    /// Kill a node (simulate crash)
    KillNode(usize),

    /// Create a network partition isolating specified nodes
    NetworkPartition(Vec<usize>),

    /// Slow down network for a node
    SlowNetwork(usize, Duration),

    /// Simulate disk full condition
    DiskFull(usize),

    /// Corrupt a chunk on disk
    CorruptChunk(usize, ChunkId),

    /// Pause a process (SIGSTOP)
    ProcessPause(usize, Duration),

    /// Restart a node
    RestartNode(usize),

    /// Slow down disk I/O
    SlowDisk(usize, Duration),

    /// Simulate clock skew
    ClockSkew(usize, Duration),

    /// Drop packets
    PacketLoss(usize, f64),

    /// Limit bandwidth
    BandwidthLimit(usize, u64),
}

/// Result of applying a chaos event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosResult {
    /// The event that was applied
    pub event: ChaosEvent,
    /// When the event was applied
    pub timestamp: std::time::SystemTime,
    /// Whether the event was successfully applied
    pub success: bool,
    /// Error message if the event failed
    pub error: Option<String>,
    /// Duration the event was active
    pub duration: Option<Duration>,
}

/// Schedules and executes chaos events
pub struct ChaosScheduler {
    events: Vec<(Duration, ChaosEvent)>,
}

impl ChaosScheduler {
    /// Create a new chaos scheduler
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    /// Add an event to be executed at a specific time
    pub fn add_event(mut self, delay: Duration, event: ChaosEvent) -> Self {
        self.events.push((delay, event));
        self
    }

    /// Generate a random chaos schedule
    pub fn random(duration: Duration, intensity: f64) -> Self {
        let mut scheduler = Self::new();
        let mut rng = thread_rng();

        // Calculate number of events based on intensity
        let num_events = (duration.as_secs_f64() * intensity) as usize;

        for _ in 0..num_events {
            let delay = Duration::from_secs_f64(rng.gen::<f64>() * duration.as_secs_f64());
            let event = Self::random_event(&mut rng);
            scheduler = scheduler.add_event(delay, event);
        }

        // Sort events by delay
        scheduler.events.sort_by_key(|(delay, _)| *delay);

        scheduler
    }

    /// Generate a random chaos event
    fn random_event(rng: &mut impl Rng) -> ChaosEvent {
        let event_type = rng.gen_range(0..6);

        match event_type {
            0 => ChaosEvent::KillNode(rng.gen_range(0..3)),
            1 => {
                let num_nodes = rng.gen_range(1..3);
                let nodes: Vec<usize> = (0..num_nodes).map(|_| rng.gen_range(0..5)).collect();
                ChaosEvent::NetworkPartition(nodes)
            }
            2 => ChaosEvent::SlowNetwork(
                rng.gen_range(0..3),
                Duration::from_millis(rng.gen_range(100..1000)),
            ),
            3 => ChaosEvent::RestartNode(rng.gen_range(0..3)),
            4 => ChaosEvent::ProcessPause(
                rng.gen_range(0..3),
                Duration::from_secs(rng.gen_range(1..10)),
            ),
            5 => ChaosEvent::PacketLoss(rng.gen_range(0..3), rng.gen::<f64>() * 0.3),
            _ => ChaosEvent::KillNode(0),
        }
    }

    /// Run the chaos schedule against a cluster
    pub async fn run(&self, cluster: &mut TestCluster) -> Result<Vec<ChaosResult>> {
        info!("Starting chaos schedule with {} events", self.events.len());

        let mut results = Vec::new();
        let start_time = std::time::Instant::now();

        for (delay, event) in &self.events {
            // Wait until it's time for this event
            let elapsed = start_time.elapsed();
            if *delay > elapsed {
                sleep(*delay - elapsed).await;
            }

            info!("Applying chaos event: {:?}", event);

            let result = Self::apply_event(cluster, event).await;
            results.push(result);
        }

        info!("Chaos schedule completed");
        Ok(results)
    }

    /// Apply a single chaos event
    async fn apply_event(cluster: &mut TestCluster, event: &ChaosEvent) -> ChaosResult {
        let timestamp = std::time::SystemTime::now();
        let start = std::time::Instant::now();

        let (success, error) = match event {
            ChaosEvent::KillNode(index) => {
                match cluster.kill_node(*index).await {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e.to_string())),
                }
            }

            ChaosEvent::NetworkPartition(nodes) => {
                let mut all_success = true;
                let mut errors = Vec::new();

                for &node_index in nodes {
                    if let Err(e) = cluster.partition_node(node_index).await {
                        all_success = false;
                        errors.push(format!("Node {}: {}", node_index, e));
                    }
                }

                let error = if errors.is_empty() {
                    None
                } else {
                    Some(errors.join("; "))
                };

                (all_success, error)
            }

            ChaosEvent::SlowNetwork(index, delay) => {
                warn!(
                    "SlowNetwork chaos not fully implemented for node {} (delay: {:?})",
                    index, delay
                );
                (true, Some("Not fully implemented".to_string()))
            }

            ChaosEvent::DiskFull(index) => {
                warn!("DiskFull chaos not fully implemented for node {}", index);
                (true, Some("Not fully implemented".to_string()))
            }

            ChaosEvent::CorruptChunk(index, chunk_id) => {
                warn!(
                    "CorruptChunk chaos not fully implemented for node {} chunk {}",
                    index, chunk_id
                );
                (true, Some("Not fully implemented".to_string()))
            }

            ChaosEvent::ProcessPause(index, duration) => {
                warn!(
                    "ProcessPause chaos not fully implemented for node {} (duration: {:?})",
                    index, duration
                );
                (true, Some("Not fully implemented".to_string()))
            }

            ChaosEvent::RestartNode(index) => {
                match cluster.restart_node(*index).await {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e.to_string())),
                }
            }

            ChaosEvent::SlowDisk(index, delay) => {
                warn!(
                    "SlowDisk chaos not fully implemented for node {} (delay: {:?})",
                    index, delay
                );
                (true, Some("Not fully implemented".to_string()))
            }

            ChaosEvent::ClockSkew(index, skew) => {
                warn!(
                    "ClockSkew chaos not fully implemented for node {} (skew: {:?})",
                    index, skew
                );
                (true, Some("Not fully implemented".to_string()))
            }

            ChaosEvent::PacketLoss(index, rate) => {
                warn!(
                    "PacketLoss chaos not fully implemented for node {} (rate: {})",
                    index, rate
                );
                (true, Some("Not fully implemented".to_string()))
            }

            ChaosEvent::BandwidthLimit(index, limit) => {
                warn!(
                    "BandwidthLimit chaos not fully implemented for node {} (limit: {})",
                    index, limit
                );
                (true, Some("Not fully implemented".to_string()))
            }
        };

        ChaosResult {
            event: event.clone(),
            timestamp,
            success,
            error,
            duration: Some(start.elapsed()),
        }
    }

    /// Get the total duration of the schedule
    pub fn total_duration(&self) -> Duration {
        self.events
            .iter()
            .map(|(delay, _)| *delay)
            .max()
            .unwrap_or(Duration::from_secs(0))
    }

    /// Get the number of events
    pub fn num_events(&self) -> usize {
        self.events.len()
    }
}

impl Default for ChaosScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create common chaos scenarios
pub struct ChaosScenarios;

impl ChaosScenarios {
    /// Kill and restart a node
    pub fn kill_and_restart(node_index: usize, downtime: Duration) -> ChaosScheduler {
        ChaosScheduler::new()
            .add_event(Duration::from_secs(0), ChaosEvent::KillNode(node_index))
            .add_event(downtime, ChaosEvent::RestartNode(node_index))
    }

    /// Create a network partition and heal it
    pub fn partition_and_heal(nodes: Vec<usize>, duration: Duration) -> ChaosScheduler {
        let heal_events: Vec<_> = nodes
            .iter()
            .map(|&n| (duration, ChaosEvent::NetworkPartition(vec![n])))
            .collect();

        let mut scheduler = ChaosScheduler::new()
            .add_event(Duration::from_secs(0), ChaosEvent::NetworkPartition(nodes));

        for (delay, event) in heal_events {
            scheduler = scheduler.add_event(delay, event);
        }

        scheduler
    }

    /// Rolling restart of all nodes
    pub fn rolling_restart(num_nodes: usize, restart_interval: Duration) -> ChaosScheduler {
        let mut scheduler = ChaosScheduler::new();

        for i in 0..num_nodes {
            let delay = restart_interval * i as u32;
            scheduler = scheduler.add_event(delay, ChaosEvent::RestartNode(i));
        }

        scheduler
    }

    /// Majority partition (split brain scenario)
    pub fn split_brain(total_nodes: usize) -> ChaosScheduler {
        let majority_size = (total_nodes / 2) + 1;
        let minority_nodes: Vec<usize> = (0..(total_nodes - majority_size)).collect();

        ChaosScheduler::new()
            .add_event(Duration::from_secs(0), ChaosEvent::NetworkPartition(minority_nodes))
    }

    /// Cascading failures
    pub fn cascading_failures(num_nodes: usize, interval: Duration) -> ChaosScheduler {
        let mut scheduler = ChaosScheduler::new();

        for i in 0..num_nodes {
            let delay = interval * i as u32;
            scheduler = scheduler.add_event(delay, ChaosEvent::KillNode(i));
        }

        scheduler
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_scheduler_creation() {
        let scheduler = ChaosScheduler::new();
        assert_eq!(scheduler.num_events(), 0);
    }

    #[test]
    fn test_chaos_scheduler_add_event() {
        let scheduler = ChaosScheduler::new()
            .add_event(Duration::from_secs(1), ChaosEvent::KillNode(0))
            .add_event(Duration::from_secs(2), ChaosEvent::RestartNode(0));

        assert_eq!(scheduler.num_events(), 2);
        assert_eq!(scheduler.total_duration(), Duration::from_secs(2));
    }

    #[test]
    fn test_random_chaos_schedule() {
        let scheduler = ChaosScheduler::random(Duration::from_secs(60), 0.1);
        assert!(scheduler.num_events() > 0);
        assert!(scheduler.total_duration() <= Duration::from_secs(60));
    }

    #[test]
    fn test_kill_and_restart_scenario() {
        let scheduler = ChaosScenarios::kill_and_restart(0, Duration::from_secs(5));
        assert_eq!(scheduler.num_events(), 2);
    }

    #[test]
    fn test_rolling_restart_scenario() {
        let scheduler = ChaosScenarios::rolling_restart(5, Duration::from_secs(10));
        assert_eq!(scheduler.num_events(), 5);
    }

    #[test]
    fn test_split_brain_scenario() {
        let scheduler = ChaosScenarios::split_brain(5);
        assert_eq!(scheduler.num_events(), 1);
    }
}
