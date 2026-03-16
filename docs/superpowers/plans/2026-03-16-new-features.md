# New Features Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add OSC chatbox, user dictionary, soundboard, media audio routing, and echo effect to zundamon_vrc.

**Architecture:** Five features layered on the existing egui/tokio/PulseAudio app. A shared `AtomicBool` playback lock coordinates exclusive audio access across TTS, soundboard, and media. New tabs (Soundboard, Media) join existing Input/Settings. OSC and echo hook into the existing TTS→playback pipeline. User dictionary uses VOICEVOX's HTTP API directly.

**Tech Stack:** Rust, egui/eframe, tokio, PulseAudio (pactl/paplay), rosc (OSC), yt-dlp + ffmpeg (runtime), rodio with mp3/vorbis features.

**Spec:** `docs/superpowers/specs/2026-03-16-new-features-design.md`

---

## Chunk 1: Foundation — Config, Playback Lock, Tab Scaffold

### Task 1: Add new config fields

**Files:**
- Modify: `src/config.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add new fields to `AppConfig` in `src/config.rs`**

Add these fields after the existing `templates` field:

```rust
// In AppConfig struct, after templates:
pub osc_enabled: bool,
pub osc_address: String,
pub osc_port: u16,
pub soundboard_path: String,
pub echo_enabled: bool,
pub echo_delay_ms: u32,
pub echo_decay: f64,
```

Update `Default for AppConfig` to include defaults:

```rust
osc_enabled: false,
osc_address: "127.0.0.1".to_string(),
osc_port: 9000,
soundboard_path: ProjectDirs::from("", "", "zundamon_vrc")
    .map(|d| d.config_dir().join("sounds").to_string_lossy().to_string())
    .unwrap_or_else(|| "sounds".to_string()),
echo_enabled: false,
echo_delay_ms: 200,
echo_decay: 0.4,
```

- [ ] **Step 2: Add `rosc` dependency to `Cargo.toml` and enable rodio features**

Change the rodio line and add rosc:

```toml
rodio = { version = "0.20", features = ["mp3", "vorbis"] }
rosc = "0.10"
```

- [ ] **Step 3: Run `cargo check` to verify**

Run: `cargo check`
Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs Cargo.toml Cargo.lock
git commit -m "feat: add config fields for OSC, soundboard, and echo"
```

---

### Task 2: Add playback lock (`is_playing` AtomicBool)

**Files:**
- Modify: `src/app.rs`
- Modify: `src/audio/playback.rs`

- [ ] **Step 1: Add `Arc<AtomicBool>` to `ZundamonApp` and pass to playback**

In `src/app.rs`, add to imports:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
```

Add field to `ZundamonApp`:

```rust
pub is_playing: Arc<AtomicBool>,
```

Initialize in `ZundamonApp::new()`:

```rust
is_playing: Arc::new(AtomicBool::new(false)),
```

Also add `is_playing: bool` to `AppState` (for UI display):

```rust
pub is_playing: bool,
```

Initialize as `false` in `AppState` init block.

- [ ] **Step 2: Use the lock in `process_messages()` WavReady handler**

Replace the current `WavReady` handler in `process_messages()`:

```rust
UiMessage::WavReady(wav) => {
    self.state.is_synthesizing = false;
    self.state.last_error = None;
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
```

Add a `PlaybackGuard` struct in `src/app.rs`:

```rust
struct PlaybackGuard(Arc<AtomicBool>);
impl Drop for PlaybackGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
```

- [ ] **Step 3: Update `is_playing` state each frame for UI**

In `update()` method, after `self.process_messages()`:

```rust
self.state.is_playing = self.is_playing.load(Ordering::SeqCst);
```

- [ ] **Step 4: Run `cargo check`**

Run: `cargo check`
Expected: Compiles. (The `is_playing` field on AppState may show unused warning — that's fine, it'll be used by UI later.)

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: add AtomicBool playback lock for exclusive audio"
```

---

### Task 3: Add new Screen variants and tab bar

**Files:**
- Modify: `src/ui/mod.rs`
- Create: `src/ui/soundboard.rs`
- Create: `src/ui/media.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add Screen variants**

In `src/ui/mod.rs`:

```rust
pub mod input;
pub mod media;
pub mod settings;
pub mod soundboard;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Input,
    Soundboard,
    Media,
    Settings,
}
```

- [ ] **Step 2: Create stub `src/ui/soundboard.rs`**

```rust
use crate::app::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("サウンドボード");
    ui.label("準備中...");
}
```

- [ ] **Step 3: Create stub `src/ui/media.rs`**

```rust
use crate::app::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("メディア");
    ui.label("準備中...");
}
```

- [ ] **Step 4: Update tab bar and central panel in `src/app.rs`**

In the `update()` method, update the tab bar:

```rust
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
```

- [ ] **Step 5: Run `cargo check`**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add src/ui/mod.rs src/ui/soundboard.rs src/ui/media.rs src/app.rs
git commit -m "feat: add Soundboard and Media tab stubs"
```

---

## Chunk 2: OSC Chatbox + Echo Effect

### Task 4: Implement OSC chatbox sender

**Files:**
- Create: `src/osc.rs`
- Modify: `src/main.rs` (add `mod osc`)
- Modify: `src/app.rs`

- [ ] **Step 1: Create `src/osc.rs`**

```rust
use anyhow::Result;
use rosc::{OscMessage, OscPacket, OscType};
use std::net::UdpSocket;

pub fn send_chatbox(address: &str, port: u16, text: &str) -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let msg = OscMessage {
        addr: "/chatbox/input".to_string(),
        args: vec![
            OscType::String(text.to_string()),
            OscType::Bool(true),  // immediate display
            OscType::Bool(false), // no sound notification
        ],
    };
    let packet = OscPacket::Message(msg);
    let buf = rosc::encoder::encode(&packet)?;
    socket.send_to(&buf, format!("{}:{}", address, port))?;
    Ok(())
}
```

- [ ] **Step 2: Add `mod osc;` to `src/main.rs`**

Add after other module declarations:

```rust
mod osc;
```

- [ ] **Step 3: Integrate OSC into `WavReady` handler in `src/app.rs`**

The `WavReady` variant needs to carry the original text. Change the enum:

```rust
enum UiMessage {
    WavReady { wav: Vec<u8>, text: String },
    // ... other variants unchanged
}
```

Update `tts_loop` to include text in `WavReady`:

```rust
TtsCommand::Synthesize { text, params } => {
    match tts.synthesize(&text, &params).await {
        Ok(wav) => {
            let _ = tx.send(UiMessage::WavReady { wav, text });
        }
        // ...
    }
}
```

Update `process_messages()` `WavReady` handler to send OSC before playback:

```rust
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
```

- [ ] **Step 4: Add OSC settings section to `src/ui/settings.rs`**

Add after the "オーディオ" section:

```rust
ui.add_space(8.0);

ui.collapsing("OSC設定", |ui| {
    ui.checkbox(&mut state.config.osc_enabled, "OSCチャットボックスを有効化");
    ui.label("VRChatのチャットボックスにテキストを表示します");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("送信先アドレス:");
        ui.text_edit_singleline(&mut state.config.osc_address);
    });
    ui.horizontal(|ui| {
        ui.label("ポート:");
        let mut port_str = state.config.osc_port.to_string();
        if ui.text_edit_singleline(&mut port_str).changed() {
            if let Ok(p) = port_str.parse::<u16>() {
                state.config.osc_port = p;
            }
        }
    });
});
```

- [ ] **Step 5: Run `cargo check`**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 6: Run `cargo build` to verify full build**

Run: `cargo build`
Expected: Builds successfully.

- [ ] **Step 7: Commit**

```bash
git add src/osc.rs src/main.rs src/app.rs src/ui/settings.rs
git commit -m "feat: add OSC chatbox integration for VRChat"
```

---

### Task 5: Implement echo effect

**Files:**
- Create: `src/audio/effects.rs`
- Modify: `src/audio/mod.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Create `src/audio/effects.rs`**

```rust
/// Apply echo effect to WAV data.
/// Expects standard WAV format (RIFF header).
/// Returns new WAV bytes with echo applied.
pub fn apply_echo(wav_data: &[u8], delay_ms: u32, decay: f64) -> Vec<u8> {
    // Need at least a WAV header
    if wav_data.len() <= 44 || &wav_data[0..4] != b"RIFF" {
        return wav_data.to_vec();
    }

    // Read sample rate from WAV header (bytes 24-27, little-endian u32)
    let sample_rate = u32::from_le_bytes([
        wav_data[24], wav_data[25], wav_data[26], wav_data[27],
    ]);
    // Read bits per sample (bytes 34-35, little-endian u16)
    let bits_per_sample = u16::from_le_bytes([wav_data[34], wav_data[35]]);

    if bits_per_sample != 16 {
        // Only support 16-bit for now
        return wav_data.to_vec();
    }

    let header = &wav_data[..44];
    let pcm_data = &wav_data[44..];

    // Convert bytes to i16 samples
    let mut samples: Vec<i16> = pcm_data
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    let delay_samples = (sample_rate as usize * delay_ms as usize) / 1000;

    // Apply echo: add delayed, decayed copy
    for i in delay_samples..samples.len() {
        let echo = (samples[i - delay_samples] as f64 * decay) as i64;
        let mixed = samples[i] as i64 + echo;
        // Clamp to i16 range
        samples[i] = mixed.clamp(i16::MIN as i64, i16::MAX as i64) as i16;
    }

    // Reconstruct WAV
    let mut result = header.to_vec();
    for sample in &samples {
        result.extend_from_slice(&sample.to_le_bytes());
    }

    // Update data chunk size (bytes 40-43) and RIFF size (bytes 4-7)
    let data_size = (samples.len() * 2) as u32;
    result[40..44].copy_from_slice(&data_size.to_le_bytes());
    let riff_size = (result.len() - 8) as u32;
    result[4..8].copy_from_slice(&riff_size.to_le_bytes());

    result
}
```

- [ ] **Step 2: Register the module in `src/audio/mod.rs`**

Add after existing module declarations:

```rust
pub mod effects;
```

- [ ] **Step 3: Apply echo in `process_messages()` WavReady handler in `src/app.rs`**

In the `WavReady` handler, after OSC send and before playback, apply echo:

```rust
// Apply echo effect if enabled
let wav = if self.state.config.echo_enabled {
    crate::audio::effects::apply_echo(
        &wav,
        self.state.config.echo_delay_ms,
        self.state.config.echo_decay,
    )
} else {
    wav
};
```

- [ ] **Step 4: Add echo settings to `src/ui/settings.rs`**

Add after the OSC settings section:

```rust
ui.add_space(8.0);

ui.collapsing("音声エフェクト", |ui| {
    ui.checkbox(&mut state.config.echo_enabled, "エコーを有効化");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("遅延(ms):");
        ui.add(
            egui::Slider::new(&mut state.config.echo_delay_ms, 50..=500)
                .step_by(10.0),
        );
    });
    ui.horizontal(|ui| {
        ui.label("減衰:");
        ui.add(
            egui::Slider::new(&mut state.config.echo_decay, 0.1..=0.8)
                .step_by(0.05),
        );
    });
});
```

- [ ] **Step 5: Run `cargo check`**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add src/audio/effects.rs src/audio/mod.rs src/app.rs src/ui/settings.rs
git commit -m "feat: add echo audio effect for TTS output"
```

---

## Chunk 3: User Dictionary

### Task 6: Add user dictionary API methods to VoicevoxEngine

**Files:**
- Modify: `src/tts/voicevox.rs`
- Modify: `src/tts/types.rs`

- [ ] **Step 1: Add `UserDictWord` type to `src/tts/types.rs`**

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDictWord {
    pub surface: String,
    pub pronunciation: String,
    pub accent_type: u32,
    // VOICEVOX returns additional fields we don't need to display
}

/// The VOICEVOX /user_dict endpoint returns a map of UUID → word
pub type UserDict = HashMap<String, UserDictWord>;
```

- [ ] **Step 2: Add dictionary methods to `VoicevoxEngine` in `src/tts/voicevox.rs`**

Add these inherent methods (not on the trait):

```rust
impl VoicevoxEngine {
    // ... existing new() method ...

    pub async fn list_user_dict(&self) -> Result<UserDict> {
        let url = format!("{}/user_dict", self.base_url);
        let resp = self.client.get(&url).send().await
            .context("Failed to fetch user dictionary")?;
        let dict: UserDict = resp.json().await
            .context("Failed to parse user dictionary")?;
        Ok(dict)
    }

    pub async fn add_user_dict_word(&self, surface: &str, pronunciation: &str) -> Result<String> {
        let url = format!("{}/user_dict", self.base_url);
        let resp = self.client.post(&url)
            .query(&[
                ("surface", surface),
                ("pronunciation", pronunciation),
                ("accent_type", "1"),
            ])
            .send().await
            .context("Failed to add dictionary word")?;
        let uuid: String = resp.json().await
            .context("Failed to parse add word response")?;
        Ok(uuid)
    }

    pub async fn delete_user_dict_word(&self, word_uuid: &str) -> Result<()> {
        let url = format!("{}/user_dict/{}", self.base_url, word_uuid);
        self.client.delete(&url).send().await
            .context("Failed to delete dictionary word")?;
        Ok(())
    }
}
```

- [ ] **Step 3: Run `cargo check`**

Run: `cargo check`
Expected: Compiles (methods are unused for now — that's fine).

- [ ] **Step 4: Commit**

```bash
git add src/tts/voicevox.rs src/tts/types.rs
git commit -m "feat: add VOICEVOX user dictionary API methods"
```

---

### Task 7: Wire user dictionary through commands and UI

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui/settings.rs`
- Modify: `src/tts/voicevox.rs`

- [ ] **Step 1: Add dictionary command/message variants to `src/app.rs`**

Add to `TtsCommand`:

```rust
pub enum TtsCommand {
    Synthesize { text: String, params: SynthParams },
    LoadSpeakers,
    HealthCheck,
    LoadUserDict,
    AddUserDictWord { surface: String, pronunciation: String },
    DeleteUserDictWord { uuid: String },
}
```

Add to `UiMessage`:

```rust
enum UiMessage {
    WavReady { wav: Vec<u8>, text: String },
    SpeakersLoaded(Vec<Speaker>),
    HealthCheckResult(bool),
    UserDictLoaded(crate::tts::types::UserDict),
    UserDictUpdated,
    Error(String),
}
```

Add dictionary state fields to `AppState`:

```rust
pub user_dict: Vec<(String, String, String)>, // (uuid, surface, pronunciation)
pub new_dict_surface: String,
pub new_dict_pronunciation: String,
pub pending_load_user_dict: bool,
pub pending_add_dict_word: Option<(String, String)>,
pub pending_delete_dict_word: Option<String>,
```

Initialize all as empty/false/None in `AppState` init.

- [ ] **Step 2: Change `tts_loop` to accept `VoicevoxEngine` directly**

The `tts_loop` needs access to `VoicevoxEngine` for dictionary operations. Change its signature:

```rust
async fn tts_loop(
    tts: TtsManager,
    voicevox: VoicevoxEngine,
    rx: mpsc::Receiver<TtsCommand>,
    tx: mpsc::Sender<UiMessage>,
)
```

In `ZundamonApp::new()`, create the engine and manager separately so we can pass both:

```rust
// In main.rs, pass engine separately or clone URL
// In app.rs new(), accept voicevox_url: String
```

Actually, since `VoicevoxEngine` holds a `reqwest::Client` and a `String`, the simplest approach: create a second `VoicevoxEngine` instance for dictionary operations in `tts_loop`. Pass `voicevox_url` to `tts_loop`.

**Important:** Add this import to the top of `src/app.rs`:

```rust
use crate::tts::voicevox::VoicevoxEngine;
```

Then update `tts_loop`:

```rust
async fn tts_loop(
    tts: TtsManager,
    voicevox_url: String,
    rx: mpsc::Receiver<TtsCommand>,
    tx: mpsc::Sender<UiMessage>,
) {
    let dict_engine = VoicevoxEngine::new(&voicevox_url);
    while let Ok(cmd) = rx.recv() {
        match cmd {
            // ... existing handlers ...
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
```

Update `ZundamonApp::new()` to pass `voicevox_url`:

```rust
let voicevox_url = config.voicevox_url.clone();
rt.spawn(Self::tts_loop(tts_manager, voicevox_url, tts_rx, ui_tx));
```

- [ ] **Step 3: Handle dictionary messages in `process_messages()`**

```rust
UiMessage::UserDictLoaded(dict) => {
    self.state.user_dict = dict
        .into_iter()
        .map(|(uuid, word)| (uuid, word.surface, word.pronunciation))
        .collect();
    self.state.user_dict.sort_by(|a, b| a.1.cmp(&b.1));
}
UiMessage::UserDictUpdated => {
    // Reload dict after add/delete
    let _ = self.tts_tx.send(TtsCommand::LoadUserDict);
}
```

- [ ] **Step 4: Process dictionary actions in `process_actions()`**

```rust
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
```

- [ ] **Step 5: Add dictionary UI section to `src/ui/settings.rs`**

Add after "スピーカー選択" section:

```rust
ui.add_space(8.0);

ui.collapsing("ユーザー辞書", |ui| {
    if state.voicevox_connected {
        // Load button
        if ui.button("辞書を読み込む").clicked() {
            state.pending_load_user_dict = true;
        }
        ui.add_space(4.0);

        // Word list
        let mut to_delete = None;
        for (uuid, surface, pronunciation) in &state.user_dict {
            ui.horizontal(|ui| {
                ui.label(format!("{} → {}", surface, pronunciation));
                if ui.small_button("削除").clicked() {
                    to_delete = Some(uuid.clone());
                }
            });
        }
        if let Some(uuid) = to_delete {
            state.pending_delete_dict_word = Some(uuid);
        }

        ui.add_space(4.0);
        ui.separator();

        // Add new word
        ui.horizontal(|ui| {
            ui.label("表記:");
            ui.add(
                egui::TextEdit::singleline(&mut state.new_dict_surface)
                    .desired_width(100.0),
            );
            ui.label("読み:");
            ui.add(
                egui::TextEdit::singleline(&mut state.new_dict_pronunciation)
                    .desired_width(100.0),
            );
            if ui.button("追加").clicked()
                && !state.new_dict_surface.trim().is_empty()
                && !state.new_dict_pronunciation.trim().is_empty()
            {
                state.pending_add_dict_word = Some((
                    state.new_dict_surface.trim().to_string(),
                    state.new_dict_pronunciation.trim().to_string(),
                ));
                state.new_dict_surface.clear();
                state.new_dict_pronunciation.clear();
            }
        });
    } else {
        ui.label("VOICEVOXに接続してください");
    }
});
```

- [ ] **Step 6: Run `cargo check`**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 7: Commit**

```bash
git add src/app.rs src/ui/settings.rs
git commit -m "feat: add user dictionary management UI"
```

---

## Chunk 4: Soundboard

### Task 8: Implement soundboard file scanning and playback

**Files:**
- Modify: `src/ui/soundboard.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add soundboard state to `AppState` in `src/app.rs`**

```rust
pub soundboard_files: Vec<(String, std::path::PathBuf)>, // (display_name, full_path)
pub pending_soundboard_scan: bool,
pub pending_soundboard_play: Option<std::path::PathBuf>,
```

Initialize: `soundboard_files: Vec::new()`, `pending_soundboard_scan: true` (auto-scan on startup), `pending_soundboard_play: None`.

- [ ] **Step 2: Implement folder scanning in `process_actions()`**

```rust
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
        // Create directory if it doesn't exist
        let _ = std::fs::create_dir_all(path);
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    self.state.soundboard_files = files;
}
```

- [ ] **Step 3: Implement soundboard playback in `process_actions()`**

```rust
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
                    // play_wav uses rodio first (supports WAV/MP3/OGG with feature flags).
                    // The paplay fallback only supports WAV, so non-WAV files will only
                    // work via rodio. This is acceptable since rodio handles most cases.
                    if let Err(e) = crate::audio::playback::play_wav(data, &device_name, monitor) {
                        tracing::error!("Soundboard playback error: {}", e);
                    }
                }
                Err(e) => tracing::error!("Failed to read soundboard file: {}", e),
            }
        });
    }
}
```

- [ ] **Step 4: Implement soundboard UI in `src/ui/soundboard.rs`**

```rust
use crate::app::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("サウンドボード");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label(format!("フォルダ: {}", state.config.soundboard_path));
        if ui.button("再スキャン").clicked() {
            state.pending_soundboard_scan = true;
        }
    });

    ui.add_space(8.0);

    if state.soundboard_files.is_empty() {
        ui.label("音声ファイルが見つかりません。");
        ui.label(format!(
            "フォルダにWAV/MP3/OGGファイルを配置してください: {}",
            state.config.soundboard_path
        ));
    } else {
        let cols = 3;
        egui::Grid::new("soundboard_grid")
            .num_columns(cols)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                let files = state.soundboard_files.clone();
                for (i, (name, path)) in files.iter().enumerate() {
                    let enabled = !state.is_playing && !state.is_synthesizing;
                    if ui.add_enabled(enabled, egui::Button::new(name)).clicked() {
                        state.pending_soundboard_play = Some(path.clone());
                    }
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    if state.is_playing {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("再生中...");
        });
    }
}
```

- [ ] **Step 5: Add soundboard path setting to `src/ui/settings.rs`**

Add a new collapsing section after the virtual device section:

```rust
ui.add_space(8.0);

ui.collapsing("サウンドボード", |ui| {
    ui.horizontal(|ui| {
        ui.label("フォルダパス:");
        ui.text_edit_singleline(&mut state.config.soundboard_path);
        if ui.button("参照").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                state.config.soundboard_path = path.to_string_lossy().to_string();
                state.pending_soundboard_scan = true;
            }
        }
    });
});
```

- [ ] **Step 6: Run `cargo check`**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 7: Run `cargo build`**

Run: `cargo build`
Expected: Builds successfully.

- [ ] **Step 8: Commit**

```bash
git add src/app.rs src/ui/soundboard.rs src/ui/settings.rs
git commit -m "feat: implement soundboard with folder scanning and playback"
```

---

## Chunk 5: Media Audio Routing

### Task 9: Implement URL playback via yt-dlp

**Files:**
- Create: `src/media/mod.rs`
- Create: `src/media/url_player.rs`
- Modify: `src/main.rs`
- Modify: `src/app.rs`
- Modify: `src/ui/media.rs`

- [ ] **Step 1: Create `src/media/mod.rs`**

```rust
pub mod url_player;
pub mod desktop_capture;
```

- [ ] **Step 2: Create `src/media/url_player.rs`**

```rust
use anyhow::{Context, Result};
use std::process::{Child, Command, Stdio};

pub struct UrlPlayer {
    child: Option<Child>,
}

impl UrlPlayer {
    pub fn new() -> Self {
        Self { child: None }
    }

    /// Check if the subprocess has finished (poll without blocking)
    pub fn poll_finished(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.child = None;
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn is_playing(&self) -> bool {
        self.child.is_some()
    }

    /// Check if yt-dlp and ffmpeg are available
    pub fn check_dependencies() -> (bool, bool) {
        let ytdlp = Command::new("yt-dlp").arg("--version")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().map(|s| s.success()).unwrap_or(false);
        let ffmpeg = Command::new("ffmpeg").arg("-version")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().map(|s| s.success()).unwrap_or(false);
        (ytdlp, ffmpeg)
    }

    /// Start streaming audio from URL to the virtual device
    pub fn play(&mut self, url: &str, device_name: &str) -> Result<()> {
        self.stop();

        let pipeline = format!(
            "yt-dlp -o - -f bestaudio '{}' | ffmpeg -i pipe:0 -f wav -acodec pcm_s16le -ar 24000 -ac 1 pipe:1 | paplay --device '{}'",
            url.replace('\'', "'\\''"),
            device_name.replace('\'', "'\\''"),
        );

        let child = Command::new("sh")
            .args(["-c", &pipeline])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start media pipeline")?;

        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(ref mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for UrlPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}
```

- [ ] **Step 3: Add `mod media;` to `src/main.rs`**

```rust
mod media;
```

- [ ] **Step 4: Add media state to `AppState` in `src/app.rs`**

```rust
pub media_url: String,
pub media_playing: bool,
pub pending_media_play: Option<String>,
pub pending_media_stop: bool,
pub media_deps_checked: bool,
pub media_has_ytdlp: bool,
pub media_has_ffmpeg: bool,
```

Initialize all as empty/false/None.

Add `url_player` to `ZundamonApp`:

```rust
url_player: crate::media::url_player::UrlPlayer,
```

Initialize: `url_player: crate::media::url_player::UrlPlayer::new()`.

- [ ] **Step 5: Handle media actions in `process_actions()`**

```rust
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
```

- [ ] **Step 6: Implement media tab UI in `src/ui/media.rs`**

```rust
use crate::app::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("メディア");
    ui.separator();

    // Dependency status
    if !state.media_has_ytdlp || !state.media_has_ffmpeg {
        ui.colored_label(
            egui::Color32::from_rgb(255, 200, 100),
            "必要なツール:",
        );
        if !state.media_has_ytdlp {
            ui.label("  yt-dlp が見つかりません。インストールしてください。");
        }
        if !state.media_has_ffmpeg {
            ui.label("  ffmpeg が見つかりません。インストールしてください。");
        }
        ui.add_space(8.0);
    }

    // URL playback section
    ui.label("URL再生");
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.media_url)
                .hint_text("URLを入力...")
                .desired_width(300.0),
        );
    });

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let can_play = !state.media_url.trim().is_empty()
            && !state.media_playing
            && state.media_has_ytdlp
            && state.media_has_ffmpeg;
        if ui.add_enabled(can_play, egui::Button::new("再生")).clicked() {
            state.pending_media_play = Some(state.media_url.trim().to_string());
        }
        if ui.add_enabled(state.media_playing, egui::Button::new("停止")).clicked() {
            state.pending_media_stop = true;
        }
    });

    if state.media_playing {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("メディア再生中...");
        });
    }

    ui.add_space(16.0);
    ui.separator();

    // Desktop capture section placeholder
    ui.label("デスクトップ音声キャプチャ");
    ui.add_space(4.0);
    ui.label("準備中...");
}
```

- [ ] **Step 7: Run `cargo check`**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 8: Commit**

```bash
git add src/media/ src/main.rs src/app.rs src/ui/media.rs
git commit -m "feat: add URL media playback via yt-dlp pipeline"
```

---

### Task 10: Implement desktop audio capture

**Files:**
- Create: `src/media/desktop_capture.rs`
- Modify: `src/app.rs`
- Modify: `src/ui/media.rs`

- [ ] **Step 1: Create `src/media/desktop_capture.rs`**

```rust
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

    pub fn is_capturing(&self) -> bool {
        self.combined_module_id.is_some()
    }

    /// List currently playing audio applications
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
                    if !current_name.is_empty() {
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
                current_sink = trimmed
                    .strip_prefix("Sink: ")
                    .unwrap_or("")
                    .to_string();
            }
        }
        // Don't forget the last one
        if let Some(id) = current_id {
            if !current_name.is_empty() {
                inputs.push(SinkInput {
                    id,
                    name: current_name,
                    sink: current_sink,
                });
            }
        }

        Ok(inputs)
    }

    /// Start capturing a sink-input by creating a combine-sink and redirecting
    pub fn start_capture(
        &mut self,
        sink_input_id: u32,
        original_sink: &str,
        virtual_sink: &str,
    ) -> Result<()> {
        self.stop_capture();

        // Get default sink name
        let default_sink = Self::get_default_sink()?;

        // Create combine-sink
        let output = Command::new("pactl")
            .args([
                "load-module",
                "module-combine-sink",
                &format!("sink_name=ZundamonCombined"),
                &format!("slaves={},{}", virtual_sink, default_sink),
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

        // Move sink-input to combined sink
        let output = Command::new("pactl")
            .args([
                "move-sink-input",
                &sink_input_id.to_string(),
                "ZundamonCombined",
            ])
            .output()
            .context("Failed to move sink-input")?;

        if !output.status.success() {
            // Cleanup combine-sink on failure
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
            "Started desktop capture: sink-input {} → ZundamonCombined",
            sink_input_id
        );
        Ok(())
    }

    pub fn stop_capture(&mut self) {
        // Restore original routing
        if let (Some(input_id), Some(ref sink)) = (self.captured_input_id, &self.original_sink) {
            let _ = Command::new("pactl")
                .args(["move-sink-input", &input_id.to_string(), sink])
                .output();
        }
        // Unload combine-sink
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

    /// Clean up stale ZundamonCombined sinks from previous runs.
    /// Finds module IDs by parsing `pactl list short modules` for combine-sink
    /// entries whose arguments contain "ZundamonCombined".
    pub fn cleanup_stale() {
        let output = Command::new("pactl")
            .args(["list", "short", "modules"])
            .output();
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("module-combine-sink") && line.contains("ZundamonCombined") {
                    // Format: <module_id>\tmodule-combine-sink\t<args>
                    if let Some(id_str) = line.split_whitespace().next() {
                        let _ = Command::new("pactl")
                            .args(["unload-module", id_str])
                            .output();
                        tracing::info!("Cleaned up stale ZundamonCombined module {}", id_str);
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
```

- [ ] **Step 2: Add desktop capture state to `AppState` in `src/app.rs`**

```rust
pub sink_inputs: Vec<crate::media::desktop_capture::SinkInput>,
pub pending_refresh_sink_inputs: bool,
pub pending_start_capture: Option<(u32, String)>, // (sink_input_id, original_sink)
pub pending_stop_capture: bool,
pub is_capturing: bool,
```

Initialize all as empty/false/None.

Add `desktop_capture` to `ZundamonApp`:

```rust
desktop_capture: crate::media::desktop_capture::DesktopCapture,
```

Initialize: `desktop_capture: crate::media::desktop_capture::DesktopCapture::new()`.

Call cleanup on startup in `ZundamonApp::new()`:

```rust
crate::media::desktop_capture::DesktopCapture::cleanup_stale();
```

- [ ] **Step 3: Handle desktop capture actions in `process_actions()`**

```rust
if self.state.pending_refresh_sink_inputs {
    self.state.pending_refresh_sink_inputs = false;
    match crate::media::desktop_capture::DesktopCapture::list_sink_inputs() {
        Ok(inputs) => self.state.sink_inputs = inputs,
        Err(e) => self.state.last_error = Some(format!("sink-input取得失敗: {}", e)),
    }
}

if let Some((input_id, original_sink)) = self.state.pending_start_capture.take() {
    let virtual_sink = &self.state.config.virtual_device_name;
    match self.desktop_capture.start_capture(input_id, &original_sink, virtual_sink) {
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
```

- [ ] **Step 4: Update media tab UI in `src/ui/media.rs`**

Replace the "デスクトップ音声キャプチャ" placeholder section with:

```rust
// Desktop capture section
ui.label("デスクトップ音声キャプチャ");
ui.add_space(4.0);

ui.horizontal(|ui| {
    if ui.button("アプリ一覧を更新").clicked() {
        state.pending_refresh_sink_inputs = true;
    }
    if state.is_capturing {
        if ui.button("キャプチャ停止").clicked() {
            state.pending_stop_capture = true;
        }
        ui.colored_label(
            egui::Color32::from_rgb(100, 200, 100),
            "キャプチャ中",
        );
    }
});

ui.add_space(4.0);

if state.sink_inputs.is_empty() {
    ui.label("「アプリ一覧を更新」を押してください");
} else {
    for input in state.sink_inputs.clone() {
        ui.horizontal(|ui| {
            ui.label(format!("{} (ID: {})", input.name, input.id));
            let can_capture = !state.is_capturing && state.device_ready;
            if ui.add_enabled(can_capture, egui::Button::new("キャプチャ")).clicked() {
                state.pending_start_capture = Some((input.id, input.sink.clone()));
            }
        });
    }
}
```

- [ ] **Step 5: Run `cargo check`**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 6: Run `cargo build`**

Run: `cargo build`
Expected: Builds successfully.

- [ ] **Step 7: Commit**

```bash
git add src/media/desktop_capture.rs src/app.rs src/ui/media.rs
git commit -m "feat: add desktop audio capture via PulseAudio combine-sink"
```

---

## Chunk 6: Final Integration and Polish

### Task 11: Wire up all remaining pieces and test

**Files:**
- Modify: `src/app.rs` (ensure Drop cleans up desktop capture)
- Modify: `src/main.rs` (window size)

- [ ] **Step 1: Ensure cleanup in `Drop for ZundamonApp`**

Add desktop capture and URL player cleanup:

```rust
impl Drop for ZundamonApp {
    fn drop(&mut self) {
        self.url_player.stop();
        self.desktop_capture.stop_capture();
        if self.is_docker {
            Self::stop_docker_container();
        }
        if let Some(ref mut child) = self.voicevox_process {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
```

- [ ] **Step 2: Increase default window size**

In `src/main.rs`, update the viewport size:

```rust
.with_inner_size([560.0, 700.0])
.with_min_inner_size([400.0, 500.0]),
```

- [ ] **Step 3: Run `cargo build`**

Run: `cargo build`
Expected: Builds successfully.

- [ ] **Step 4: Run `cargo clippy`**

Run: `cargo clippy`
Expected: No errors (warnings are OK for now).

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/main.rs
git commit -m "feat: add cleanup handlers and increase window size"
```

- [ ] **Step 6: Run the app manually and verify**

Run: `cargo run`
Expected:
- App opens with 4 tabs: 入力 | サウンドボード | メディア | 設定
- Settings show new sections: OSC設定, 音声エフェクト, ユーザー辞書, サウンドボード
- Soundboard tab shows folder path and rescan button
- Media tab shows URL input and desktop capture section

- [ ] **Step 7: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "fix: address issues found during manual testing"
```
