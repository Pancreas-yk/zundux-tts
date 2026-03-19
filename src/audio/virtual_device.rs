use anyhow::{Context, Result};
use std::process::Command;

pub struct VirtualDevice {
    pub(crate) sink_name: String,
    sink_module_id: Option<u32>,
    source_module_id: Option<u32>,
}

impl VirtualDevice {
    pub fn new(sink_name: &str) -> Self {
        Self {
            sink_name: sink_name.to_string(),
            sink_module_id: None,
            source_module_id: None,
        }
    }

    /// The virtual source name that applications should use as a microphone input.
    pub fn source_name(&self) -> String {
        format!("{}_mic", self.sink_name)
    }

    pub fn monitor_source(&self) -> String {
        format!("{}.monitor", self.sink_name)
    }

    fn sink_exists(&self) -> Result<bool> {
        let output = Command::new("pactl")
            .args(["list", "short", "sinks"])
            .output()
            .context("Failed to run pactl")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().any(|line| line.contains(&self.sink_name)))
    }

    fn virtual_source_exists(&self) -> Result<bool> {
        let source_name = self.source_name();
        let output = Command::new("pactl")
            .args(["list", "short", "sources"])
            .output()
            .context("Failed to run pactl")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().any(|line| line.contains(&source_name)))
    }

    pub fn exists(&self) -> Result<bool> {
        self.sink_exists()
    }

    pub fn create(&mut self) -> Result<()> {
        // Step 1: Create the null sink (audio target for TTS playback)
        if !self.sink_exists()? {
            let output = Command::new("pactl")
                .args([
                    "load-module",
                    "module-null-sink",
                    &format!("sink_name={}", self.sink_name),
                    &format!(
                        "sink_properties=device.description=\"Zundamon_VRC_Virtual_Mic\""
                    ),
                ])
                .output()
                .context("Failed to create virtual sink")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("pactl load-module (null-sink) failed: {}", stderr);
            }

            let id_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            self.sink_module_id = id_str.parse().ok();
            tracing::info!(
                "Created virtual sink {} (module {})",
                self.sink_name,
                id_str
            );
        } else {
            tracing::info!("Virtual sink {} already exists", self.sink_name);
        }

        // Step 2: Create a virtual source that wraps the sink's monitor.
        // This exposes a proper input device (microphone) that apps like VRChat can detect.
        if !self.virtual_source_exists()? {
            let source_name = self.source_name();
            let output = Command::new("pactl")
                .args([
                    "load-module",
                    "module-virtual-source",
                    &format!("source_name={}", source_name),
                    &format!("master={}.monitor", self.sink_name),
                    &format!(
                        "source_properties=device.description=\"Zundamon_VRC_Microphone\""
                    ),
                ])
                .output()
                .context("Failed to create virtual source")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Non-fatal: the monitor source still works on systems without module-virtual-source
                tracing::warn!(
                    "module-virtual-source failed ({}). The monitor source ({}.monitor) \
                     can still be used manually, but some apps may not detect it as a microphone.",
                    stderr.trim(),
                    self.sink_name
                );
            } else {
                let id_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                self.source_module_id = id_str.parse().ok();
                tracing::info!(
                    "Created virtual source {} (module {})",
                    source_name,
                    id_str
                );
            }
        } else {
            tracing::info!("Virtual source {} already exists", self.source_name());
        }

        Ok(())
    }

    pub fn destroy(&mut self) -> Result<()> {
        // Destroy virtual source first, then the sink
        if let Some(id) = self.source_module_id.take() {
            let output = Command::new("pactl")
                .args(["unload-module", &id.to_string()])
                .output()
                .context("Failed to unload virtual source module")?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("Failed to unload virtual source module {}: {}", id, stderr);
            } else {
                tracing::info!("Destroyed virtual source (module {})", id);
            }
        }
        if let Some(id) = self.sink_module_id.take() {
            let output = Command::new("pactl")
                .args(["unload-module", &id.to_string()])
                .output()
                .context("Failed to unload sink module")?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("Failed to unload sink module {}: {}", id, stderr);
            } else {
                tracing::info!("Destroyed virtual sink (module {})", id);
            }
        }
        Ok(())
    }
}

impl Drop for VirtualDevice {
    fn drop(&mut self) {
        if let Err(e) = self.destroy() {
            tracing::error!("Error destroying virtual device: {}", e);
        }
    }
}
