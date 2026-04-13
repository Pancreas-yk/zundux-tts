use crate::ui::theme::Theme;
use crate::validation;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TtsEngineType {
    #[default]
    Voicevox,
    Voiceger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub voicevox_url: String,
    pub voicevox_path: String,
    pub auto_launch_voicevox: bool,
    #[serde(default)]
    pub auto_launch_voiceger: bool,
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
    #[serde(default = "default_window_width")]
    pub window_width: f32,
    #[serde(default = "default_window_height")]
    pub window_height: f32,
    #[serde(default)]
    pub active_engine: TtsEngineType,
    #[serde(default = "default_voiceger_url")]
    pub voiceger_url: String,
    #[serde(default)]
    pub voiceger_path: String,
    #[serde(default)]
    pub voiceger_ref_audio: String,
    #[serde(default)]
    pub voiceger_prompt_text: String,
    #[serde(default = "default_voiceger_prompt_lang")]
    pub voiceger_prompt_lang: String,
    #[serde(default)]
    pub voiceger_ref_free: bool,
    /// Per-language client-side text replacements applied before Voiceger synthesis.
    /// Outer key = language code (ja/en/zh/ko/yue), inner key = surface, value = reading.
    #[serde(default)]
    pub voiceger_dict: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
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
    #[serde(default)]
    pub engine: TtsEngineType,
    /// Voiceger emotion name (e.g. "甘え"). Empty string = ノーマル (use global ref audio).
    #[serde(default)]
    pub voiceger_emotion: String,
    /// Voiceger only: optional per-preset reference wav path.
    /// If set, this takes precedence over emotion/global reference audio.
    #[serde(default)]
    pub voiceger_ref_audio_override: String,
}

fn default_target_lufs() -> f64 {
    -14.0
}

fn default_loudness_tolerance() -> f64 {
    3.0
}

fn default_window_width() -> f32 {
    560.0
}

fn default_window_height() -> f32 {
    700.0
}

fn default_voiceger_url() -> String {
    "http://localhost:9880".to_string()
}

fn default_voiceger_prompt_lang() -> String {
    "ja".to_string()
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
            auto_launch_voiceger: false,
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
            window_width: 560.0,
            window_height: 700.0,
            active_engine: TtsEngineType::Voicevox,
            voiceger_url: "http://localhost:9880".to_string(),
            voiceger_path: String::new(),
            voiceger_ref_audio: String::new(),
            voiceger_prompt_text: String::new(),
            voiceger_prompt_lang: "ja".to_string(),
            voiceger_ref_free: false,
            voiceger_dict: std::collections::HashMap::new(), // per-lang dicts initialized on demand
        }
    }
}

impl AppConfig {
    /// Default Voiceger install directory (~/voiceger_v2, matching install.sh).
    pub fn default_voiceger_install_dir() -> PathBuf {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("voiceger_v2")
    }

    /// Build the default launch command from the standard install.sh layout.
    /// Uses `conda run` if conda is found, otherwise falls back to plain python.
    pub fn default_voiceger_launch_cmd() -> String {
        let api_py = Self::default_voiceger_install_dir()
            .join("GPT-SoVITS")
            .join("api_v2.py");

        // Look for conda in common locations
        let home = std::env::var("HOME").unwrap_or_default();
        let miniconda = format!("{home}/miniconda3/bin/conda");
        let anaconda = format!("{home}/anaconda3/bin/conda");
        let conda_candidates = ["conda", miniconda.as_str(), anaconda.as_str()];
        for candidate in &conda_candidates {
            if std::process::Command::new(candidate)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return format!(
                    "{} run -n voiceger --no-capture-output python {}",
                    candidate,
                    api_py.display()
                );
            }
        }

        // Fallback: plain python in PATH
        format!("python {}", api_py.display())
    }

    /// The effective launch command: user-set value, or the computed default.
    pub fn effective_voiceger_launch_cmd(&self) -> String {
        if self.voiceger_path.trim().is_empty() {
            Self::default_voiceger_launch_cmd()
        } else {
            self.voiceger_path.clone()
        }
    }

    /// Derive the Voiceger repository root from voiceger_path.
    /// e.g. "python /home/user/voiceger_v2/api.py" → "/home/user/voiceger_v2"
    /// Falls back to the default install directory when voiceger_path is empty.
    pub fn voiceger_base_dir(&self) -> Option<PathBuf> {
        if self.voiceger_path.trim().is_empty() {
            let dir = Self::default_voiceger_install_dir();
            return if dir.exists() { Some(dir) } else { None };
        }
        let words = shell_words::split(self.voiceger_path.trim()).ok()?;
        // Prefer a .py script argument over any other path-like word.
        // e.g. "python /path/to/api_v2.py" → pick api_v2.py, not the python binary.
        let script = words
            .iter()
            .find(|w| w.ends_with(".py"))
            .or_else(|| words.iter().find(|w| w.contains('/')))?;
        let parent = PathBuf::from(script).parent().map(|p| p.to_path_buf())?;
        // api_v2.py is inside GPT-SoVITS/, so the repo root is one level up
        parent.parent().map(|p| p.to_path_buf()).or(Some(parent))
    }

    /// Default ref audio path derived from voiceger_path.
    pub fn default_voiceger_ref_audio(&self) -> String {
        self.voiceger_base_dir()
            .map(|d| {
                d.join("reference")
                    .join("01_ref_emoNormal026.wav")
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_default()
    }

    pub const DEFAULT_VOICEGER_PROMPT_LANG: &'static str = "ja";

    /// Read prompt text from ref_text.txt in the reference folder, falling back to the known default.
    pub fn default_voiceger_prompt_text(&self) -> String {
        if let Some(base) = self.voiceger_base_dir() {
            let path = base.join("reference").join("ref_text.txt");
            if let Ok(text) = std::fs::read_to_string(&path) {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    return trimmed;
                }
            }
        }
        // All _026.wav reference files contain JSUT dataset utterance #026
        "私はいつもミネラルウォーターを持ち歩いています".to_string()
    }

    /// Apply all Voiceger defaults derived from voiceger_path.
    pub fn reset_voiceger_defaults(&mut self) {
        self.voiceger_ref_audio = self.default_voiceger_ref_audio();
        self.voiceger_prompt_text = self.default_voiceger_prompt_text();
        self.voiceger_prompt_lang = Self::DEFAULT_VOICEGER_PROMPT_LANG.to_string();
    }

    pub fn default_presets() -> Vec<SpeakerPreset> {
        vec![
            SpeakerPreset {
                name: "デフォルト：ずんだもん".to_string(),
                speaker_id: 3,
                synth_params: SynthParamsConfig::default(),
                engine: TtsEngineType::Voicevox,
                voiceger_emotion: String::new(),
                voiceger_ref_audio_override: String::new(),
            },
            SpeakerPreset {
                name: "デフォルト：めたん".to_string(),
                speaker_id: 2,
                synth_params: SynthParamsConfig::default(),
                engine: TtsEngineType::Voicevox,
                voiceger_emotion: String::new(),
                voiceger_ref_audio_override: String::new(),
            },
            SpeakerPreset {
                name: "デフォルト：つむぎ".to_string(),
                speaker_id: 8,
                synth_params: SynthParamsConfig::default(),
                engine: TtsEngineType::Voicevox,
                voiceger_emotion: String::new(),
                voiceger_ref_audio_override: String::new(),
            },
            SpeakerPreset {
                name: "Voiceger：日本語".to_string(),
                speaker_id: 0,
                synth_params: SynthParamsConfig::default(),
                engine: TtsEngineType::Voiceger,
                voiceger_emotion: String::new(),
                voiceger_ref_audio_override: String::new(),
            },
            SpeakerPreset {
                name: "Voiceger：English".to_string(),
                speaker_id: 1,
                synth_params: SynthParamsConfig::default(),
                engine: TtsEngineType::Voiceger,
                voiceger_emotion: String::new(),
                voiceger_ref_audio_override: String::new(),
            },
            SpeakerPreset {
                name: "Voiceger：中文".to_string(),
                speaker_id: 2,
                synth_params: SynthParamsConfig::default(),
                engine: TtsEngineType::Voiceger,
                voiceger_emotion: String::new(),
                voiceger_ref_audio_override: String::new(),
            },
            SpeakerPreset {
                name: "Voiceger：한국어".to_string(),
                speaker_id: 3,
                synth_params: SynthParamsConfig::default(),
                engine: TtsEngineType::Voiceger,
                voiceger_emotion: String::new(),
                voiceger_ref_audio_override: String::new(),
            },
            SpeakerPreset {
                name: "Voiceger：粤語".to_string(),
                speaker_id: 4,
                synth_params: SynthParamsConfig::default(),
                engine: TtsEngineType::Voiceger,
                voiceger_emotion: String::new(),
                voiceger_ref_audio_override: String::new(),
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
