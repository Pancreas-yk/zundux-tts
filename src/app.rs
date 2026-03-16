use crate::audio::AudioManager;
use crate::config::AppConfig;
use crate::tts::types::{Speaker, SynthParams};
use crate::tts::voicevox::VoicevoxEngine;
use crate::tts::TtsManager;
use crate::ui::Screen;

use std::process::{Child, Command};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    Synthesize { text: String, params: SynthParams },
    LoadSpeakers,
    HealthCheck,
    LoadUserDict,
    AddUserDictWord { surface: String, pronunciation: String },
    DeleteUserDictWord { uuid: String },
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
    pub media_url: String,
    pub media_playing: bool,
    pub pending_media_play: Option<String>,
    pub pending_media_stop: bool,
    pub media_deps_checked: bool,
    pub media_has_ytdlp: bool,
    pub media_has_ffmpeg: bool,
}

const DOCKER_CONTAINER_NAME: &str = "zundamon-voicevox";

pub struct ZundamonApp {
    state: AppState,
    audio_manager: AudioManager,
    ui_rx: mpsc::Receiver<UiMessage>,
    tts_tx: mpsc::Sender<TtsCommand>,
    voicevox_process: Option<Child>,
    is_docker: bool,
    last_health_check: Instant,
    pub is_playing: Arc<AtomicBool>,
    url_player: crate::media::url_player::UrlPlayer,
}

const HEALTH_CHECK_INTERVAL_SECS: u64 = 5;
const HEALTH_CHECK_INTERVAL_LAUNCHING_SECS: u64 = 1;

impl ZundamonApp {
    pub fn new(
        config: AppConfig,
        tts_manager: TtsManager,
        rt: tokio::runtime::Handle,
    ) -> Self {
        let (ui_tx, ui_rx) = mpsc::channel::<UiMessage>();
        let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();

        let auto_launch_voicevox = config.auto_launch_voicevox;
        let device_name = config.virtual_device_name.clone();
        let mut audio_manager = AudioManager::new(&device_name);
        let device_ready = audio_manager.ensure_device().is_ok()
            && audio_manager.device_exists().unwrap_or(false);

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
                media_url: String::new(),
                media_playing: false,
                pending_media_play: None,
                pending_media_stop: false,
                media_deps_checked: false,
                media_has_ytdlp: false,
                media_has_ffmpeg: false,
            },
            audio_manager,
            ui_rx,
            tts_tx,
            voicevox_process: None,
            is_docker: false,
            last_health_check: Instant::now(),
            is_playing: Arc::new(AtomicBool::new(false)),
            url_player: crate::media::url_player::UrlPlayer::new(),
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
                        let _ = tx.send(UiMessage::Error(format!(
                            "スピーカー取得失敗: {}",
                            e
                        )));
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
                TtsCommand::LoadUserDict => {
                    match dict_engine.list_user_dict().await {
                        Ok(dict) => { let _ = tx.send(UiMessage::UserDictLoaded(dict)); }
                        Err(e) => { let _ = tx.send(UiMessage::Error(format!("辞書取得失敗: {}", e))); }
                    }
                }
                TtsCommand::AddUserDictWord { surface, pronunciation } => {
                    match dict_engine.add_user_dict_word(&surface, &pronunciation).await {
                        Ok(_) => { let _ = tx.send(UiMessage::UserDictUpdated); }
                        Err(e) => { let _ = tx.send(UiMessage::Error(format!("辞書登録失敗: {}", e))); }
                    }
                }
                TtsCommand::DeleteUserDictWord { uuid } => {
                    match dict_engine.delete_user_dict_word(&uuid).await {
                        Ok(()) => { let _ = tx.send(UiMessage::UserDictUpdated); }
                        Err(e) => { let _ = tx.send(UiMessage::Error(format!("辞書削除失敗: {}", e))); }
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
                        let device_name = self.state.config.virtual_device_name.clone();
                        let monitor = self.state.config.monitor_audio;
                        let playing = self.is_playing.clone();
                        playing.store(true, Ordering::SeqCst);
                        std::thread::spawn(move || {
                            let _guard = PlaybackGuard(playing);
                            if let Err(e) = crate::audio::playback::play_wav(wav, &device_name, monitor) {
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
        // Handle text send
        if let Some(text) = self.state.pending_send.take() {
            let params = SynthParams::from_config(&self.state.config);
            self.state.is_synthesizing = true;
            self.state.last_error = None;
            let _ = self.tts_tx.send(TtsCommand::Synthesize { text, params });
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
            let _ = self.tts_tx.send(TtsCommand::AddUserDictWord { surface, pronunciation });
        }
        if let Some(uuid) = self.state.pending_delete_dict_word.take() {
            let _ = self.tts_tx.send(TtsCommand::DeleteUserDictWord { uuid });
        }

        if self.state.pending_soundboard_scan {
            self.state.pending_soundboard_scan = false;
            let path = std::path::Path::new(&self.state.config.soundboard_path);
            let mut files = Vec::new();
            if path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                            if matches!(ext.to_lowercase().as_str(), "wav" | "mp3" | "ogg") {
                                let name = p.file_stem()
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

        if let Some(file_path) = self.state.pending_soundboard_play.take() {
            if self.is_playing.load(Ordering::SeqCst) {
                self.state.last_error = Some("再生中です...".to_string());
            } else {
                let device_name = self.state.config.virtual_device_name.clone();
                let monitor = self.state.config.monitor_audio;
                let playing = self.is_playing.clone();
                playing.store(true, Ordering::SeqCst);
                std::thread::spawn(move || {
                    let _guard = PlaybackGuard(playing);
                    match std::fs::read(&file_path) {
                        Ok(data) => {
                            if let Err(e) = crate::audio::playback::play_wav(data, &device_name, monitor) {
                                tracing::error!("Soundboard playback error: {}", e);
                            }
                        }
                        Err(e) => tracing::error!("Failed to read soundboard file: {}", e),
                    }
                });
            }
        }

        // Check media dependencies
        if !self.state.media_deps_checked {
            self.state.media_deps_checked = true;
            let (ytdlp, ffmpeg) = crate::media::url_player::UrlPlayer::check_dependencies();
            self.state.media_has_ytdlp = ytdlp;
            self.state.media_has_ffmpeg = ffmpeg;
        }

        // Poll media process for natural completion
        if self.state.media_playing && self.url_player.poll_finished() {
            self.state.media_playing = false;
            self.is_playing.store(false, Ordering::SeqCst);
        }

        // Media URL playback
        if let Some(url) = self.state.pending_media_play.take() {
            if self.is_playing.load(Ordering::SeqCst) {
                self.state.last_error = Some("再生中です...".to_string());
            } else {
                let device_name = &self.state.config.virtual_device_name;
                match self.url_player.play(&url, device_name) {
                    Ok(()) => {
                        self.state.media_playing = true;
                        self.is_playing.store(true, Ordering::SeqCst);
                        self.state.last_error = None;
                    }
                    Err(e) => {
                        self.state.last_error = Some(format!("メディア再生失敗: {}", e));
                    }
                }
            }
        }

        if self.state.pending_media_stop {
            self.state.pending_media_stop = false;
            self.url_player.stop();
            self.state.media_playing = false;
            self.is_playing.store(false, Ordering::SeqCst);
        }
    }

    fn is_docker_command(cmd: &str) -> bool {
        let trimmed = cmd.trim_start();
        trimmed.starts_with("docker ") || trimmed.starts_with("docker run")
    }

    fn cleanup_docker_container() {
        let _ = Command::new("docker")
            .args(["rm", "-f", DOCKER_CONTAINER_NAME])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    fn stop_docker_container() {
        let _ = Command::new("docker")
            .args(["stop", DOCKER_CONTAINER_NAME])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    fn launch_voicevox(&mut self) {
        // Kill existing process if any
        if let Some(ref mut child) = self.voicevox_process {
            let _ = child.kill();
            let _ = child.wait();
        }
        if self.is_docker {
            Self::cleanup_docker_container();
        }

        let path = &self.state.config.voicevox_path;
        let is_docker = Self::is_docker_command(path);

        // Determine how to launch based on path content
        let result = if is_docker {
            // Remove stale container with the same name
            Self::cleanup_docker_container();
            // Inject --name into the docker command
            let docker_cmd =
                path.replacen("docker run", &format!("docker run --name {DOCKER_CONTAINER_NAME}"), 1);
            Command::new("sh")
                .args(["-c", &docker_cmd])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        } else if path.contains(' ') {
            // Command with arguments: run via sh
            Command::new("sh")
                .args(["-c", path])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        } else {
            // Simple executable path
            Command::new(path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        };

        match result {
            Ok(child) => {
                self.voicevox_process = Some(child);
                self.is_docker = is_docker;
                self.state.last_error = None;
                self.state.voicevox_launching = true;
                tracing::info!("Launched VOICEVOX: {}", path);
            }
            Err(e) => {
                self.state.last_error = Some(format!(
                    "VOICEVOX起動失敗: {}\nパスを設定画面で確認してください",
                    e
                ));
                tracing::error!("Failed to launch VOICEVOX: {}", e);
            }
        }
    }
}

impl Drop for ZundamonApp {
    fn drop(&mut self) {
        if self.is_docker {
            Self::stop_docker_container();
        }
        if let Some(ref mut child) = self.voicevox_process {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl eframe::App for ZundamonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_messages();
        self.state.is_playing = self.is_playing.load(Ordering::SeqCst);
        self.periodic_health_check();
        self.process_actions();

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.state.current_screen, Screen::Input, "入力");
                ui.selectable_value(&mut self.state.current_screen, Screen::Soundboard, "サウンドボード");
                ui.selectable_value(&mut self.state.current_screen, Screen::Media, "メディア");
                ui.selectable_value(&mut self.state.current_screen, Screen::Settings, "設定");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.state.current_screen {
            Screen::Input => crate::ui::input::show(ui, &mut self.state),
            Screen::Soundboard => crate::ui::soundboard::show(ui, &mut self.state),
            Screen::Media => crate::ui::media::show(ui, &mut self.state),
            Screen::Settings => crate::ui::settings::show(ui, &mut self.state),
        });

        // Keep repainting for periodic health checks and spinner
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}
