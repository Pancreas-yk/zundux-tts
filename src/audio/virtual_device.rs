use anyhow::{Context, Result};
use std::process::Command;

/// Parse the module ID printed by `pactl load-module`.  Logs a warning (with
/// the raw output) when parsing fails so the operator can clean up by hand — a
/// lost ID means the module stays loaded until `cleanup_stale_loopbacks` runs.
fn parse_module_id(stdout: &[u8], what: &str) -> Option<u32> {
    let text = String::from_utf8_lossy(stdout);
    let trimmed = text.trim();
    match trimmed.parse::<u32>() {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!("Failed to parse {what} module id from pactl output {trimmed:?}: {e}");
            None
        }
    }
}

pub struct VirtualDevice {
    pub(crate) sink_name: String,
    sink_module_id: Option<u32>,
    source_module_id: Option<u32>,
    loopback_module_id: Option<u32>,
    ladspa_module_id: Option<u32>,
}

impl VirtualDevice {
    pub fn new(sink_name: &str) -> Self {
        Self {
            sink_name: sink_name.to_string(),
            sink_module_id: None,
            source_module_id: None,
            loopback_module_id: None,
            ladspa_module_id: None,
        }
    }

    /// Check if the RNNoise LADSPA plugin is available on the system.
    pub fn is_rnnoise_available() -> bool {
        std::path::Path::new("/usr/lib/ladspa/librnnoise_ladspa.so").exists()
    }

    fn denoised_source_name(&self) -> String {
        format!("{}_denoised", self.sink_name)
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

            self.sink_module_id = parse_module_id(&output.stdout, "null-sink");
            tracing::info!(
                "Created virtual sink {} (module {:?})",
                self.sink_name,
                self.sink_module_id
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
                self.source_module_id = parse_module_id(&output.stdout, "virtual-source");
                tracing::info!(
                    "Created virtual source {} (module {:?})",
                    source_name,
                    self.source_module_id
                );
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
    /// When `noise_suppression` is true and RNNoise is available, an intermediate
    /// LADSPA source is created to denoise the mic before routing.
    pub fn enable_mic_passthrough(
        &mut self,
        source_name: &str,
        noise_suppression: bool,
    ) -> Result<()> {
        if self.loopback_module_id.is_some() {
            return Ok(()); // Already enabled
        }

        let effective_source = if noise_suppression && Self::is_rnnoise_available() {
            let denoised = self.denoised_source_name();
            let output = Command::new("pactl")
                .args([
                    "load-module",
                    "module-ladspa-source",
                    &format!("source_name={}", denoised),
                    &format!("master={}", source_name),
                    "plugin=librnnoise_ladspa",
                    "label=noise_suppressor_mono",
                ])
                .output()
                .context("Failed to create RNNoise LADSPA source")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!(
                    "RNNoise LADSPA failed ({}), falling back to raw mic",
                    stderr.trim()
                );
                source_name.to_string()
            } else {
                self.ladspa_module_id = parse_module_id(&output.stdout, "ladspa-source");
                tracing::info!(
                    "RNNoise noise suppression enabled (module {:?})",
                    self.ladspa_module_id
                );
                denoised
            }
        } else {
            if noise_suppression && !Self::is_rnnoise_available() {
                tracing::warn!(
                    "noise-suppression-for-voice is not installed, skipping noise cancellation"
                );
            }
            source_name.to_string()
        };

        let output = Command::new("pactl")
            .args([
                "load-module",
                "module-loopback",
                &format!("source={}", effective_source),
                &format!("sink={}", self.sink_name),
                "latency_msec=30",
            ])
            .output()
            .context("Failed to create mic loopback")?;

        if !output.status.success() {
            self.unload_ladspa();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("module-loopback failed: {}", stderr.trim());
        }

        self.loopback_module_id = parse_module_id(&output.stdout, "loopback");
        tracing::info!(
            "Mic passthrough enabled (module {:?})",
            self.loopback_module_id
        );
        Ok(())
    }

    fn unload_ladspa(&mut self) {
        if let Some(id) = self.ladspa_module_id.take() {
            let output = Command::new("pactl")
                .args(["unload-module", &id.to_string()])
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    tracing::info!("Unloaded RNNoise LADSPA module {}", id);
                }
                _ => {
                    tracing::warn!("Failed to unload RNNoise LADSPA module {}", id);
                }
            }
        }
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
        self.unload_ladspa();
        Ok(())
    }

    /// Remove stale loopback modules targeting this sink from previous sessions.
    pub fn cleanup_stale_loopbacks(&self) {
        let output = Command::new("pactl")
            .args(["list", "short", "modules"])
            .output();
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let denoised = self.denoised_source_name();
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
                if line.contains("module-ladspa-source") && line.contains(&denoised) {
                    if let Some(id_str) = line.split_whitespace().next() {
                        let _ = Command::new("pactl")
                            .args(["unload-module", id_str])
                            .output();
                        tracing::info!("Cleaned up stale LADSPA module {}", id_str);
                    }
                }
            }
        }
    }

    pub fn destroy(&mut self) -> Result<()> {
        // Destroy loopback and LADSPA first
        let _ = self.disable_mic_passthrough();
        self.unload_ladspa();
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
        // On app exit, keep the null sink and virtual source alive in PipeWire.
        // Leaving them loaded means VRChat retains its microphone connection and
        // does not require a manual mic-toggle after the app is restarted.
        // Only the loopback and LADSPA modules are transient and must be cleaned up.
        let _ = self.disable_mic_passthrough();
        self.unload_ladspa();
        // Belt-and-suspenders: if a module-id parse failed earlier (rare), the
        // tracked handle is None and the calls above are no-ops.  Scan the
        // module list for anything still pointing at our sink / denoised source
        // so a panic or an unparsable pactl response can't leave modules behind.
        self.cleanup_stale_loopbacks();
        // Forget the module IDs so destroy() (if called explicitly) is a no-op for
        // sink/source, and pactl unload-module is not issued for them.
        self.sink_module_id = None;
        self.source_module_id = None;
    }
}
