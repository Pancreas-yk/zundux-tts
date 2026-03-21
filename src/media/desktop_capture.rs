use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SinkInput {
    pub id: u32,
    pub name: String,
    pub sink: String,
}

pub struct DesktopCapture {
    combined_module_id: Option<u32>,
    captured_input_id: Option<u32>,
    original_sink: Option<String>,
}

impl DesktopCapture {
    pub fn new() -> Self {
        Self {
            combined_module_id: None,
            captured_input_id: None,
            original_sink: None,
        }
    }

    pub fn list_sink_inputs() -> Result<Vec<SinkInput>> {
        let output = Command::new("pactl")
            .args(["list", "sink-inputs"])
            .output()
            .context("Failed to run pactl list sink-inputs")?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut inputs = Vec::new();
        let mut current_id: Option<u32> = None;
        let mut current_name = String::new();
        let mut current_sink = String::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("Sink Input #") {
                if let Some(id) = current_id {
                    if !current_name.is_empty()
                        && crate::validation::is_valid_pa_name(&current_sink)
                    {
                        inputs.push(SinkInput {
                            id,
                            name: current_name.clone(),
                            sink: current_sink.clone(),
                        });
                    }
                }
                current_id = rest.parse().ok();
                current_name.clear();
                current_sink.clear();
            } else if trimmed.starts_with("application.name = ") {
                current_name = trimmed
                    .strip_prefix("application.name = ")
                    .unwrap_or("")
                    .trim_matches('"')
                    .to_string();
            } else if trimmed.starts_with("Sink: ") {
                current_sink = trimmed.strip_prefix("Sink: ").unwrap_or("").to_string();
            }
        }
        if let Some(id) = current_id {
            if !current_name.is_empty() && crate::validation::is_valid_pa_name(&current_sink) {
                inputs.push(SinkInput {
                    id,
                    name: current_name,
                    sink: current_sink,
                });
            }
        }

        Ok(inputs)
    }

    pub fn start_capture(
        &mut self,
        sink_input_id: u32,
        original_sink: &str,
        virtual_sink: &str,
        skip_speaker: bool,
    ) -> Result<()> {
        self.stop_capture();

        let slaves = if skip_speaker {
            tracing::info!("Capture: speaker output skipped (mic passthrough active)");
            virtual_sink.to_string()
        } else {
            let default_sink = Self::get_default_sink()?;
            format!("{},{}", virtual_sink, default_sink)
        };

        let output = Command::new("pactl")
            .args([
                "load-module",
                "module-combine-sink",
                "sink_name=ZunduxCombined",
                &format!("slaves={}", slaves),
            ])
            .output()
            .context("Failed to create combine-sink")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create combine-sink: {}", stderr);
        }

        let module_id: u32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .context("Failed to parse combine-sink module ID")?;

        let output = Command::new("pactl")
            .args([
                "move-sink-input",
                &sink_input_id.to_string(),
                "ZunduxCombined",
            ])
            .output()
            .context("Failed to move sink-input")?;

        if !output.status.success() {
            let _ = Command::new("pactl")
                .args(["unload-module", &module_id.to_string()])
                .output();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to redirect audio: {}", stderr);
        }

        self.combined_module_id = Some(module_id);
        self.captured_input_id = Some(sink_input_id);
        self.original_sink = Some(original_sink.to_string());

        tracing::info!(
            "Started desktop capture: sink-input {} → ZunduxCombined (skip_speaker={})",
            sink_input_id,
            skip_speaker
        );
        Ok(())
    }

    /// Restart capture with updated speaker routing (e.g. when mic toggle changes).
    pub fn restart_capture(&mut self, virtual_sink: &str, skip_speaker: bool) -> Result<()> {
        let input_id = match self.captured_input_id {
            Some(id) => id,
            None => return Ok(()),
        };
        let original_sink = match self.original_sink.clone() {
            Some(s) => s,
            None => return Ok(()),
        };
        self.start_capture(input_id, &original_sink, virtual_sink, skip_speaker)
    }

    pub fn stop_capture(&mut self) {
        if let (Some(input_id), Some(ref sink)) = (self.captured_input_id, &self.original_sink) {
            let _ = Command::new("pactl")
                .args(["move-sink-input", &input_id.to_string(), sink])
                .output();
        }
        if let Some(module_id) = self.combined_module_id.take() {
            let _ = Command::new("pactl")
                .args(["unload-module", &module_id.to_string()])
                .output();
        }
        self.captured_input_id = None;
        self.original_sink = None;
    }

    fn get_default_sink() -> Result<String> {
        let output = Command::new("pactl")
            .args(["get-default-sink"])
            .output()
            .context("Failed to get default sink")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn cleanup_stale() {
        let output = Command::new("pactl")
            .args(["list", "short", "modules"])
            .output();
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("module-combine-sink") && line.contains("ZunduxCombined") {
                    if let Some(id_str) = line.split_whitespace().next() {
                        let _ = Command::new("pactl")
                            .args(["unload-module", id_str])
                            .output();
                        tracing::info!("Cleaned up stale ZunduxCombined module {}", id_str);
                    }
                }
            }
        }
    }
}

impl Drop for DesktopCapture {
    fn drop(&mut self) {
        self.stop_capture();
    }
}
