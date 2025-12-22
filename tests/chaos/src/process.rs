//! Process chaos utilities
//!
//! Provides process manipulation capabilities for chaos testing.

use crate::{ChaosError, Result};
use std::process::Command;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Process chaos controller
pub struct ProcessChaos;

impl ProcessChaos {
    /// Create a new process chaos controller
    pub fn new() -> Self {
        Self
    }

    /// Pause a process (send SIGSTOP)
    pub async fn pause_process(&self, pid: u32, duration: Duration) -> Result<()> {
        info!("Pausing process {} for {:?}", pid, duration);

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(pid as i32);

            // Send SIGSTOP
            kill(pid, Signal::SIGSTOP)
                .map_err(|e| ChaosError::ProcessChaos(format!("Failed to pause process: {}", e)))?;

            debug!("Process {} paused", pid);

            // Wait for the specified duration
            tokio::time::sleep(duration).await;

            // Send SIGCONT to resume
            kill(pid, Signal::SIGCONT).map_err(|e| {
                ChaosError::ProcessChaos(format!("Failed to resume process: {}", e))
            })?;

            info!("Process {} resumed", pid);
        }

        #[cfg(not(unix))]
        {
            warn!("Process pausing not supported on non-Unix systems");
            return Err(ChaosError::ProcessChaos(
                "Process pausing not supported on this platform".to_string(),
            ));
        }

        Ok(())
    }

    /// Kill a process (send SIGKILL)
    pub async fn kill_process(&self, pid: u32) -> Result<()> {
        info!("Killing process {}", pid);

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(pid as i32);

            kill(pid, Signal::SIGKILL)
                .map_err(|e| ChaosError::ProcessChaos(format!("Failed to kill process: {}", e)))?;

            debug!("Process {} killed", pid);
        }

        #[cfg(not(unix))]
        {
            // Use platform-specific kill command
            let output = Command::new("taskkill")
                .args(&["/F", "/PID", &pid.to_string()])
                .output()
                .map_err(|e| ChaosError::ProcessChaos(format!("Failed to kill process: {}", e)))?;

            if !output.status.success() {
                return Err(ChaosError::ProcessChaos("Kill command failed".to_string()));
            }
        }

        Ok(())
    }

    /// Terminate a process gracefully (send SIGTERM)
    pub async fn terminate_process(&self, pid: u32) -> Result<()> {
        info!("Terminating process {}", pid);

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(pid as i32);

            kill(pid, Signal::SIGTERM).map_err(|e| {
                ChaosError::ProcessChaos(format!("Failed to terminate process: {}", e))
            })?;

            debug!("Process {} terminated", pid);
        }

        #[cfg(not(unix))]
        {
            warn!("Graceful termination not supported on non-Unix systems, using kill");
            self.kill_process(pid).await?;
        }

        Ok(())
    }

    /// Limit CPU usage of a process using cpulimit
    pub async fn limit_cpu(&self, pid: u32, limit_percent: u32) -> Result<()> {
        info!("Limiting CPU of process {} to {}%", pid, limit_percent);

        // This would require cpulimit to be installed
        // Example: cpulimit -p <pid> -l <limit>

        let output = Command::new("cpulimit")
            .args([
                "-p",
                &pid.to_string(),
                "-l",
                &limit_percent.to_string(),
                "-b", // Run in background
            ])
            .output()
            .map_err(|e| {
                ChaosError::ProcessChaos(format!(
                    "Failed to limit CPU (is cpulimit installed?): {}",
                    e
                ))
            })?;

        if !output.status.success() {
            return Err(ChaosError::ProcessChaos(
                "cpulimit command failed".to_string(),
            ));
        }

        Ok(())
    }

    /// Set CPU affinity for a process
    pub async fn set_cpu_affinity(&self, pid: u32, cpus: Vec<usize>) -> Result<()> {
        info!("Setting CPU affinity of process {} to {:?}", pid, cpus);

        #[cfg(target_os = "linux")]
        {
            // Use taskset on Linux
            let cpu_list = cpus
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let output = Command::new("taskset")
                .args(&["-cp", &cpu_list, &pid.to_string()])
                .output()
                .map_err(|e| {
                    ChaosError::ProcessChaos(format!("Failed to set CPU affinity: {}", e))
                })?;

            if !output.status.success() {
                return Err(ChaosError::ProcessChaos(
                    "taskset command failed".to_string(),
                ));
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            warn!("CPU affinity setting not implemented for this platform");
            Err(ChaosError::ProcessChaos(
                "CPU affinity not supported on this platform".to_string(),
            ))
        }
    }

    /// Inject memory pressure by allocating memory
    pub async fn inject_memory_pressure(&self, megabytes: usize, duration: Duration) -> Result<()> {
        info!(
            "Injecting {} MB of memory pressure for {:?}",
            megabytes, duration
        );

        // This would spawn a process that allocates memory
        // For now, just a placeholder
        warn!("Memory pressure injection not fully implemented");

        tokio::time::sleep(duration).await;

        Ok(())
    }
}

impl Default for ProcessChaos {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_chaos_creation() {
        let _chaos = ProcessChaos::new();
    }
}
