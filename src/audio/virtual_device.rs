use anyhow::{Context, Result};
use std::process::Command;

pub struct VirtualDevice {
    pub(crate) sink_name: String,
    sink_module_id: Option<u32>,
    source_module_id: Option<u32>,
    loopback_module_id: Option<u32>,
}

impl VirtualDevice {
    pub fn new(sink_name: &str) -> Self {
        Self {
            sink_name: sink_name.to_string(),
            sink_module_id: None,
            source_module_id: None,
            loopback_module_id: None,
        }
    }

    /// The virtual source name that applications should use as a microphone input.
    pub fn source_name(&self) -> String {
        format!("{}_mic", self.sink_name)
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
                        "sink_properties=device.description=\"{}_Virtual_Mic\"",
                        self.sink_name
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

        // Ensure sink is unmuted and at full volume (PipeWire may default to muted)
        let _ = Command::new("pactl")
            .args(["set-sink-mute", &self.sink_name, "0"])
            .output();
        let _ = Command::new("pactl")
            .args(["set-sink-volume", &self.sink_name, "100%"])
            .output();

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
                        "source_properties=device.description=\"{}_Microphone\"",
                        self.sink_name
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
                tracing::info!("Created virtual source {} (module {})", source_name, id_str);
            }
        } else {
            tracing::info!("Virtual source {} already exists", self.source_name());
        }

        Ok(())
    }

    /// List available PulseAudio input sources (excluding monitors).
    pub fn list_sources() -> Result<Vec<(String, String)>> {
        let output = Command::new("pactl")
            .args(["list", "sources"])
            .output()
            .context("Failed to run pactl list sources")?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut sources = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_desc: Option<String> = None;

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Name: ") {
                // Flush previous source
                if let Some(name) = current_name.take() {
                    let desc = current_desc.take().unwrap_or_else(|| name.clone());
                    if !name.contains(".monitor") {
                        sources.push((name, desc));
                    }
                }
                current_name = Some(trimmed.trim_start_matches("Name: ").to_string());
                current_desc = None;
            } else if trimmed.starts_with("Description: ") {
                current_desc = Some(trimmed.trim_start_matches("Description: ").to_string());
            }
        }
        // Flush last source
        if let Some(name) = current_name {
            let desc = current_desc.unwrap_or_else(|| name.clone());
            if !name.contains(".monitor") {
                sources.push((name, desc));
            }
        }

        Ok(sources)
    }

    /// Whether the real microphone is currently being passed through to the virtual sink.
    pub fn is_mic_passthrough(&self) -> bool {
        self.loopback_module_id.is_some()
    }

    /// Enable real microphone passthrough: routes a specific PulseAudio source
    /// into the virtual sink so VRChat hears the real mic instead of TTS.
    pub fn enable_mic_passthrough(&mut self, source_name: &str) -> Result<()> {
        if self.loopback_module_id.is_some() {
            return Ok(()); // Already enabled
        }

        let output = Command::new("pactl")
            .args([
                "load-module",
                "module-loopback",
                &format!("source={}", source_name),
                &format!("sink={}", self.sink_name),
                "latency_msec=30",
            ])
            .output()
            .context("Failed to create mic loopback")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("module-loopback failed: {}", stderr.trim());
        }

        let id_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        self.loopback_module_id = id_str.parse().ok();
        tracing::info!("Mic passthrough enabled (module {})", id_str);
        Ok(())
    }

    /// Disable real microphone passthrough.
    pub fn disable_mic_passthrough(&mut self) -> Result<()> {
        if let Some(id) = self.loopback_module_id.take() {
            let output = Command::new("pactl")
                .args(["unload-module", &id.to_string()])
                .output()
                .context("Failed to unload loopback module")?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("Failed to unload loopback module {}: {}", id, stderr);
            } else {
                tracing::info!("Mic passthrough disabled (module {})", id);
            }
        }
        Ok(())
    }

    /// Remove stale loopback modules targeting this sink from previous sessions.
    pub fn cleanup_stale_loopbacks(&self) {
        let output = Command::new("pactl")
            .args(["list", "short", "modules"])
            .output();
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("module-loopback") && line.contains(&self.sink_name) {
                    if let Some(id_str) = line.split_whitespace().next() {
                        let _ = Command::new("pactl")
                            .args(["unload-module", id_str])
                            .output();
                        tracing::info!(
                            "Cleaned up stale loopback module {} (sink={})",
                            id_str,
                            self.sink_name
                        );
                    }
                }
            }
        }
    }

    pub fn destroy(&mut self) -> Result<()> {
        // Destroy loopback first
        let _ = self.disable_mic_passthrough();
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
