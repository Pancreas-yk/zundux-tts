use crate::ui::theme::Theme;
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
    pub auto_start_app: bool,
    pub synth_params: SynthParamsConfig,
    pub speaker_id: u32,
    pub virtual_device_name: String,
    pub monitor_audio: bool,
    pub templates: Vec<String>,
    pub osc_enabled: bool,
    pub osc_address: String,
    pub osc_port: u16,
    pub soundboard_path: String,
    pub mic_source_name: Option<String>,
    pub echo_enabled: bool,
    pub echo_delay_ms: u32,
    pub echo_decay: f64,
    #[serde(default = "default_target_lufs")]
    pub target_lufs: f64,
    #[serde(default = "default_loudness_tolerance")]
    pub loudness_tolerance: f64,
    #[serde(default)]
    pub soundboard_gains: std::collections::HashMap<String, f64>,
    #[serde(default)]
    pub noise_suppression: bool,
    #[serde(default)]
    pub silent_words: Vec<String>,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub presets: Vec<SpeakerPreset>,
    #[serde(default)]
    pub templates_default_expanded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SynthParamsConfig {
    pub speed_scale: f64,
    pub pitch_scale: f64,
    pub intonation_scale: f64,
    pub volume_scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerPreset {
    pub name: String,
    pub speaker_id: u32,
    pub synth_params: SynthParamsConfig,
}

fn default_target_lufs() -> f64 {
    -14.0
}

fn default_loudness_tolerance() -> f64 {
    3.0
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
            voicevox_url: validation::DEFAULT_VOICEVOX_URL.to_string(),
            voicevox_path: String::new(),
            auto_launch_voicevox: false,
            auto_start_app: false,
            synth_params: SynthParamsConfig::default(),
            speaker_id: 3, // ずんだもん (ノーマル)
            monitor_audio: true,
            virtual_device_name: validation::DEFAULT_DEVICE_NAME.to_string(),
            templates: vec![
                "こんにちは！".to_string(),
                "ありがとう！".to_string(),
                "おつかれさまなのだ！".to_string(),
                "了解なのだ！".to_string(),
            ],
            osc_enabled: false,
            osc_address: "127.0.0.1".to_string(),
            osc_port: 9000,
            soundboard_path: ProjectDirs::from("", "", "zundux_tts")
                .map(|d| d.config_dir().join("sounds").to_string_lossy().to_string())
                .unwrap_or_else(|| "sounds".to_string()),
            mic_source_name: None,
            echo_enabled: false,
            echo_delay_ms: 200,
            echo_decay: 0.4,
            target_lufs: -14.0,
            loudness_tolerance: 3.0,
            soundboard_gains: std::collections::HashMap::new(),
            noise_suppression: false,
            silent_words: Vec::new(),
            theme: Theme::default(),
            presets: Self::default_presets(),
            templates_default_expanded: false,
        }
    }
}

impl AppConfig {
    pub fn default_presets() -> Vec<SpeakerPreset> {
        vec![
            SpeakerPreset {
                name: "デフォルト：ずんだもん".to_string(),
                speaker_id: 3,
                synth_params: SynthParamsConfig::default(),
            },
            SpeakerPreset {
                name: "デフォルト：めたん".to_string(),
                speaker_id: 2,
                synth_params: SynthParamsConfig::default(),
            },
            SpeakerPreset {
                name: "デフォルト：つむぎ".to_string(),
                speaker_id: 8,
                synth_params: SynthParamsConfig::default(),
            },
        ]
    }

    fn config_dir() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "zundux_tts")
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
            self.virtual_device_name = validation::DEFAULT_DEVICE_NAME.to_string();
        }

        if let Err(e) = validation::is_valid_voicevox_url(&self.voicevox_url) {
            tracing::warn!("Invalid voicevox_url: {}, using default", e);
            self.voicevox_url = validation::DEFAULT_VOICEVOX_URL.to_string();
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

        self.theme = std::mem::take(&mut self.theme).validated();

        // Migrate: if no presets exist, add the three named defaults.
        if self.presets.is_empty() {
            self.presets = Self::default_presets();
        }
        if self.presets.len() > 50 {
            self.presets.truncate(50);
        }
        for p in &mut self.presets {
            if p.name.len() > 64 {
                p.name = p.name.chars().take(64).collect();
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

    /// Returns the path to the XDG autostart .desktop file
    fn autostart_desktop_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(".config/autostart/zundux_tts.desktop"))
    }

    /// Returns the path to the currently running executable
    fn current_exe_path() -> Result<String> {
        let exe = std::env::current_exe().context("Failed to get current exe path")?;
        Ok(exe.to_string_lossy().to_string())
    }

    /// Check if the autostart .desktop file exists
    pub fn is_autostart_enabled() -> bool {
        Self::autostart_desktop_path()
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    /// Install or remove the autostart .desktop entry
    pub fn set_autostart(enabled: bool) -> Result<()> {
        let desktop_path = Self::autostart_desktop_path()?;

        if enabled {
            let exe_path = Self::current_exe_path()?;
            let autostart_dir = desktop_path.parent().context("No parent dir")?;
            std::fs::create_dir_all(autostart_dir)?;

            let content = format!(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Name=ZunduxTTS\n\
                 Comment=VOICEVOX TTS virtual microphone\n\
                 Exec={exe_path}\n\
                 Terminal=false\n\
                 X-GNOME-Autostart-enabled=true\n"
            );
            std::fs::write(&desktop_path, content)
                .with_context(|| format!("Failed to write {}", desktop_path.display()))?;
            tracing::info!("Autostart enabled: {}", desktop_path.display());
        } else if desktop_path.exists() {
            std::fs::remove_file(&desktop_path)
                .with_context(|| format!("Failed to remove {}", desktop_path.display()))?;
            tracing::info!("Autostart disabled: removed {}", desktop_path.display());
        }
        Ok(())
    }
}
