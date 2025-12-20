//! Network chaos utilities
//!
//! Provides network manipulation capabilities for chaos testing including
//! partitions, latency injection, packet loss, and bandwidth limiting.

use crate::{ChaosError, Result};
use anode_test_harness::NodeId;
use std::process::Command;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Network condition to apply
#[derive(Debug, Clone)]
pub enum NetworkCondition {
    /// Add latency (delay in ms, jitter in ms)
    Latency { delay_ms: u64, jitter_ms: u64 },
    /// Add packet loss (percentage 0-100)
    PacketLoss { percent: f64 },
    /// Limit bandwidth (bytes per second)
    BandwidthLimit { bytes_per_sec: u64 },
    /// Duplicate packets (percentage)
    PacketDuplication { percent: f64 },
    /// Corrupt packets (percentage)
    PacketCorruption { percent: f64 },
}

/// Network chaos controller
pub struct NetworkChaos {
    /// Network interface to manipulate (e.g., "eth0", "lo")
    interface: String,
    /// Whether tc commands are available
    tc_available: bool,
}

impl NetworkChaos {
    /// Create a new network chaos controller
    pub fn new(interface: String) -> Self {
        let tc_available = Self::check_tc_available();

        if !tc_available {
            warn!("tc (traffic control) not available - network chaos features will be limited");
        }

        Self {
            interface,
            tc_available,
        }
    }

    /// Create with default interface
    pub fn default() -> Self {
        Self::new("eth0".to_string())
    }

    /// Partition a node from the network
    pub async fn partition(&self, node: NodeId, target_port: u16) -> Result<()> {
        if !self.tc_available {
            warn!("Cannot partition - tc not available");
            return Err(ChaosError::NetworkChaos(
                "tc (traffic control) not available".to_string(),
            ));
        }

        info!("Partitioning node {} on port {}", node, target_port);

        // Use iptables to block traffic to/from the node's port
        self.run_iptables_command(&[
            "-A", "INPUT",
            "-p", "tcp",
            "--dport", &target_port.to_string(),
            "-j", "DROP",
        ])?;

        self.run_iptables_command(&[
            "-A", "OUTPUT",
            "-p", "tcp",
            "--sport", &target_port.to_string(),
            "-j", "DROP",
        ])?;

        Ok(())
    }

    /// Remove partition for a node
    pub async fn heal_partition(&self, node: NodeId, target_port: u16) -> Result<()> {
        info!("Healing partition for node {} on port {}", node, target_port);

        // Remove iptables rules
        self.run_iptables_command(&[
            "-D", "INPUT",
            "-p", "tcp",
            "--dport", &target_port.to_string(),
            "-j", "DROP",
        ]).ok(); // Ignore errors if rule doesn't exist

        self.run_iptables_command(&[
            "-D", "OUTPUT",
            "-p", "tcp",
            "--sport", &target_port.to_string(),
            "-j", "DROP",
        ]).ok();

        Ok(())
    }

    /// Add a network condition
    pub async fn add_condition(&self, condition: NetworkCondition) -> Result<()> {
        if !self.tc_available {
            warn!("Cannot add network condition - tc not available");
            return Err(ChaosError::NetworkChaos(
                "tc (traffic control) not available".to_string(),
            ));
        }

        let mut args = vec![
            "qdisc", "add", "dev", &self.interface, "root", "netem",
        ];

        let delay_str;
        let jitter_str;
        let loss_str;
        let rate_str;
        let duplicate_str;
        let corrupt_str;

        match condition {
            NetworkCondition::Latency { delay_ms, jitter_ms } => {
                delay_str = format!("{}ms", delay_ms);
                args.extend(&["delay", &delay_str]);

                if jitter_ms > 0 {
                    jitter_str = format!("{}ms", jitter_ms);
                    args.push(&jitter_str);
                }
            }
            NetworkCondition::PacketLoss { percent } => {
                loss_str = format!("{}%", percent);
                args.extend(&["loss", &loss_str]);
            }
            NetworkCondition::BandwidthLimit { bytes_per_sec } => {
                // For bandwidth limiting, we need to use tbf (token bucket filter)
                // This is more complex and would need a separate implementation
                warn!("Bandwidth limiting requires tbf qdisc - not implemented in this basic version");
                return Err(ChaosError::NetworkChaos(
                    "Bandwidth limiting not fully implemented".to_string(),
                ));
            }
            NetworkCondition::PacketDuplication { percent } => {
                duplicate_str = format!("{}%", percent);
                args.extend(&["duplicate", &duplicate_str]);
            }
            NetworkCondition::PacketCorruption { percent } => {
                corrupt_str = format!("{}%", percent);
                args.extend(&["corrupt", &corrupt_str]);
            }
        }

        self.run_tc_command(&args)?;

        Ok(())
    }

    /// Remove all network conditions
    pub async fn clear_conditions(&self) -> Result<()> {
        if !self.tc_available {
            return Ok(());
        }

        info!("Clearing all network conditions on {}", self.interface);

        self.run_tc_command(&[
            "qdisc", "del", "dev", &self.interface, "root",
        ]).ok(); // Ignore errors if no qdisc exists

        Ok(())
    }

    /// Add latency to the network
    pub async fn add_latency(&self, delay: Duration, jitter: Duration) -> Result<()> {
        self.add_condition(NetworkCondition::Latency {
            delay_ms: delay.as_millis() as u64,
            jitter_ms: jitter.as_millis() as u64,
        }).await
    }

    /// Add packet loss
    pub async fn add_packet_loss(&self, percent: f64) -> Result<()> {
        self.add_condition(NetworkCondition::PacketLoss { percent }).await
    }

    /// Add packet duplication
    pub async fn add_packet_duplication(&self, percent: f64) -> Result<()> {
        self.add_condition(NetworkCondition::PacketDuplication { percent }).await
    }

    /// Add packet corruption
    pub async fn add_packet_corruption(&self, percent: f64) -> Result<()> {
        self.add_condition(NetworkCondition::PacketCorruption { percent }).await
    }

    /// Heal all network issues
    pub async fn heal(&self) -> Result<()> {
        self.clear_conditions().await
    }

    /// Check if tc is available on the system
    fn check_tc_available() -> bool {
        Command::new("tc")
            .arg("-Version")
            .output()
            .is_ok()
    }

    /// Run a tc command
    fn run_tc_command(&self, args: &[&str]) -> Result<()> {
        debug!("Running tc command: tc {}", args.join(" "));

        let output = Command::new("tc")
            .args(args)
            .output()
            .map_err(|e| ChaosError::NetworkChaos(format!("Failed to run tc: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChaosError::NetworkChaos(format!(
                "tc command failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Run an iptables command
    fn run_iptables_command(&self, args: &[&str]) -> Result<()> {
        debug!("Running iptables command: iptables {}", args.join(" "));

        let output = Command::new("iptables")
            .args(args)
            .output()
            .map_err(|e| ChaosError::NetworkChaos(format!("Failed to run iptables: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChaosError::NetworkChaos(format!(
                "iptables command failed: {}",
                stderr
            )));
        }

        Ok(())
    }
}

impl Drop for NetworkChaos {
    fn drop(&mut self) {
        // Clean up network conditions on drop
        if self.tc_available {
            let _ = self.run_tc_command(&[
                "qdisc", "del", "dev", &self.interface, "root",
            ]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_chaos_creation() {
        let chaos = NetworkChaos::new("lo".to_string());
        assert_eq!(chaos.interface, "lo");
    }

    #[test]
    fn test_network_condition_variants() {
        let latency = NetworkCondition::Latency {
            delay_ms: 100,
            jitter_ms: 10,
        };
        assert!(matches!(latency, NetworkCondition::Latency { .. }));

        let loss = NetworkCondition::PacketLoss { percent: 10.0 };
        assert!(matches!(loss, NetworkCondition::PacketLoss { .. }));
    }
}
