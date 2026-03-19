use crate::validation;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub voicevox_url: String,
    pub voicevox_path: String,
    pub auto_launch_voicevox: bool,
    pub synth_params: SynthParamsConfig,
    pub speaker_id: u32,
    pub virtual_device_name: String,
    pub monitor_audio: bool,
    pub templates: Vec<String>,
    pub osc_enabled: bool,
    pub osc_address: String,
    pub osc_port: u16,
    pub soundboard_path: String,
    pub echo_enabled: bool,
    pub echo_delay_ms: u32,
    pub echo_decay: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SynthParamsConfig {
    pub speed_scale: f64,
    pub pitch_scale: f64,
    pub intonation_scale: f64,
    pub volume_scale: f64,
}

impl Default for SynthParamsConfig {
    fn default() -> Self {
        Self {
            speed_scale: 1.0,
            pitch_scale: 0.0,
            intonation_scale: 1.0,
            volume_scale: 1.0,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            voicevox_url: "http://127.0.0.1:50021".to_string(),
            voicevox_path: "voicevox".to_string(),
            auto_launch_voicevox: false,
            synth_params: SynthParamsConfig::default(),
            speaker_id: 3, // ずんだもん (ノーマル)
            monitor_audio: true,
            virtual_device_name: "ZundamonVRC".to_string(),
            templates: vec![
                "こんにちは！".to_string(),
                "ありがとう！".to_string(),
                "おつかれさまなのだ！".to_string(),
                "了解なのだ！".to_string(),
            ],
            osc_enabled: false,
            osc_address: "127.0.0.1".to_string(),
            osc_port: 9000,
            soundboard_path: ProjectDirs::from("", "", "zundamon_vrc")
                .map(|d| d.config_dir().join("sounds").to_string_lossy().to_string())
                .unwrap_or_else(|| "sounds".to_string()),
            echo_enabled: false,
            echo_delay_ms: 200,
            echo_decay: 0.4,
        }
    }
}

impl AppConfig {
    fn config_dir() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "zundamon_vrc")
            .context("Failed to determine config directory")?;
        Ok(dirs.config_dir().to_path_buf())
    }

    fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }
        validation::check_config_file_size(&path)?;
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        let mut config: Self =
            toml::from_str(&content).with_context(|| "Failed to parse config TOML")?;
        config.validate_and_sanitize();
        Ok(config)
    }

    fn validate_and_sanitize(&mut self) {
        if !validation::is_valid_device_name(&self.virtual_device_name) {
            tracing::warn!(
                "Invalid virtual_device_name '{}', using default",
                self.virtual_device_name
            );
            self.virtual_device_name = "ZundamonVRC".to_string();
        }

        if let Err(e) = validation::is_valid_voicevox_url(&self.voicevox_url) {
            tracing::warn!("Invalid voicevox_url: {}, using default", e);
            self.voicevox_url = "http://127.0.0.1:50021".to_string();
        }

        if self.templates.len() > 100 {
            tracing::warn!(
                "Too many templates ({}), truncating to 100",
                self.templates.len()
            );
            self.templates.truncate(100);
        }
        for t in &mut self.templates {
            if t.len() > 512 {
                tracing::warn!("Template too long, truncating to 512 chars");
                *t = t.chars().take(512).collect();
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = Self::config_path()?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
