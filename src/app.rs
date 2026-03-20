use crate::audio::AudioManager;
use crate::config::AppConfig;
use crate::tts::types::{Speaker, SynthParams};
use crate::tts::voicevox::VoicevoxEngine;
use crate::tts::TtsManager;
use crate::ui::Screen;

use anyhow::Context as _;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

struct PlaybackGuard(Arc<AtomicBool>);
impl Drop for PlaybackGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Messages from the tokio runtime back to the UI thread
enum UiMessage {
    WavReady { wav: Vec<u8>, text: String },
    SpeakersLoaded(Vec<Speaker>),
    HealthCheckResult(bool),
    Error(String),
    UserDictLoaded(crate::tts::types::UserDict),
    UserDictUpdated,
}

/// Commands from the UI thread to the tokio runtime
pub enum TtsCommand {
    Synthesize {
        text: String,
        params: SynthParams,
    },
    LoadSpeakers,
    HealthCheck,
    LoadUserDict,
    AddUserDictWord {
        surface: String,
        pronunciation: String,
    },
    DeleteUserDictWord {
        uuid: String,
    },
}

/// Shared mutable state accessed by UI drawing functions
pub struct AppState {
    pub config: AppConfig,
    pub input_text: String,
    pub speakers: Vec<Speaker>,
    pub voicevox_connected: bool,
    pub device_ready: bool,
    pub is_synthesizing: bool,
    pub is_playing: bool,
    pub last_error: Option<String>,
    pub pending_send: Option<String>,
    pub pending_health_check: bool,
    pub pending_create_device: bool,
    pub pending_destroy_device: bool,
    pub pending_launch_voicevox: bool,
    pub voicevox_launching: bool,
    pub current_screen: Screen,
    pub new_template_text: String,
    pub user_dict: Vec<(String, String, String)>, // (uuid, surface, pronunciation)
    pub new_dict_surface: String,
    pub new_dict_pronunciation: String,
    pub pending_load_user_dict: bool,
    pub pending_add_dict_word: Option<(String, String)>,
    pub pending_delete_dict_word: Option<String>,
    pub soundboard_files: Vec<(String, std::path::PathBuf)>,
    pub pending_soundboard_scan: bool,
    pub pending_soundboard_play: Option<std::path::PathBuf>,
    pub pending_soundboard_stop: bool,
    pub is_soundboard_playing: bool,
    pub sink_inputs: Vec<crate::media::desktop_capture::SinkInput>,
    pub pending_refresh_sink_inputs: bool,
    pub pending_start_capture: Option<(u32, String)>,
    pub pending_stop_capture: bool,
    pub is_capturing: bool,
    pub error_display_time: Option<std::time::Instant>,
    pub error_hovered: bool,
    pub templates_expanded: bool,
    pub adding_template: bool,
    pub needs_theme_update: bool,
    pub mic_passthrough: bool,
    pub pending_toggle_mic: bool,
    pub pending_reconnect_mic: bool,
    pub available_mic_sources: Vec<(String, String)>, // (name, description)
    pub pending_refresh_mic_sources: bool,
    pub color_edit_buffers: std::collections::HashMap<String, String>,
    pub pending_stop_speaking: bool,
}

const DOCKER_CONTAINER_NAME: &str = "zundux-voicevox";
const DOCKER_NAME_FILTER: &str = "name=zundux-voicevox";

pub struct ZunduxApp {
    state: AppState,
    audio_manager: AudioManager,
    ui_rx: mpsc::Receiver<UiMessage>,
    tts_tx: mpsc::Sender<TtsCommand>,
    voicevox_process: Option<Child>,
    is_docker: bool,
    last_health_check: Instant,
    pub is_playing: Arc<AtomicBool>,
    tts_cancel: Arc<AtomicBool>,
    is_soundboard_playing: Arc<AtomicBool>,
    soundboard_pids: Arc<std::sync::Mutex<Vec<u32>>>,
    soundboard_cancel: Arc<AtomicBool>,
    desktop_capture: crate::media::desktop_capture::DesktopCapture,
    needs_theme_update: bool,
}

const HEALTH_CHECK_INTERVAL_SECS: u64 = 5;
const HEALTH_CHECK_INTERVAL_LAUNCHING_SECS: u64 = 1;

impl ZunduxApp {
    pub fn new(config: AppConfig, tts_manager: TtsManager, rt: tokio::runtime::Handle) -> Self {
        let (ui_tx, ui_rx) = mpsc::channel::<UiMessage>();
        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();

        let auto_launch_voicevox = config.auto_launch_voicevox;
        let device_name = config.virtual_device_name.clone();
        let mut audio_manager = AudioManager::new(&device_name);
        let device_ready =
            audio_manager.ensure_device().is_ok() && audio_manager.device_exists().unwrap_or(false);

        crate::media::desktop_capture::DesktopCapture::cleanup_stale();
        audio_manager.virtual_device.cleanup_stale_loopbacks();

        // Spawn the TTS command processing loop on tokio
        let voicevox_url = config.voicevox_url.clone();
        rt.spawn(Self::tts_loop(tts_manager, voicevox_url, tts_rx, ui_tx));

        // Trigger initial health check + speaker load
        let _ = tts_tx.send(TtsCommand::HealthCheck);
        let _ = tts_tx.send(TtsCommand::LoadSpeakers);

        Self {
            state: AppState {
                config,
                input_text: String::new(),
                speakers: Vec::new(),
                voicevox_connected: false,
                device_ready,
                is_synthesizing: false,
                is_playing: false,
                last_error: None,
                pending_send: None,
                pending_health_check: false,
                pending_create_device: false,
                pending_destroy_device: false,
                pending_launch_voicevox: auto_launch_voicevox,
                voicevox_launching: false,
                current_screen: Screen::Input,
                new_template_text: String::new(),
                user_dict: Vec::new(),
                new_dict_surface: String::new(),
                new_dict_pronunciation: String::new(),
                pending_load_user_dict: false,
                pending_add_dict_word: None,
                pending_delete_dict_word: None,
                soundboard_files: Vec::new(),
                pending_soundboard_scan: true,
                pending_soundboard_play: None,
                pending_soundboard_stop: false,
                is_soundboard_playing: false,
                sink_inputs: Vec::new(),
                pending_refresh_sink_inputs: false,
                pending_start_capture: None,
                pending_stop_capture: false,
                is_capturing: false,
                error_display_time: None,
                error_hovered: false,
                templates_expanded: false,
                adding_template: false,
                needs_theme_update: false,
                mic_passthrough: false,
                pending_toggle_mic: false,
                pending_reconnect_mic: false,
                available_mic_sources: Vec::new(),
                pending_refresh_mic_sources: true,
                color_edit_buffers: std::collections::HashMap::new(),
                pending_stop_speaking: false,
            },
            audio_manager,
            ui_rx,
            tts_tx,
            voicevox_process: None,
            is_docker: false,
            last_health_check: Instant::now(),
            is_playing: Arc::new(AtomicBool::new(false)),
            tts_cancel: Arc::new(AtomicBool::new(false)),
            is_soundboard_playing: Arc::new(AtomicBool::new(false)),
            soundboard_pids: Arc::new(std::sync::Mutex::new(Vec::new())),
            soundboard_cancel: Arc::new(AtomicBool::new(false)),
            desktop_capture: crate::media::desktop_capture::DesktopCapture::new(),
            needs_theme_update: true,
        }
    }

    async fn tts_loop(
        tts: TtsManager,
        voicevox_url: String,
        rx: mpsc::Receiver<TtsCommand>,
        tx: mpsc::Sender<UiMessage>,
    ) {
        let dict_engine = VoicevoxEngine::new(&voicevox_url);
        while let Ok(cmd) = rx.recv() {
            match cmd {
                TtsCommand::Synthesize { text, params } => {
                    match tts.synthesize(&text, &params).await {
                        Ok(wav) => {
                            let _ = tx.send(UiMessage::WavReady { wav, text });
                        }
                        Err(e) => {
                            let _ = tx.send(UiMessage::Error(format!("合成エラー: {}", e)));
                        }
                    }
                }
                TtsCommand::LoadSpeakers => match tts.list_speakers().await {
                    Ok(speakers) => {
                        let _ = tx.send(UiMessage::SpeakersLoaded(speakers));
                    }
                    Err(e) => {
                        let _ = tx.send(UiMessage::Error(format!("スピーカー取得失敗: {}", e)));
                    }
                },
                TtsCommand::HealthCheck => match tts.health_check().await {
                    Ok(ok) => {
                        let _ = tx.send(UiMessage::HealthCheckResult(ok));
                    }
                    Err(_) => {
                        let _ = tx.send(UiMessage::HealthCheckResult(false));
                    }
                },
                TtsCommand::LoadUserDict => match dict_engine.list_user_dict().await {
                    Ok(dict) => {
                        let _ = tx.send(UiMessage::UserDictLoaded(dict));
                    }
                    Err(e) => {
                        let _ = tx.send(UiMessage::Error(format!("辞書取得失敗: {}", e)));
                    }
                },
                TtsCommand::AddUserDictWord {
                    surface,
                    pronunciation,
                } => {
                    match dict_engine
                        .add_user_dict_word(&surface, &pronunciation)
                        .await
                    {
                        Ok(_) => {
                            let _ = tx.send(UiMessage::UserDictUpdated);
                        }
                        Err(e) => {
                            let _ = tx.send(UiMessage::Error(format!("辞書登録失敗: {}", e)));
                        }
                    }
                }
                TtsCommand::DeleteUserDictWord { uuid } => {
                    match dict_engine.delete_user_dict_word(&uuid).await {
                        Ok(()) => {
                            let _ = tx.send(UiMessage::UserDictUpdated);
                        }
                        Err(e) => {
                            let _ = tx.send(UiMessage::Error(format!("辞書削除失敗: {}", e)));
                        }
                    }
                }
            }
        }
    }

    fn process_messages(&mut self) {
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMessage::WavReady { wav, text } => {
                    self.state.is_synthesizing = false;
                    self.state.last_error = None;

                    // Send OSC chatbox message before playback
                    if self.state.config.osc_enabled {
                        if let Err(e) = crate::osc::send_chatbox(
                            &self.state.config.osc_address,
                            self.state.config.osc_port,
                            &text,
                        ) {
                            tracing::warn!("OSC send failed: {}", e);
                        }
                    }

                    let wav = if self.state.config.echo_enabled {
                        crate::audio::effects::apply_echo(
                            &wav,
                            self.state.config.echo_delay_ms,
                            self.state.config.echo_decay,
                        )
                    } else {
                        wav
                    };

                    if self.is_playing.load(Ordering::SeqCst) {
                        tracing::warn!("Playback already in progress, dropping TTS audio");
                    } else {
                        // Ensure sink is unmuted before playback
                        let _ = std::process::Command::new("pactl")
                            .args(["set-sink-mute", &self.state.config.virtual_device_name, "0"])
                            .output();
                        let device_name = self.state.config.virtual_device_name.clone();
                        let monitor = self.state.config.monitor_audio;
                        let playing = self.is_playing.clone();
                        let cancel = self.tts_cancel.clone();
                        cancel.store(false, Ordering::SeqCst);
                        playing.store(true, Ordering::SeqCst);
                        std::thread::spawn(move || {
                            let _guard = PlaybackGuard(playing);
                            if let Err(e) =
                                crate::audio::playback::play_wav(wav, &device_name, monitor, cancel)
                            {
                                tracing::error!("Playback error: {}", e);
                            }
                        });
                    }
                }
                UiMessage::SpeakersLoaded(speakers) => {
                    self.state.speakers = speakers;
                }
                UiMessage::HealthCheckResult(ok) => {
                    let was_disconnected = !self.state.voicevox_connected;
                    self.state.voicevox_connected = ok;
                    if ok {
                        self.state.voicevox_launching = false;
                        if was_disconnected {
                            let _ = self.tts_tx.send(TtsCommand::LoadSpeakers);
                        }
                    }
                }
                UiMessage::Error(err) => {
                    self.state.is_synthesizing = false;
                    self.state.last_error = Some(err);
                    self.state.error_display_time = Some(std::time::Instant::now());
                }
                UiMessage::UserDictLoaded(dict) => {
                    self.state.user_dict = dict
                        .into_iter()
                        .map(|(uuid, word)| (uuid, word.surface, word.pronunciation))
                        .collect();
                    self.state.user_dict.sort_by(|a, b| a.1.cmp(&b.1));
                }
                UiMessage::UserDictUpdated => {
                    let _ = self.tts_tx.send(TtsCommand::LoadUserDict);
                }
            }
        }
    }

    fn periodic_health_check(&mut self) {
        let interval = if self.state.voicevox_launching {
            HEALTH_CHECK_INTERVAL_LAUNCHING_SECS
        } else {
            HEALTH_CHECK_INTERVAL_SECS
        };
        if self.last_health_check.elapsed().as_secs() >= interval {
            self.last_health_check = Instant::now();
            let _ = self.tts_tx.send(TtsCommand::HealthCheck);
        }
    }

    fn process_actions(&mut self) {
        // Handle stop speaking
        if self.state.pending_stop_speaking {
            self.state.pending_stop_speaking = false;
            self.tts_cancel.store(true, Ordering::SeqCst);
            // Clear is_playing immediately so new synthesis isn't blocked
            self.is_playing.store(false, Ordering::SeqCst);
            // Mute the virtual sink immediately — kills audio even if paplay buffers remain
            let sink = &self.state.config.virtual_device_name;
            let _ = std::process::Command::new("pactl")
                .args(["set-sink-mute", sink, "1"])
                .output();
            tracing::info!("TTS stop: muted sink {}", sink);
        }

        // Handle text send
        if let Some(text) = self.state.pending_send.take() {
            // Strip silent words before synthesis
            let text = Self::strip_silent_words(&text, &self.state.config.silent_words);
            if text.is_empty() {
                // Nothing left to speak after stripping
            } else {
                let params = SynthParams::from_config(&self.state.config);
                self.state.is_synthesizing = true;
                self.state.last_error = None;
                let _ = self.tts_tx.send(TtsCommand::Synthesize { text, params });
            }
        }

        // Handle health check
        if self.state.pending_health_check {
            self.state.pending_health_check = false;
            let _ = self.tts_tx.send(TtsCommand::HealthCheck);
        }

        // Handle VOICEVOX launch
        if self.state.pending_launch_voicevox {
            self.state.pending_launch_voicevox = false;
            self.launch_voicevox();
        }

        // Handle device creation
        if self.state.pending_create_device {
            self.state.pending_create_device = false;
            self.audio_manager = AudioManager::new(&self.state.config.virtual_device_name);
            match self.audio_manager.ensure_device() {
                Ok(()) => {
                    self.state.device_ready = true;
                    self.state.last_error = None;
                }
                Err(e) => {
                    self.state.last_error = Some(format!("デバイス作成失敗: {}", e));
                }
            }
        }

        // Handle device destruction
        if self.state.pending_destroy_device {
            self.state.pending_destroy_device = false;
            match self.audio_manager.destroy_device() {
                Ok(()) => {
                    self.state.device_ready = false;
                    self.state.last_error = None;
                }
                Err(e) => {
                    self.state.last_error = Some(format!("デバイス削除失敗: {}", e));
                }
            }
        }

        if self.state.pending_load_user_dict {
            self.state.pending_load_user_dict = false;
            let _ = self.tts_tx.send(TtsCommand::LoadUserDict);
        }
        if let Some((surface, pronunciation)) = self.state.pending_add_dict_word.take() {
            let _ = self.tts_tx.send(TtsCommand::AddUserDictWord {
                surface,
                pronunciation,
            });
        }
        if let Some(uuid) = self.state.pending_delete_dict_word.take() {
            let _ = self.tts_tx.send(TtsCommand::DeleteUserDictWord { uuid });
        }

        if self.state.pending_soundboard_scan {
            self.state.pending_soundboard_scan = false;
            let path = std::path::Path::new(&self.state.config.soundboard_path);
            let mut files = Vec::new();
            if path.is_dir() {
                let base_canonical = path.canonicalize().ok();
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if std::fs::symlink_metadata(&p)
                            .map(|m| m.file_type().is_symlink())
                            .unwrap_or(false)
                        {
                            tracing::warn!("Skipping symlink in soundboard path: {}", p.display());
                            continue;
                        }
                        if let Some(base) = &base_canonical {
                            let Ok(canonical) = p.canonicalize() else {
                                continue;
                            };
                            if !canonical.starts_with(base) {
                                tracing::warn!(
                                    "Skipping out-of-directory soundboard file: {}",
                                    p.display()
                                );
                                continue;
                            }
                        }
                        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                            if matches!(ext.to_lowercase().as_str(), "wav" | "mp3" | "ogg") {
                                let name = p
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("?")
                                    .to_string();
                                files.push((name, p));
                            }
                        }
                    }
                }
            } else {
                let _ = std::fs::create_dir_all(path);
            }
            files.sort_by(|a, b| a.0.cmp(&b.0));
            self.state.soundboard_files = files;
        }

        if self.state.pending_refresh_mic_sources {
            self.state.pending_refresh_mic_sources = false;
            match crate::audio::virtual_device::VirtualDevice::list_sources() {
                Ok(sources) => {
                    // Auto-select first source if none configured
                    if self.state.config.mic_source_name.is_none() {
                        if let Some((name, _)) = sources.first() {
                            self.state.config.mic_source_name = Some(name.clone());
                        }
                    }
                    self.state.available_mic_sources = sources;
                }
                Err(e) => {
                    tracing::warn!("Failed to list mic sources: {}", e);
                }
            }
        }

        if self.state.pending_toggle_mic {
            self.state.pending_toggle_mic = false;
            let was_on = self.audio_manager.virtual_device.is_mic_passthrough();
            tracing::info!(
                "Mic toggle requested: currently {}",
                if was_on { "ON" } else { "OFF" }
            );
            let result = if was_on {
                self.audio_manager.virtual_device.disable_mic_passthrough()
            } else {
                let source = self
                    .state
                    .config
                    .mic_source_name
                    .as_deref()
                    .unwrap_or("@DEFAULT_SOURCE@");
                tracing::info!("Enabling mic passthrough with source: {}", source);
                // Ensure sink is unmuted (STOP may have muted it)
                let _ = std::process::Command::new("pactl")
                    .args(["set-sink-mute", &self.state.config.virtual_device_name, "0"])
                    .output();
                self.audio_manager
                    .virtual_device
                    .enable_mic_passthrough(source)
            };
            match result {
                Ok(()) => {
                    self.state.mic_passthrough =
                        self.audio_manager.virtual_device.is_mic_passthrough();
                    tracing::info!("Mic passthrough is now: {}", self.state.mic_passthrough);
                    // Restart capture with updated speaker routing to prevent acoustic feedback
                    if self.state.is_capturing {
                        let virtual_sink = self.state.config.virtual_device_name.clone();
                        if let Err(e) = self
                            .desktop_capture
                            .restart_capture(&virtual_sink, self.state.mic_passthrough)
                        {
                            tracing::warn!("Failed to restart capture after mic toggle: {}", e);
                        }
                    }
                }
                Err(e) => {
                    self.state.last_error = Some(format!("マイク切替失敗: {}", e));
                }
            }
        }

        // Reconnect mic loopback with newly selected source
        if self.state.pending_reconnect_mic {
            self.state.pending_reconnect_mic = false;
            let source = self
                .state
                .config
                .mic_source_name
                .as_deref()
                .unwrap_or("@DEFAULT_SOURCE@");
            tracing::info!("Reconnecting mic loopback with source: {}", source);
            let _ = self.audio_manager.virtual_device.disable_mic_passthrough();
            match self
                .audio_manager
                .virtual_device
                .enable_mic_passthrough(source)
            {
                Ok(()) => {
                    self.state.mic_passthrough =
                        self.audio_manager.virtual_device.is_mic_passthrough();
                }
                Err(e) => {
                    self.state.last_error = Some(format!("マイク再接続失敗: {}", e));
                    self.state.mic_passthrough = false;
                }
            }
        }

        if self.state.pending_soundboard_stop {
            self.state.pending_soundboard_stop = false;
            crate::audio::playback::stop_file_playback(
                &self.soundboard_pids,
                &self.soundboard_cancel,
            );
        }

        if let Some(file_path) = self.state.pending_soundboard_play.take() {
            if self.is_soundboard_playing.load(Ordering::SeqCst) {
                // Stop current playback and start new one
                crate::audio::playback::stop_file_playback(
                    &self.soundboard_pids,
                    &self.soundboard_cancel,
                );
            }
            // Reset cancel signal for new playback
            self.soundboard_cancel.store(false, Ordering::SeqCst);
            let device_name = self.state.config.virtual_device_name.clone();
            let monitor = self.state.config.monitor_audio;
            let playing = self.is_soundboard_playing.clone();
            let pids = self.soundboard_pids.clone();
            let cancel = self.soundboard_cancel.clone();
            playing.store(true, Ordering::SeqCst);
            std::thread::spawn(move || {
                let _guard = PlaybackGuard(playing);
                if let Err(e) = crate::audio::playback::play_file(
                    &file_path,
                    &device_name,
                    monitor,
                    pids,
                    cancel,
                ) {
                    tracing::error!("Soundboard playback error: {}", e);
                }
            });
        }

        if self.state.pending_refresh_sink_inputs {
            self.state.pending_refresh_sink_inputs = false;
            match crate::media::desktop_capture::DesktopCapture::list_sink_inputs() {
                Ok(inputs) => self.state.sink_inputs = inputs,
                Err(e) => self.state.last_error = Some(format!("sink-input取得失敗: {}", e)),
            }
        }

        if let Some((input_id, original_sink)) = self.state.pending_start_capture.take() {
            let virtual_sink = &self.state.config.virtual_device_name;
            let skip_speaker = self.state.mic_passthrough;
            match self.desktop_capture.start_capture(
                input_id,
                &original_sink,
                virtual_sink,
                skip_speaker,
            ) {
                Ok(()) => {
                    self.state.is_capturing = true;
                    self.state.last_error = None;
                }
                Err(e) => {
                    self.state.last_error = Some(format!("キャプチャ開始失敗: {}", e));
                }
            }
        }

        if self.state.pending_stop_capture {
            self.state.pending_stop_capture = false;
            self.desktop_capture.stop_capture();
            self.state.is_capturing = false;
        }
    }

    /// Remove words marked as silent from text before synthesis.
    fn strip_silent_words(text: &str, silent_words: &[String]) -> String {
        let mut result = text.to_string();
        for word in silent_words {
            result = result.replace(word.as_str(), "");
        }
        result.trim().to_string()
    }

    fn is_voicevox_docker_running() -> bool {
        std::process::Command::new("docker")
            .args([
                "ps",
                "--filter",
                DOCKER_NAME_FILTER,
                "--format",
                "{{.Names}}",
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(DOCKER_CONTAINER_NAME))
            .unwrap_or(false)
    }

    fn is_docker_command(cmd: &str) -> bool {
        let trimmed = cmd.trim_start();
        trimmed.starts_with("docker ") || trimmed.starts_with("docker run")
    }

    /// Check if a stopped container with our name exists and can be restarted.
    fn has_stopped_docker_container() -> bool {
        Command::new("docker")
            .args([
                "ps",
                "-a",
                "--filter",
                DOCKER_NAME_FILTER,
                "--filter",
                "status=exited",
                "--format",
                "{{.Names}}",
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(DOCKER_CONTAINER_NAME))
            .unwrap_or(false)
    }

    fn launch_voicevox(&mut self) {
        let path = self.state.config.voicevox_path.trim().to_string();
        if path.is_empty() {
            tracing::warn!("voicevox_path is empty, cannot launch");
            return;
        }

        let is_docker = Self::is_docker_command(&path);

        // Duplicate guard — for Docker, check container; for local, check process
        if is_docker && Self::is_voicevox_docker_running() {
            tracing::info!("VOICEVOX Docker container already running");
            self.state.voicevox_launching = true;
            return;
        }

        // Local process guard
        if let Some(ref mut proc) = self.voicevox_process {
            match proc.try_wait() {
                Ok(None) => {
                    tracing::info!("VOICEVOX process already running");
                    return;
                }
                _ => {
                    self.voicevox_process = None;
                }
            }
        }

        // If a stopped container exists, restart it instead of creating a new one
        if is_docker && Self::has_stopped_docker_container() {
            tracing::info!("Restarting stopped VOICEVOX container");
            let result = Command::new("docker")
                .args(["start", DOCKER_CONTAINER_NAME])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn();
            match result {
                Ok(child) => {
                    self.voicevox_process = Some(child);
                    self.is_docker = true;
                    self.state.last_error = None;
                    self.state.voicevox_launching = true;
                    tracing::info!("VOICEVOX container restarted");
                }
                Err(e) => {
                    tracing::error!("Failed to restart VOICEVOX container: {}", e);
                    self.state.last_error = Some(format!("VOICEVOX再起動失敗: {}", e));
                    self.state.voicevox_launching = false;
                }
            }
            return;
        }

        tracing::info!("Launching VOICEVOX: {}", path);

        let result = if is_docker {
            Self::launch_docker_voicevox(&path, &self.state.config.voicevox_url)
        } else {
            Self::launch_local_voicevox(&path)
        };

        match result {
            Ok(child) => {
                self.voicevox_process = Some(child);
                self.is_docker = is_docker;
                self.state.last_error = None;
                self.state.voicevox_launching = true;
                tracing::info!("VOICEVOX process spawned");
            }
            Err(e) => {
                tracing::error!("Failed to launch VOICEVOX: {}", e);
                self.state.last_error = Some(format!("VOICEVOX起動失敗: {}", e));
            }
        }
    }

    fn launch_docker_voicevox(path: &str, _url: &str) -> anyhow::Result<std::process::Child> {
        let words = shell_words::split(path)
            .map_err(|e| anyhow::anyhow!("Failed to parse docker command: {}", e))?;

        if words.is_empty() {
            anyhow::bail!("Empty docker command");
        }

        // Reject shell metacharacters in any argument
        for word in &words {
            if word.chars().any(|c| {
                matches!(
                    c,
                    ';' | '|' | '&' | '$' | '`' | '(' | ')' | '{' | '}' | '<' | '>'
                )
            }) {
                anyhow::bail!(
                    "Shell metacharacter detected in docker command argument: {}",
                    word
                );
            }
        }

        // Insert --name and -d flags before the image name.
        // Docker syntax: docker run [OPTIONS] IMAGE [COMMAND] [ARG...]
        // We need to find where "run" is, then insert our flags right after
        // the user-supplied options but before the image name.
        // Strategy: find "run" in args, then find the first positional arg
        // (not starting with '-' and not a value for a preceding flag) — that's the image.
        let mut args: Vec<String> = words[1..].to_vec();

        // Find image position: skip flags and their values after "run"
        let run_idx = args.iter().position(|w| w == "run");
        let search_start = run_idx.map_or(0, |i| i + 1);
        let mut i = search_start;
        while i < args.len() {
            let arg = &args[i];
            if arg.starts_with('-') {
                // Flags that take a value (next arg is consumed)
                let takes_value = matches!(
                    arg.as_str(),
                    "-p" | "--publish"
                        | "-v"
                        | "--volume"
                        | "-e"
                        | "--env"
                        | "--name"
                        | "--gpus"
                        | "--network"
                        | "--platform"
                        | "-w"
                        | "--workdir"
                        | "-u"
                        | "--user"
                        | "--entrypoint"
                        | "--mount"
                        | "-l"
                        | "--label"
                        | "--memory"
                        | "-m"
                );
                if takes_value && !arg.contains('=') {
                    i += 2; // skip flag + value
                } else {
                    i += 1; // boolean flag or --flag=value
                }
            } else {
                // First positional arg = image name
                break;
            }
        }

        // Insert our flags right before the image name
        args.insert(i, "-d".to_string());
        args.insert(i, DOCKER_CONTAINER_NAME.to_string());
        args.insert(i, "--name".to_string());

        let child = std::process::Command::new(&words[0])
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to spawn docker command")?;

        Ok(child)
    }

    fn launch_local_voicevox(path: &str) -> anyhow::Result<std::process::Child> {
        let words = shell_words::split(path)
            .map_err(|e| anyhow::anyhow!("Failed to parse voicevox command: {}", e))?;

        if words.is_empty() {
            anyhow::bail!("Empty voicevox command");
        }

        // Reject shell metacharacters
        for word in &words {
            if word.chars().any(|c| {
                matches!(
                    c,
                    ';' | '|' | '&' | '$' | '`' | '(' | ')' | '{' | '}' | '<' | '>'
                )
            }) {
                anyhow::bail!("Shell metacharacter detected in voicevox command: {}", word);
            }
        }

        if words.len() != 1 {
            anyhow::bail!(
                "Local VOICEVOX launch accepts executable path only (no arguments allowed)"
            );
        }

        let child = std::process::Command::new(&words[0])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to spawn voicevox process")?;

        Ok(child)
    }
}

impl Drop for ZunduxApp {
    fn drop(&mut self) {
        self.desktop_capture.stop_capture();
        if self.is_docker {
            // Graceful: try stop first (sends SIGTERM to container)
            let _ = std::process::Command::new("docker")
                .args(["stop", "-t", "5", DOCKER_CONTAINER_NAME])
                .status();
        }
        if let Some(ref mut child) = self.voicevox_process {
            // Try graceful SIGTERM first, then force kill after 5 seconds
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .status();
            match child.try_wait() {
                Ok(Some(_)) => {}
                _ => {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}

impl eframe::App for ZunduxApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Transparent clear color for the window
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_messages();
        self.state.is_playing = self.is_playing.load(Ordering::SeqCst);
        self.state.is_soundboard_playing = self.is_soundboard_playing.load(Ordering::SeqCst);
        self.periodic_health_check();
        self.process_actions();

        let theme = &self.state.config.theme;

        // Apply theme visuals when needed
        if self.state.needs_theme_update {
            self.needs_theme_update = true;
            self.state.needs_theme_update = false;
        }
        if self.needs_theme_update {
            ctx.set_visuals(theme.to_visuals());
            ctx.set_style(theme.to_style());
            self.needs_theme_update = false;
        }

        // Paint window background with rounded rect
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let rounding = if is_maximized {
            0.0
        } else {
            theme.window_rounding
        };
        let screen_rect = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(
            screen_rect,
            egui::CornerRadius::same(rounding as u8),
            theme.color(theme.window_background),
        );

        // Custom title bar
        crate::ui::titlebar::show(ctx, theme);

        // Keyboard shortcuts
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::F4)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Tab panel
        egui::TopBottomPanel::top("tabs")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::symmetric(
                        theme.spacing_medium as i8,
                        theme.spacing_small as i8,
                    )),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (screen, label) in [
                        (Screen::Input, "Input"),
                        (Screen::Soundboard, "Soundboard"),
                        (Screen::Settings, "Settings"),
                    ] {
                        let is_active = self.state.current_screen == screen;
                        let bg = if is_active {
                            theme.color(theme.tab_active_background)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        let text_color = if is_active {
                            theme.color(theme.text_primary)
                        } else {
                            theme.color(theme.text_muted)
                        };
                        let btn = ui.add(
                            egui::Button::new(
                                egui::RichText::new(label).size(11.0).color(text_color),
                            )
                            .fill(bg)
                            .corner_radius(egui::CornerRadius::same(theme.tab_rounding as u8)),
                        );
                        if btn.clicked() {
                            self.state.current_screen = screen;
                        }
                    }
                });
            });

        // Status bar (bottom)
        let theme = &self.state.config.theme;
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .frame(egui::Frame::NONE.fill(theme.color(theme.titlebar_background)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let (vox_color, vox_text) = if self.state.voicevox_connected {
                        (theme.color(theme.status_ok), "VOICEVOX")
                    } else if self.state.voicevox_launching {
                        (theme.color(theme.status_warn), "VOICEVOX...")
                    } else {
                        (theme.color(theme.status_error), "VOICEVOX")
                    };
                    ui.colored_label(vox_color, format!("\u{25CF} {}", vox_text));
                    ui.add_space(12.0);
                    let (mic_color, mic_text) = if self.state.device_ready {
                        (theme.color(theme.status_ok), "Virtual Mic")
                    } else {
                        (theme.color(theme.status_warn), "Virtual Mic")
                    };
                    ui.colored_label(mic_color, format!("\u{25CF} {}", mic_text));
                    if let Some(ref error) = self.state.last_error.clone() {
                        ui.add_space(12.0);
                        let error_label = ui.colored_label(
                            theme.color(theme.status_error),
                            error.chars().take(60).collect::<String>(),
                        );
                        self.state.error_hovered = error_label.hovered();
                        if error_label.clicked() {
                            self.state.last_error = None;
                            self.state.error_display_time = None;
                        }
                    }

                    // Resize handle at bottom-right corner
                    let available = ui.available_size();
                    ui.add_space(available.x - 16.0);
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::drag());
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "\u{2921}",
                        egui::FontId::proportional(12.0),
                        theme.color(theme.text_muted),
                    );
                    if response.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                            egui::ResizeDirection::SouthEast,
                        ));
                    }
                });
            });

        // Toast auto-dismiss (5 seconds, paused on hover)
        if let Some(time) = self.state.error_display_time {
            if !self.state.error_hovered && time.elapsed() > std::time::Duration::from_secs(5) {
                self.state.last_error = None;
                self.state.error_display_time = None;
            }
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ctx, |ui| match self.state.current_screen {
                Screen::Input => crate::ui::input::show(ui, &mut self.state),
                Screen::Soundboard => crate::ui::soundboard::show(ui, &mut self.state),
                Screen::Settings => crate::ui::settings::show(ui, &mut self.state),
            });

        // Keep repainting for periodic health checks and spinner
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_shell_metacharacters_in_docker_cmd() {
        let result =
            ZunduxApp::launch_docker_voicevox("docker run evil;rm -rf /", "http://127.0.0.1:50021");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metacharacter"));
    }

    #[test]
    fn rejects_shell_metacharacters_in_local_cmd() {
        let result = ZunduxApp::launch_local_voicevox("voicevox && rm -rf /");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_local_cmd_with_arguments() {
        let result = ZunduxApp::launch_local_voicevox("voicevox --host 0.0.0.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no arguments allowed"));
    }
}
