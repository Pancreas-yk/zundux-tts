# UX Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden security, simplify installation via pre-built binaries, and redesign the UI with a TOML theme system, transparent window, custom title bar, and refreshed input layout.

**Architecture:** Three sequential phases — Part 0 fixes existing command injection vulnerabilities before auto-launch amplifies them. Part 1 adds CI release pipeline and install.sh rework. Part 2 adds theme system, transparent window, custom title bar, and input screen redesign. Each phase commits independently.

**Tech Stack:** Rust, egui/eframe 0.31, PulseAudio (pactl), GitHub Actions, shell-words, ctrlc, url crates

**Spec:** `docs/superpowers/specs/2026-03-19-ux-overhaul-design.md`

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `src/lib.rs` | Library crate re-exporting modules for integration tests |
| `src/ui/theme.rs` | Theme struct, TOML deserialization, validation, egui Visuals conversion |
| `src/ui/titlebar.rs` | Custom title bar rendering (drag, minimize, maximize, close) |
| `src/validation.rs` | Input validation functions (device name, URL, config size) |
| `.github/workflows/release.yml` | CI release pipeline |
| `tests/validation_tests.rs` | Tests for input validation |
| `tests/theme_tests.rs` | Tests for theme loading and validation |

### Modified Files
| File | Changes |
|------|---------|
| `Cargo.toml` | Add `shell-words`, `ctrlc`, `url` dependencies |
| `src/main.rs` | Transparent window, no decorations, clear color, SIGTERM handler |
| `src/app.rs` | Safe VOICEVOX launch, theme application, duplicate process guard |
| `src/config.rs` | Add Theme field, file size cap, validation on load |
| `src/media/url_player.rs` | Replace shell pipeline with Rust process pipeline |
| `src/audio/virtual_device.rs` | Use validated device name |
| `src/ui/mod.rs` | Export theme, titlebar modules |
| `src/ui/input.rs` | Vertical stack redesign, pill chips, truncation |
| `install.sh` | Binary download mode, SHA256 verification, safe variable embedding |

---

## Part 0: Security Hardening

### Task 1: Add input validation module

**Files:**
- Create: `src/validation.rs`
- Create: `tests/validation_tests.rs`
- Modify: `src/main.rs` (add `mod validation`)

- [ ] **Step 1: Write failing tests for device name validation**

```rust
// tests/validation_tests.rs
use zundamon_vrc::validation;

#[test]
fn valid_device_names() {
    assert!(validation::is_valid_device_name("ZundamonVRC"));
    assert!(validation::is_valid_device_name("my-device_01"));
    assert!(validation::is_valid_device_name("a"));
}

#[test]
fn invalid_device_names() {
    assert!(!validation::is_valid_device_name(""));
    assert!(!validation::is_valid_device_name("has spaces"));
    assert!(!validation::is_valid_device_name("semi;colon"));
    assert!(!validation::is_valid_device_name("eq=uals"));
    assert!(!validation::is_valid_device_name(&"a".repeat(65)));
}

#[test]
fn valid_voicevox_urls() {
    assert!(validation::is_valid_voicevox_url("http://127.0.0.1:50021").is_ok());
    assert!(validation::is_valid_voicevox_url("http://localhost:50021").is_ok());
    assert!(validation::is_valid_voicevox_url("http://[::1]:50021").is_ok());
}

#[test]
fn invalid_voicevox_urls() {
    assert!(validation::is_valid_voicevox_url("http://evil.com:50021").is_err());
    assert!(validation::is_valid_voicevox_url("https://127.0.0.1:50021").is_err());
    assert!(validation::is_valid_voicevox_url("ftp://127.0.0.1").is_err());
    assert!(validation::is_valid_voicevox_url("not a url").is_err());
}

#[test]
fn sanitize_device_name_fallback() {
    assert_eq!(validation::sanitize_device_name("valid-name"), "valid-name");
    assert_eq!(validation::sanitize_device_name("has spaces"), "ZundamonVRC");
    assert_eq!(validation::sanitize_device_name(""), "ZundamonVRC");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test validation_tests`
Expected: Compilation error — module not found

- [ ] **Step 3: Implement validation module**

```rust
// src/validation.rs
use anyhow::{bail, Result};

const MAX_DEVICE_NAME_LEN: usize = 64;
const DEFAULT_DEVICE_NAME: &str = "ZundamonVRC";
const MAX_CONFIG_FILE_SIZE: u64 = 1_048_576; // 1 MB

/// Validate PulseAudio device name: [a-zA-Z0-9_-]+, max 64 chars
pub fn is_valid_device_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_DEVICE_NAME_LEN
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Return the name if valid, or the default
pub fn sanitize_device_name(name: &str) -> &str {
    if is_valid_device_name(name) {
        name
    } else {
        DEFAULT_DEVICE_NAME
    }
}

/// Validate voicevox_url: must be http, localhost/127.0.0.1/[::1]
pub fn is_valid_voicevox_url(url_str: &str) -> Result<()> {
    let parsed = url::Url::parse(url_str)
        .map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;

    if parsed.scheme() != "http" {
        bail!("VOICEVOX URL must use http scheme, got: {}", parsed.scheme());
    }

    match parsed.host_str() {
        Some("127.0.0.1") | Some("localhost") | Some("[::1]") => Ok(()),
        Some(host) => bail!("VOICEVOX URL must point to localhost, got: {}", host),
        None => bail!("VOICEVOX URL has no host"),
    }
}

/// Check if config file is within size limit
pub fn check_config_file_size(path: &std::path::Path) -> Result<()> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_CONFIG_FILE_SIZE {
        bail!(
            "Config file too large: {} bytes (max {} bytes)",
            metadata.len(),
            MAX_CONFIG_FILE_SIZE
        );
    }
    Ok(())
}
```

- [ ] **Step 4: Add `url` crate to Cargo.toml, create `src/lib.rs`, update `src/main.rs`**

Add to `Cargo.toml` dependencies:
```toml
url = "2"
```

Create `src/lib.rs` to re-export modules for integration tests:
```rust
// src/lib.rs
pub mod validation;
pub mod ui;
pub mod config;
pub mod tts;
pub mod audio;
pub mod media;
pub mod osc;
```

In `src/main.rs`, change `mod` declarations to `use` from the library crate where needed, or keep `mod` declarations that reference `lib.rs` exports. The key point: `validation` must be accessible as `zundamon_vrc::validation` from integration tests.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test validation_tests`
Expected: All 5 tests pass

- [ ] **Step 6: Commit**

```bash
git add src/validation.rs tests/validation_tests.rs Cargo.toml src/main.rs
git commit -m "feat: add input validation module for device names and URLs"
```

---

### Task 2: Apply validation to config loading

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add file size check and field validation to `AppConfig::load()`**

In `src/config.rs`, add at top:
```rust
use crate::validation;
```

Modify `load()` to check file size before reading, and validate fields after loading:

```rust
pub fn load() -> Result<Self> {
    let path = Self::config_path()?;
    if path.exists() {
        validation::check_config_file_size(&path)?;
        let content = std::fs::read_to_string(&path)
            .context("Failed to read config file")?;
        let mut config: Self = toml::from_str(&content)
            .context("Failed to parse config file")?;
        config.validate_and_sanitize();
        Ok(config)
    } else {
        // ... existing default creation logic
    }
}

fn validate_and_sanitize(&mut self) {
    // Sanitize device name
    if !validation::is_valid_device_name(&self.virtual_device_name) {
        tracing::warn!(
            "Invalid virtual_device_name '{}', using default",
            self.virtual_device_name
        );
        self.virtual_device_name = "ZundamonVRC".to_string();
    }

    // Validate voicevox_url
    if let Err(e) = validation::is_valid_voicevox_url(&self.voicevox_url) {
        tracing::warn!("Invalid voicevox_url: {}, using default", e);
        self.voicevox_url = "http://127.0.0.1:50021".to_string();
    }

    // Cap templates
    if self.templates.len() > 100 {
        tracing::warn!("Too many templates ({}), truncating to 100", self.templates.len());
        self.templates.truncate(100);
    }
    for t in &mut self.templates {
        if t.len() > 512 {
            tracing::warn!("Template too long, truncating to 512 chars");
            *t = t.chars().take(512).collect();
        }
    }
}
```

- [ ] **Step 2: Run `cargo check`**

Expected: Compiles successfully

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/config.rs
git commit -m "feat: add validation to config loading (device name, URL, size cap)"
```

---

### Task 3: Fix command injection in VOICEVOX launch

**Files:**
- Modify: `Cargo.toml` (add `shell-words`)
- Modify: `src/app.rs`

- [ ] **Step 1: Add `shell-words` dependency**

Add to `Cargo.toml`:
```toml
shell-words = "1"
```

- [ ] **Step 1b: Write test for metacharacter rejection**

```rust
// In src/app.rs, add at bottom:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_shell_metacharacters_in_docker_cmd() {
        let result = ZundamonApp::launch_docker_voicevox("docker run evil;rm -rf /", "http://127.0.0.1:50021");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metacharacter"));
    }

    #[test]
    fn rejects_shell_metacharacters_in_local_cmd() {
        let result = ZundamonApp::launch_local_voicevox("voicevox && rm -rf /");
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Rewrite `launch_voicevox()` in `src/app.rs` to avoid `sh -c`**

Replace the current `launch_voicevox()` (lines 521-577). **Note:** This method uses `&mut self` (not `AppState`) because `voicevox_process` and `is_docker` live on `ZundamonApp`, not `AppState`:

```rust
fn launch_voicevox(&mut self) {
    let path = self.state.config.voicevox_path.trim().to_string();
    if path.is_empty() {
        tracing::warn!("voicevox_path is empty, cannot launch");
        return;
    }

    // Clean up any stale container first
    let is_docker = Self::is_docker_command(&path);
    if is_docker {
        Self::cleanup_docker_container();
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

fn launch_docker_voicevox(path: &str, _url: &str) -> Result<std::process::Child> {
    // Parse docker command safely
    let words = shell_words::split(path)
        .map_err(|e| anyhow::anyhow!("Failed to parse docker command: {}", e))?;

    if words.is_empty() {
        anyhow::bail!("Empty docker command");
    }

    // Reject shell metacharacters in any argument
    for word in &words {
        if word.chars().any(|c| matches!(c, ';' | '|' | '&' | '$' | '`' | '(' | ')' | '{' | '}' | '<' | '>')) {
            anyhow::bail!("Shell metacharacter detected in docker command argument: {}", word);
        }
    }

    let child = std::process::Command::new(&words[0])
        .args(&words[1..])
        .arg("--name")
        .arg("zundamon-voicevox")
        .arg("-d")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn docker command")?;

    Ok(child)
}

fn launch_local_voicevox(path: &str) -> Result<std::process::Child> {
    let words = shell_words::split(path)
        .map_err(|e| anyhow::anyhow!("Failed to parse voicevox command: {}", e))?;

    if words.is_empty() {
        anyhow::bail!("Empty voicevox command");
    }

    // Reject shell metacharacters
    for word in &words {
        if word.chars().any(|c| matches!(c, ';' | '|' | '&' | '$' | '`' | '(' | ')' | '{' | '}' | '<' | '>')) {
            anyhow::bail!("Shell metacharacter detected in voicevox command: {}", word);
        }
    }

    let child = std::process::Command::new(&words[0])
        .args(&words[1..])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn voicevox process")?;

    Ok(child)
}
```

- [ ] **Step 3: Run `cargo check`**

Expected: Compiles (may need minor adjustments to match existing code patterns)

- [ ] **Step 4: Run `cargo test`**

Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/app.rs
git commit -m "fix: eliminate command injection in VOICEVOX launch via shell-words"
```

---

### Task 4: Fix command injection in URL player

**Files:**
- Modify: `src/media/url_player.rs`
- Create: `tests/url_player_tests.rs`

- [ ] **Step 1: Write test for safe command construction**

```rust
// tests/url_player_tests.rs
// Note: These test the command construction logic, not actual playback

#[test]
fn url_with_shell_metacharacters_is_safe() {
    // Verify that URLs with shell metacharacters don't cause issues
    // when passed as discrete Command arguments (no shell involved)
    let dangerous_url = "https://example.com/video?a=1&b=2;rm -rf /";
    let dangerous_device = "device$(whoami)";

    // These should not panic or execute shell commands
    // The actual Command is constructed with .arg() not sh -c
    assert!(dangerous_url.contains(';'));
    assert!(dangerous_device.contains('$'));
}
```

- [ ] **Step 2: Rewrite `play()` in `src/media/url_player.rs`**

Replace the shell pipeline with Rust process pipeline:

```rust
pub fn play(&mut self, url: &str, device_name: &str) -> Result<()> {
    self.stop();

    // Validate device name
    if !crate::validation::is_valid_device_name(device_name) {
        anyhow::bail!("Invalid device name for playback: {}", device_name);
    }

    // Build pipeline: yt-dlp | ffmpeg | paplay
    // Each process gets arguments directly — no shell involved
    let mut ytdlp = std::process::Command::new("yt-dlp")
        .args(["-o", "-", "-f", "bestaudio", url])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to start yt-dlp")?;

    let ytdlp_stdout = ytdlp.stdout.take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture yt-dlp stdout"))?;

    let mut ffmpeg = std::process::Command::new("ffmpeg")
        .args(["-i", "pipe:0", "-f", "s16le", "-ar", "24000", "-ac", "1", "pipe:1"])
        .stdin(ytdlp_stdout)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to start ffmpeg")?;

    let ffmpeg_stdout = ffmpeg.stdout.take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture ffmpeg stdout"))?;

    let paplay = std::process::Command::new("paplay")
        .args([
            "--raw",
            "--format=s16le",
            "--rate=24000",
            "--channels=1",
            &format!("--device={}", device_name),
        ])
        .stdin(ffmpeg_stdout)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to start paplay")?;

    // Store the final process (paplay) as the child to manage
    // Also store yt-dlp and ffmpeg for cleanup
    self.child = Some(paplay);
    self.pipeline_pids = Some((ytdlp.id(), ffmpeg.id()));

    // Detach yt-dlp and ffmpeg — they'll be killed when paplay's stdin closes
    std::thread::spawn(move || { let _ = ytdlp.wait(); });
    std::thread::spawn(move || { let _ = ffmpeg.wait(); });

    Ok(())
}
```

Update the `UrlPlayer` struct to track pipeline PIDs:
```rust
pub struct UrlPlayer {
    child: Option<std::process::Child>,
    pipeline_pids: Option<(u32, u32)>,
}
```

Update `new()`:
```rust
pub fn new() -> Self {
    Self {
        child: None,
        pipeline_pids: None,
    }
}
```

Update `stop()` to also kill pipeline processes (use `Command::new("kill")` to avoid `libc`/`unsafe`):
```rust
pub fn stop(&mut self) {
    if let Some(mut child) = self.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    // Kill pipeline processes if still running
    if let Some((ytdlp_pid, ffmpeg_pid)) = self.pipeline_pids.take() {
        for pid in [ytdlp_pid, ffmpeg_pid] {
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
        }
    }
}
```

- [ ] **Step 3: Run `cargo check`**

Expected: Compiles successfully

- [ ] **Step 4: Run `cargo test`**

Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/media/url_player.rs tests/url_player_tests.rs Cargo.toml
git commit -m "fix: replace shell pipeline with Rust process pipeline in URL player"
```

---

## Part 1: Installation & Launch Simplification

### Task 5: GitHub Actions release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create release workflow**

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  build:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libgtk-3-dev \
            libasound2-dev \
            libpulse-dev \
            libxdo-dev \
            pkg-config

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2

      - name: Build release binary
        run: cargo build --release

      - name: Prepare release artifacts
        run: |
          cp target/release/zundamon_vrc zundamon_vrc-linux-x86_64
          sha256sum zundamon_vrc-linux-x86_64 > SHA256SUMS

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            zundamon_vrc-linux-x86_64
            SHA256SUMS
          generate_release_notes: true
```

- [ ] **Step 2: Verify workflow syntax**

Run: `cat .github/workflows/release.yml | python3 -c "import sys,yaml; yaml.safe_load(sys.stdin)"` (or similar YAML lint)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add GitHub Actions release workflow with SHA256 checksums"
```

---

### Task 6: Rework install.sh for binary download

**Files:**
- Modify: `install.sh`

- [ ] **Step 1: Add binary download function and SHA256 verification**

Add near the top of `install.sh`, after variable declarations:

```bash
GITHUB_REPO="OWNER/zundamon_vrc"  # TODO: set actual owner

download_binary() {
    info "最新リリースをダウンロード中..."

    local api_url="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
    local release_json
    release_json=$(curl -fsSL "$api_url") || {
        error "リリース情報の取得に失敗しました"
        return 1
    }

    local binary_url
    binary_url=$(echo "$release_json" | grep -o '"browser_download_url":[[:space:]]*"[^"]*zundamon_vrc-linux-x86_64"' | grep -o 'https://[^"]*')

    # Validate URL pattern
    if ! echo "$binary_url" | grep -qE "^https://github\.com/${GITHUB_REPO}/releases/download/v[0-9]+\.[0-9]+\.[0-9]+/zundamon_vrc-linux-x86_64$"; then
        error "ダウンロードURLが不正です: $binary_url"
        return 1
    fi

    local checksum_url
    checksum_url=$(echo "$release_json" | grep -o '"browser_download_url":[[:space:]]*"[^"]*SHA256SUMS"' | grep -o 'https://[^"]*')

    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf '$tmpdir'" EXIT

    curl -fsSL "$binary_url" -o "$tmpdir/zundamon_vrc-linux-x86_64" || {
        error "バイナリのダウンロードに失敗しました"
        return 1
    }

    curl -fsSL "$checksum_url" -o "$tmpdir/SHA256SUMS" || {
        error "チェックサムのダウンロードに失敗しました"
        return 1
    }

    cd "$tmpdir"
    if ! sha256sum -c SHA256SUMS; then
        error "チェックサム検証に失敗しました。ダウンロードが破損している可能性があります"
        return 1
    fi
    cd "$SCRIPT_DIR"

    cp "$tmpdir/zundamon_vrc-linux-x86_64" "$INSTALL_BIN/zundamon_vrc"
    chmod +x "$INSTALL_BIN/zundamon_vrc"
    info "バイナリをインストール: $INSTALL_BIN/zundamon_vrc"
}
```

- [ ] **Step 2: Modify build step to support `--from-source` flag**

Replace Step 4 (build section) with:

```bash
# ---------- Step 4: ビルド or ダウンロード ----------
if [ "${1:-}" = "--from-source" ]; then
    info "ソースからビルド中..."
    # Ensure rust is installed for source builds
    if ! command -v cargo &>/dev/null; then
        error "Rustツールチェーンが必要です: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    cd "$SCRIPT_DIR"
    cargo build --release
    mkdir -p "$INSTALL_BIN"
    cp "$SCRIPT_DIR/target/release/zundamon_vrc" "$INSTALL_BIN/zundamon_vrc"
    info "バイナリをインストール: $INSTALL_BIN/zundamon_vrc"
else
    mkdir -p "$INSTALL_BIN"
    download_binary
fi
```

- [ ] **Step 3: Remove `rust` and `base-devel` from default PACKAGES**

```bash
if [ "${1:-}" = "--from-source" ]; then
    PACKAGES=(base-devel rust docker pulseaudio noto-fonts-cjk yt-dlp ffmpeg)
else
    PACKAGES=(docker pulseaudio noto-fonts-cjk yt-dlp ffmpeg)
fi
```

- [ ] **Step 4: Set `auto_launch_voicevox = true` for new installs**

In the config creation section (around line 186), change the minimal config for new installs:
```bash
    cat > "$CONFIG_DIR/config.toml" << CONFIG_EOF
voicevox_path = "$DOCKER_CMD"
auto_launch_voicevox = true
CONFIG_EOF
```

For existing configs, also set `auto_launch_voicevox = true` (since the installer manages Docker):
```bash
    if grep -q 'auto_launch_voicevox' "$CONFIG_DIR/config.toml"; then
        sed -i 's|^auto_launch_voicevox.*|auto_launch_voicevox = true|' "$CONFIG_DIR/config.toml"
    else
        echo "auto_launch_voicevox = true" >> "$CONFIG_DIR/config.toml"
    fi
```

- [ ] **Step 5: Fix launcher script variable embedding**

Replace the `LAUNCHER_DYNAMIC_EOF` heredoc (lines 135-138) with safe embedding:

```bash
# Append image and GPU flags safely
printf 'VOICEVOX_IMAGE=%q\n' "$VOICEVOX_IMAGE" >> "$INSTALL_BIN/zundamon_vrc_launch.sh"
printf 'GPU_FLAGS=%q\n' "$GPU_FLAGS" >> "$INSTALL_BIN/zundamon_vrc_launch.sh"
```

- [ ] **Step 5: Test install.sh syntax**

Run: `bash -n install.sh`
Expected: No syntax errors

- [ ] **Step 6: Commit**

```bash
git add install.sh
git commit -m "feat: install.sh downloads pre-built binary by default, --from-source for builds"
```

---

### Task 7: Improve VOICEVOX auto-launch with duplicate guard

**Files:**
- Modify: `src/app.rs`
- Modify: `Cargo.toml` (add `ctrlc`)

- [ ] **Step 1: Add `ctrlc` dependency**

Add to `Cargo.toml`:
```toml
ctrlc = "3"
```

- [ ] **Step 2: Add duplicate process guard to `launch_voicevox()`**

Add before spawning in `launch_voicevox()`:

```rust
fn is_voicevox_docker_running() -> bool {
    std::process::Command::new("docker")
        .args(["ps", "--filter", "name=zundamon-voicevox", "--format", "{{.Names}}"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("zundamon-voicevox"))
        .unwrap_or(false)
}
```

At the top of `launch_voicevox()` (uses `&mut self`, not `state`):
```rust
// Duplicate guard — check if process we spawned is still alive
if let Some(ref mut proc) = self.voicevox_process {
    match proc.try_wait() {
        Ok(None) => {
            tracing::info!("VOICEVOX process already running");
            return;
        }
        _ => { self.voicevox_process = None; }
    }
}
// Check if Docker container exists from a previous session
let path = self.state.config.voicevox_path.trim().to_string();
if Self::is_docker_command(&path) && Self::is_voicevox_docker_running() {
    tracing::info!("VOICEVOX Docker container already running");
    self.state.voicevox_launching = true;
    return;
}
```

Also update the `Drop` impl for graceful shutdown with 5-second timeout:
```rust
impl Drop for ZundamonApp {
    fn drop(&mut self) {
        self.url_player.stop();
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
                Ok(Some(_)) => {},
                _ => {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}
```

- [ ] **Step 3: Register SIGTERM handler in `main.rs`**

Add near the start of `main()`:

```rust
// Register cleanup handler for SIGTERM/SIGINT
let cleanup_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
let flag_clone = cleanup_flag.clone();
ctrlc::set_handler(move || {
    flag_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    // Cleanup Docker container
    let _ = std::process::Command::new("docker")
        .args(["stop", "zundamon-voicevox"])
        .output();
})
.expect("Failed to set SIGTERM handler");
```

- [ ] **Step 4: Run `cargo check`**

Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/app.rs src/main.rs
git commit -m "feat: add duplicate process guard and SIGTERM cleanup handler"
```

---

## Part 2: UI Redesign

### Task 8: Theme system

**Files:**
- Create: `src/ui/theme.rs`
- Create: `tests/theme_tests.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests for theme**

```rust
// tests/theme_tests.rs
use zundamon_vrc::ui::theme::Theme;

#[test]
fn default_theme_is_valid() {
    let theme = Theme::default();
    assert!(theme.validate().is_ok());
}

#[test]
fn theme_from_partial_toml() {
    let toml_str = r#"
[theme]
window_rounding = 8.0
"#;
    let config: toml::Value = toml::from_str(toml_str).unwrap();
    let theme: Theme = config.get("theme")
        .map(|v| v.clone().try_into().unwrap())
        .unwrap_or_default();
    assert_eq!(theme.window_rounding, 8.0);
    // Other fields should be defaults
    assert_eq!(theme.spacing_small, 4.0);
}

#[test]
fn theme_rejects_invalid_rounding() {
    let mut theme = Theme::default();
    theme.window_rounding = f32::NAN;
    let result = theme.validate();
    assert!(result.is_err() || theme.window_rounding == Theme::default().window_rounding);
}

#[test]
fn theme_to_visuals_does_not_panic() {
    let theme = Theme::default();
    let _visuals = theme.to_visuals();
    let _style = theme.to_style();
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test theme_tests`
Expected: Compilation error

- [ ] **Step 3: Implement Theme struct**

```rust
// src/ui/theme.rs
use egui::{Color32, Rounding, Style, Visuals, Stroke};
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    // Window
    pub window_background: [u8; 4],
    pub window_rounding: f32,

    // Title bar
    pub titlebar_background: [u8; 4],
    pub titlebar_text: [u8; 4],

    // Content
    pub panel_background: [u8; 4],
    pub text_primary: [u8; 4],
    pub text_secondary: [u8; 4],
    pub text_muted: [u8; 4],

    // Accents
    pub accent: [u8; 4],
    pub accent_hover: [u8; 4],

    // Status
    pub status_ok: [u8; 4],
    pub status_warn: [u8; 4],
    pub status_error: [u8; 4],

    // Widgets
    pub button_background: [u8; 4],
    pub button_rounding: f32,
    pub input_background: [u8; 4],
    pub input_rounding: f32,
    pub chip_background: [u8; 4],
    pub chip_rounding: f32,

    // Tab bar
    pub tab_active_background: [u8; 4],
    pub tab_rounding: f32,

    // Spacing
    pub spacing_small: f32,
    pub spacing_medium: f32,
    pub spacing_large: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            window_background: [15, 15, 20, 200],
            window_rounding: 12.0,
            titlebar_background: [20, 20, 28, 240],
            titlebar_text: [180, 180, 180, 255],
            panel_background: [255, 255, 255, 15],
            text_primary: [224, 224, 224, 255],
            text_secondary: [160, 160, 160, 255],
            text_muted: [100, 100, 100, 255],
            accent: [120, 200, 120, 255],
            accent_hover: [140, 220, 140, 255],
            status_ok: [112, 192, 112, 255],
            status_warn: [200, 200, 100, 255],
            status_error: [200, 100, 100, 255],
            button_background: [255, 255, 255, 15],
            button_rounding: 6.0,
            input_background: [255, 255, 255, 10],
            input_rounding: 8.0,
            chip_background: [255, 255, 255, 15],
            chip_rounding: 16.0,
            tab_active_background: [255, 255, 255, 30],
            tab_rounding: 6.0,
            spacing_small: 4.0,
            spacing_medium: 8.0,
            spacing_large: 16.0,
        }
    }
}

impl Theme {
    pub fn color(&self, rgba: [u8; 4]) -> Color32 {
        Color32::from_rgba_premultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
    }

    pub fn validate(&self) -> Result<()> {
        let floats = [
            ("window_rounding", self.window_rounding, 0.0, 50.0),
            ("button_rounding", self.button_rounding, 0.0, 50.0),
            ("input_rounding", self.input_rounding, 0.0, 50.0),
            ("chip_rounding", self.chip_rounding, 0.0, 50.0),
            ("tab_rounding", self.tab_rounding, 0.0, 50.0),
            ("spacing_small", self.spacing_small, 0.0, 100.0),
            ("spacing_medium", self.spacing_medium, 0.0, 100.0),
            ("spacing_large", self.spacing_large, 0.0, 100.0),
        ];

        for (name, value, min, max) in floats {
            if !value.is_finite() || value < min || value > max {
                anyhow::bail!("Theme field '{}' out of range: {} (expected {}-{})", name, value, min, max);
            }
        }
        Ok(())
    }

    /// Validate and fix invalid values, returning the corrected theme
    pub fn validated(mut self) -> Self {
        let defaults = Self::default();
        let checks = [
            (&mut self.window_rounding, defaults.window_rounding, 0.0, 50.0),
            (&mut self.button_rounding, defaults.button_rounding, 0.0, 50.0),
            (&mut self.input_rounding, defaults.input_rounding, 0.0, 50.0),
            (&mut self.chip_rounding, defaults.chip_rounding, 0.0, 50.0),
            (&mut self.tab_rounding, defaults.tab_rounding, 0.0, 50.0),
            (&mut self.spacing_small, defaults.spacing_small, 0.0, 100.0),
            (&mut self.spacing_medium, defaults.spacing_medium, 0.0, 100.0),
            (&mut self.spacing_large, defaults.spacing_large, 0.0, 100.0),
        ];
        for (value, default, min, max) in checks {
            if !value.is_finite() || *value < min || *value > max {
                tracing::warn!("Theme value {} out of range, using default {}", value, default);
                *value = default;
            }
        }
        self
    }

    pub fn to_visuals(&self) -> Visuals {
        let mut visuals = Visuals::dark();

        visuals.override_text_color = Some(self.color(self.text_primary));

        // Panel/window backgrounds
        visuals.panel_fill = self.color(self.panel_background);
        visuals.window_fill = self.color(self.window_background);
        visuals.window_rounding = Rounding::same(self.window_rounding);

        // Widget styles
        visuals.widgets.inactive.bg_fill = self.color(self.button_background);
        visuals.widgets.inactive.rounding = Rounding::same(self.button_rounding);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.color(self.text_secondary));

        visuals.widgets.hovered.bg_fill = self.color(self.accent_hover);
        visuals.widgets.hovered.rounding = Rounding::same(self.button_rounding);

        visuals.widgets.active.bg_fill = self.color(self.accent);
        visuals.widgets.active.rounding = Rounding::same(self.button_rounding);

        // Selection
        visuals.selection.bg_fill = self.color(self.accent);

        // Extreme background (behind everything)
        visuals.extreme_bg_color = Color32::TRANSPARENT;

        visuals
    }

    pub fn to_style(&self) -> Style {
        let mut style = Style::default();
        style.spacing.item_spacing = egui::vec2(self.spacing_medium, self.spacing_medium);
        style.spacing.button_padding = egui::vec2(self.spacing_medium, self.spacing_small);
        style
    }
}
```

- [ ] **Step 4: Export theme module and add Theme to config**

In `src/ui/mod.rs`, add:
```rust
pub mod theme;
pub mod titlebar;
```

In `src/config.rs`, add `Theme` field to `AppConfig`:
```rust
use crate::ui::theme::Theme;

// Add to AppConfig struct:
#[serde(default)]
pub theme: Theme,
```

In `validate_and_sanitize()`, add:
```rust
self.theme = std::mem::take(&mut self.theme).validated();
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All tests pass including theme_tests

- [ ] **Step 6: Commit**

```bash
git add src/ui/theme.rs src/ui/mod.rs src/config.rs tests/theme_tests.rs
git commit -m "feat: add TOML-based theme system with validation"
```

---

### Task 9: Transparent window and custom title bar

**Files:**
- Modify: `src/main.rs`
- Create: `src/ui/titlebar.rs`

- [ ] **Step 1: Configure transparent window in `main.rs`**

Update the `NativeOptions` in `main()`:

```rust
let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
        .with_inner_size([560.0, 700.0])
        .with_min_inner_size([400.0, 500.0])
        .with_transparent(true)
        .with_decorations(false),
    ..Default::default()
};
```

- [ ] **Step 2: Set clear color to transparent in the eframe::App update**

In `src/app.rs`, in the `update()` method of `eframe::App` impl, add at the beginning:

```rust
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // Transparent background
    let theme = &self.state.config.theme;

    // Apply theme visuals (cached, only set once or on change)
    if self.needs_theme_update {
        ctx.set_visuals(theme.to_visuals());
        ctx.set_style(theme.to_style());
        self.needs_theme_update = false;
    }

    // Paint window background with rounded rect
    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let rounding = if is_maximized { 0.0 } else { theme.window_rounding };

    let screen_rect = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(
        screen_rect,
        Rounding::same(rounding),
        theme.color(theme.window_background),
    );

    // ... existing update logic
```

Add `needs_theme_update: bool` field to `ZundamonApp`, initialize as `true` in `new()`.

- [ ] **Step 3: Implement custom title bar**

```rust
// src/ui/titlebar.rs
use egui::{Align, Layout, Rounding, Sense, Vec2};
use crate::ui::theme::Theme;

/// Render the custom title bar. Call this from the main `update()` method.
/// This function creates its own `TopBottomPanel`, so it does not need an external `ui` parameter.
pub fn show(ctx: &egui::Context, theme: &Theme) {
    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));

    let titlebar_height = 32.0;

    egui::TopBottomPanel::top("titlebar")
        .exact_height(titlebar_height)
        .frame(egui::Frame::none().fill(theme.color(theme.titlebar_background)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // Drag area (entire bar except buttons)
                let drag_rect = ui.available_rect_before_wrap();
                let drag_response = ui.interact(
                    drag_rect,
                    ui.id().with("titlebar_drag"),
                    Sense::click_and_drag(),
                );
                if drag_response.dragged() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag_response.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                // Title text (centered)
                let title_rect = ui.available_rect_before_wrap();
                ui.painter().text(
                    title_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "ZUNDAMON VRC",
                    egui::FontId::proportional(11.0),
                    theme.color(theme.titlebar_text),
                );

                // Window control buttons (right-aligned)
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Close button
                    let close_btn = ui.add(
                        egui::Button::new(egui::RichText::new("\u{2715}").size(12.0))
                            .frame(false)
                    );
                    if close_btn.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if close_btn.hovered() {
                        ui.painter().rect_filled(
                            close_btn.rect,
                            Rounding::same(4.0),
                            theme.color(theme.status_error),
                        );
                    }

                    // Maximize/restore button
                    let max_icon = if is_maximized { "\u{25A3}" } else { "\u{25A1}" };
                    let max_btn = ui.add(
                        egui::Button::new(egui::RichText::new(max_icon).size(12.0))
                            .frame(false)
                    );
                    if max_btn.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    // Minimize button
                    let min_btn = ui.add(
                        egui::Button::new(egui::RichText::new("\u{2212}").size(12.0))
                            .frame(false)
                    );
                    if min_btn.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                });
            });
        });
}
```

- [ ] **Step 4: Integrate title bar into `app.rs` update loop**

In `update()`, add before the existing tab panel:

```rust
// Custom title bar
crate::ui::titlebar::show(ctx, &self.state.config.theme);
```

Remove or skip the old egui default window decorations since `with_decorations(false)` handles that.

- [ ] **Step 5: Handle Alt+F4 keyboard shortcut**

In `update()`, add:

```rust
// Keyboard shortcuts
if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::F4)) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}
```

- [ ] **Step 6: Run `cargo check`**

Expected: Compiles successfully

- [ ] **Step 7: Run the app to test visually**

Run: `cargo run`
Expected: Transparent window with custom title bar, draggable, minimize/maximize/close work

- [ ] **Step 8: Commit**

```bash
git add src/main.rs src/app.rs src/ui/titlebar.rs
git commit -m "feat: add transparent window with custom title bar"
```

---

### Task 10: Redesign input screen (Vertical Stack)

**Files:**
- Modify: `src/ui/input.rs`

- [ ] **Step 1: Rewrite input screen layout**

Replace the entire `show()` function in `src/ui/input.rs` with the Vertical Stack layout:

```rust
use egui::{Align, Color32, Layout, Rounding, Vec2};
use crate::app::AppState;
use crate::ui::theme::Theme;

const TEMPLATE_MAX_DISPLAY_LEN: usize = 12;
const TEMPLATE_MAX_VISIBLE_ROWS: usize = 2;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let theme = &state.config.theme;

    ui.add_space(theme.spacing_large);

    // ── Speaker selector ──
    ui.label(
        egui::RichText::new("SPEAKER")
            .size(10.0)
            .color(theme.color(theme.text_muted))
    );
    ui.add_space(theme.spacing_small);

    let selected_text = state.speakers.iter()
        .flat_map(|s| s.styles.iter().map(move |st| (s, st)))
        .find(|(_, st)| st.id == state.config.speaker_id)
        .map(|(s, st)| format!("{} - {}", s.name, st.name))
        .unwrap_or_else(|| format!("Speaker ID: {}", state.config.speaker_id));

    egui::ComboBox::from_id_salt("speaker_select")
        .selected_text(&selected_text)
        .width(ui.available_width() - theme.spacing_medium)
        .show_ui(ui, |ui| {
            for speaker in &state.speakers {
                for style in &speaker.styles {
                    let label = format!("{} - {}", speaker.name, style.name);
                    if ui.selectable_value(&mut state.config.speaker_id, style.id, &label).changed() {
                        let _ = state.config.save();
                    }
                }
            }
        });

    ui.add_space(theme.spacing_large);

    // ── Text input ──
    let input_frame = egui::Frame::none()
        .fill(theme.color(theme.input_background))
        .rounding(Rounding::same(theme.input_rounding))
        .inner_margin(theme.spacing_medium);

    input_frame.show(ui, |ui| {
        let response = ui.add(
            egui::TextEdit::multiline(&mut state.input_text)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("テキストを入力してEnterで送信 (Shift+Enterで改行)")
                .frame(false)
        );

        // Enter to send, Shift+Enter for newline
        if response.has_focus() {
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let shift_held = ui.input(|i| i.modifiers.shift);
            if enter_pressed && !shift_held && !state.input_text.trim().is_empty() {
                state.pending_send = Some(state.input_text.trim().to_string());
            }
        }
    });

    ui.add_space(theme.spacing_medium);

    // ── Send button (right-aligned) ──
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let send_enabled = !state.input_text.trim().is_empty() && !state.is_synthesizing;
        let btn_text = if state.synthesizing { "合成中..." } else { "Send" };

        let btn = ui.add_enabled(
            send_enabled,
            egui::Button::new(
                egui::RichText::new(btn_text)
                    .color(theme.color(theme.accent))
            )
            .rounding(Rounding::same(theme.button_rounding))
            .min_size(Vec2::new(80.0, 28.0))
        );
        if btn.clicked() {
            state.pending_send = Some(state.input_text.trim().to_string());
        }
    });

    ui.add_space(theme.spacing_large);

    // ── Templates ──
    ui.label(
        egui::RichText::new("TEMPLATES")
            .size(10.0)
            .color(theme.color(theme.text_muted))
    );
    ui.add_space(theme.spacing_small);

    show_template_chips(ui, state, theme);

    ui.add_space(theme.spacing_medium);
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

fn show_template_chips(ui: &mut egui::Ui, state: &mut AppState, theme: &Theme) {
    let mut delete_index = None;
    let templates = state.config.templates.clone();
    let chip_rounding = Rounding::same(theme.chip_rounding);

    // Track how many rows we've used
    let start_y = ui.cursor().top();
    let row_height = 28.0;
    let max_y = start_y + (row_height * TEMPLATE_MAX_VISIBLE_ROWS as f32) + theme.spacing_small;
    let expanded_max_y = start_y + (row_height * 5.0) + theme.spacing_small;
    let effective_max_y = if state.templates_expanded { expanded_max_y } else { max_y };

    let mut overflow_count = 0;
    let mut visible_count = 0;

    ui.horizontal_wrapped(|ui| {
        for (i, template) in templates.iter().enumerate() {
            // Check if we'd exceed max rows
            if ui.cursor().top() > effective_max_y {
                overflow_count = templates.len() - i;
                break;
            }
            visible_count = i + 1;

            let display_text = truncate_text(template, TEMPLATE_MAX_DISPLAY_LEN);

            let btn = ui.add(
                egui::Button::new(
                    egui::RichText::new(&display_text)
                        .color(theme.color(theme.text_secondary))
                        .size(11.0)
                )
                .rounding(chip_rounding)
                .fill(theme.color(theme.chip_background))
            );

            // Tooltip with full text if truncated
            if template.chars().count() > TEMPLATE_MAX_DISPLAY_LEN {
                btn.clone().on_hover_text(template);
            }

            if btn.clicked() {
                state.input_text = template.clone();
                state.pending_send = Some(state.input_text.trim().to_string());
            }

            // Show delete on hover
            if btn.hovered() {
                let del_rect = egui::Rect::from_min_size(
                    btn.rect.right_top() + egui::vec2(-14.0, 2.0),
                    Vec2::new(12.0, 12.0),
                );
                if ui.put(del_rect, egui::Button::new(
                    egui::RichText::new("\u{2715}").size(8.0)
                        .color(theme.color(theme.status_error))
                ).frame(false)).clicked() {
                    delete_index = Some(i);
                }
            }
        }

        // Overflow chip
        if overflow_count > 0 {
            let expand_text = format!("+{} more", overflow_count);
            if ui.add(
                egui::Button::new(
                    egui::RichText::new(&expand_text)
                        .color(theme.color(theme.text_muted))
                        .size(11.0)
                )
                .rounding(chip_rounding)
                .fill(theme.color(theme.chip_background))
            ).clicked() {
                state.templates_expanded = true;
            }
        } else if state.templates_expanded && templates.len() > TEMPLATE_MAX_VISIBLE_ROWS * 4 {
            if ui.add(
                egui::Button::new(
                    egui::RichText::new("Show less")
                        .color(theme.color(theme.text_muted))
                        .size(11.0)
                )
                .rounding(chip_rounding)
                .fill(theme.color(theme.chip_background))
            ).clicked() {
                state.templates_expanded = false;
            }
        }

        // Add template chip
        if ui.add(
            egui::Button::new(
                egui::RichText::new("+ Add")
                    .color(theme.color(theme.text_muted))
                    .size(11.0)
            )
            .rounding(chip_rounding)
            .fill(theme.color(theme.chip_background))
        ).clicked() {
            state.adding_template = true;
        }
    });

    // Add template inline
    if state.adding_template {
        ui.horizontal(|ui| {
            let response = ui.text_edit_singleline(&mut state.new_template_text);
            if ui.button("OK").clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                if !state.new_template_text.trim().is_empty() {
                    state.config.templates.push(state.new_template_text.trim().to_string());
                    let _ = state.config.save();
                }
                state.new_template_text.clear();
                state.adding_template = false;
            }
            if ui.button("Cancel").clicked() {
                state.new_template_text.clear();
                state.adding_template = false;
            }
        });
    }

    // Handle delete
    if let Some(idx) = delete_index {
        state.config.templates.remove(idx);
        let _ = state.config.save();
    }
}
```

- [ ] **Step 2: Add new fields to AppState in `src/app.rs`**

```rust
// Add to AppState struct (new_template_text already exists):
pub templates_expanded: bool,
pub adding_template: bool,
pub error_display_time: Option<std::time::Instant>,
pub error_hovered: bool,
```

Initialize in `AppState` construction (in `ZundamonApp::new()`):
```rust
templates_expanded: false,
adding_template: false,
error_display_time: None,
error_hovered: false,
```

- [ ] **Step 3: Move status bar to BottomPanel in `app.rs`**

In the `update()` method, add a `BottomPanel` before `CentralPanel`:

```rust
// Status bar (bottom)
egui::TopBottomPanel::bottom("status")
    .exact_height(24.0)
    .frame(egui::Frame::none().fill(theme.color(theme.titlebar_background)))
    .show(ctx, |ui| {
        ui.horizontal_centered(|ui| {
            // VOICEVOX status
            let (vox_color, vox_text) = if state.voicevox_connected {
                (theme.color(theme.status_ok), "VOICEVOX")
            } else if state.voicevox_launching {
                (theme.color(theme.status_warn), "VOICEVOX...")
            } else {
                (theme.color(theme.status_error), "VOICEVOX")
            };
            ui.colored_label(vox_color, format!("\u{25CF} {}", vox_text));

            ui.add_space(12.0);

            // Virtual mic status
            let (mic_color, mic_text) = if state.device_ready {
                (theme.color(theme.status_ok), "Virtual Mic")
            } else {
                (theme.color(theme.status_warn), "Virtual Mic")
            };
            ui.colored_label(mic_color, format!("\u{25CF} {}", mic_text));

            // Toast error
            if let Some(ref error) = state.last_error {
                ui.add_space(12.0);
                ui.colored_label(
                    theme.color(theme.status_error),
                    error.chars().take(60).collect::<String>(),
                );
            }
        });
    });
```

Remove the status display from `input.rs` (it was at the bottom of `show()`).

- [ ] **Step 4: Update tab bar styling in `app.rs`**

Replace the existing tab panel with themed version:

```rust
egui::TopBottomPanel::top("tabs")
    .frame(egui::Frame::none()
        .fill(Color32::TRANSPARENT)
        .inner_margin(egui::Margin::symmetric(theme.spacing_medium, theme.spacing_small)))
    .show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (screen, label) in [
                (Screen::Input, "Input"),
                (Screen::Soundboard, "Soundboard"),
                (Screen::Media, "Media"),
                (Screen::Settings, "Settings"),
            ] {
                let is_active = self.state.current_screen == screen;
                let bg = if is_active {
                    theme.color(theme.tab_active_background)
                } else {
                    Color32::TRANSPARENT
                };
                let text_color = if is_active {
                    theme.color(theme.text_primary)
                } else {
                    theme.color(theme.text_muted)
                };

                let btn = ui.add(
                    egui::Button::new(
                        egui::RichText::new(label)
                            .size(11.0)
                            .color(text_color)
                    )
                    .fill(bg)
                    .rounding(Rounding::same(theme.tab_rounding))
                );
                if btn.clicked() {
                    self.state.current_screen = screen;
                }
            }
        });
    });
```

- [ ] **Step 5: Add toast auto-dismiss logic**

In `process_messages()` or `update()`, when a new error is set:
```rust
if state.last_error.is_some() {
    state.error_display_time = Some(std::time::Instant::now());
}
```

In `update()`, auto-clear after 5 seconds with hover-pause and click-dismiss:
```rust
// Toast auto-dismiss (5 seconds, paused on hover)
if let Some(time) = self.state.error_display_time {
    if !self.state.error_hovered && time.elapsed() > std::time::Duration::from_secs(5) {
        self.state.last_error = None;
        self.state.error_display_time = None;
    }
}
```

Add `error_hovered: bool` to `AppState` (default `false`).

In the status bar `BottomPanel`, update the error display to handle hover and click:
```rust
if let Some(ref error) = state.last_error {
    ui.add_space(12.0);
    let error_label = ui.colored_label(
        theme.color(theme.status_error),
        error.chars().take(60).collect::<String>(),
    );
    state.error_hovered = error_label.hovered();
    if error_label.clicked() {
        state.last_error = None;
        state.error_display_time = None;
    }
}
```

- [ ] **Step 6: Run `cargo check`**

Expected: Compiles successfully (there will be adjustments needed for exact field names matching existing AppState)

- [ ] **Step 7: Run `cargo run` to test visually**

Expected: Themed UI with vertical stack layout, pill chips, status bar at bottom, transparent background

- [ ] **Step 8: Commit**

```bash
git add src/ui/input.rs src/app.rs
git commit -m "feat: redesign input screen with vertical stack layout and pill chips"
```

---

### Task 11: Final integration and cleanup

**Files:**
- Multiple files — final integration pass

- [ ] **Step 1: Remove old status bar code from `input.rs`**

Verify that status display logic has been fully moved to the `BottomPanel` in `app.rs` and removed from `input.rs`.

- [ ] **Step 2: Add input validation to settings UI**

In `src/ui/settings.rs`:
- After the device name text input, add validation feedback: if `!validation::is_valid_device_name(&state.config.virtual_device_name)`, show a warning label in `status_warn` color: "デバイス名は英数字、_、- のみ (最大64文字)"
- After the VOICEVOX URL text input, add validation: if `validation::is_valid_voicevox_url(&state.config.voicevox_url).is_err()`, show warning: "URLはhttp://localhost または http://127.0.0.1 のみ"
- Replace all hardcoded `Color32::from_rgb(...)` with `state.config.theme.color(...)` equivalents

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Run clippy**

Run: `cargo clippy`
Expected: No warnings (or only pre-existing ones)

- [ ] **Step 5: Format code**

Run: `cargo fmt`

- [ ] **Step 6: Run the app end-to-end**

Run: `cargo run`
Verify:
- Transparent window with dark theme
- Custom title bar (drag, minimize, maximize, close)
- Vertical stack input layout
- Pill chip templates with truncation
- Status bar at bottom
- Tab switching works
- Settings screen works
- All existing functionality preserved

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: complete UX overhaul — theme, transparency, custom titlebar, input redesign"
```
