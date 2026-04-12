# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Build release
cargo build --release

# Run
cargo run

# Check (faster than build, no binary)
cargo check

# Lint
cargo clippy

# Format
cargo fmt

# Run tests
cargo test

# Run a single test
cargo test <test_name>
```

## What This Is

A Linux desktop app (egui/eframe) that routes VOICEVOX TTS audio to a PulseAudio virtual microphone, intended for use as a voice changer / voice output tool in VRChat. Text is synthesized via the VOICEVOX HTTP API and played through a virtual PulseAudio sink so VRChat can pick it up as a microphone source.

## Architecture

The app has two concurrency contexts that communicate via `std::sync::mpsc` channels:

- **Main (UI) thread** — runs the egui event loop via `eframe`. Renders UI, processes pending actions, and polls messages from the TTS thread.
- **Tokio runtime** — spawned at startup and used exclusively for async HTTP calls to the VOICEVOX API (`src/tts/voicevox.rs`). The runtime handle is passed into `ZundamonApp` but async work only happens in `tts_loop`.

### Channel pattern (`src/app.rs`)
- `TtsCommand` (UI → Tokio): `Synthesize`, `LoadSpeakers`, `HealthCheck`
- `UiMessage` (Tokio → UI): `WavReady`, `SpeakersLoaded`, `HealthCheckResult`, `Error`
- UI sets "pending_*" flags on `AppState` each frame; `process_actions()` drains them and sends `TtsCommand`s. This keeps all egui state mutations on the UI thread.

### Audio pipeline
1. `VirtualDevice` (`src/audio/virtual_device.rs`) creates a PulseAudio null sink via `pactl load-module module-null-sink`. VRChat should be configured to use the monitor source (`<sink_name>.monitor`) as its microphone input.
2. `playback::play_wav` (`src/audio/playback.rs`) plays TTS WAV through `paplay` to the configured virtual sink. Monitor playback to speakers is handled separately.

### TTS abstraction (`src/tts/`)
`TtsEngine` trait with `list_speakers`, `synthesize`, `health_check`. Only `VoicevoxEngine` is implemented. Synthesis is a two-step VOICEVOX API call: `POST /audio_query` then `POST /synthesis`.

### Config (`src/config.rs`)
TOML config stored via `directories::ProjectDirs` (typically `~/.config/zundamon_vrc/config.toml`). Config is auto-saved on changes from the UI (speaker selection, templates, settings). Fields: `voicevox_url`, `voicevox_path`, `speaker_id`, `virtual_device_name`, `templates`, `synth_params`.

### UI (`src/ui/`)
Two screens (tabs): `Screen::Input` (`src/ui/input.rs`) and `Screen::Settings` (`src/ui/settings.rs`). Japanese text requires NotoSansCJK font — `setup_japanese_fonts` in `main.rs` tries several system paths.

## Runtime Dependencies

- **PulseAudio** (`pactl`, `paplay`) — required for virtual device creation and TTS audio playback
- **VOICEVOX Engine** — HTTP server at `http://127.0.0.1:50021` by default; can be a local binary, command with args, or Docker command
- **NotoSansCJK font** — for Japanese text rendering (checked at runtime, gracefully degrades)
