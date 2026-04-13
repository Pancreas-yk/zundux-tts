use anyhow::{Context, Result};
use std::process::Command;

/// Play WAV data to the virtual sink using paplay.
/// If `monitor` is true, also plays to the default output device for self-monitoring.
/// `cancel` can be used to stop playback early.
pub fn play_wav(
    wav_data: Vec<u8>,
    device_name: &str,
    monitor: bool,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    if monitor {
        let data_clone = wav_data.clone();
        let cancel_clone = cancel.clone();
        std::thread::spawn(move || {
            if let Err(e) = play_on_default_output_cancellable(&data_clone, &cancel_clone) {
                if !cancel_clone.load(std::sync::atomic::Ordering::SeqCst) {
                    tracing::warn!("Monitor playback failed: {}", e);
                }
            }
        });
    }

    tracing::info!(
        "play_wav: {} bytes → device={:?}",
        wav_data.len(),
        device_name
    );
    let result = play_with_paplay(&wav_data, device_name, &cancel);
    if let Err(ref pe) = result {
        tracing::error!("paplay playback failed: {}", pe);
    } else {
        tracing::info!("play_wav: paplay OK");
    }
    result
}

/// Play WAV data on the default output device with cancel support.
/// Uses paplay (no --device) so it hits the real speakers.
fn play_on_default_output_cancellable(
    wav_data: &[u8],
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    use std::io::Write;

    let tmp_path = std::env::temp_dir().join(format!(
        "zundux_monitor_{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    {
        let mut f =
            std::fs::File::create(&tmp_path).context("Failed to create monitor temp file")?;
        f.write_all(wav_data)
            .context("Failed to write monitor WAV")?;
    }

    let mut child = Command::new("paplay")
        .arg(&tmp_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn paplay for monitor")?;

    loop {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&tmp_path);
            return Ok(());
        }
        match child
            .try_wait()
            .context("Failed to wait for monitor paplay")?
        {
            Some(_) => {
                let _ = std::fs::remove_file(&tmp_path);
                return Ok(());
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
}

/// Play an audio file (WAV/MP3/OGG) through a PulseAudio device using ffmpeg+paplay.
/// Unlike play_wav, this handles arbitrary formats and sample rates.
/// `pids` is used to store child process IDs so they can be killed externally for stop.
/// `cancel` is checked by the monitor thread to stop monitor playback.
pub fn play_file(
    path: &std::path::Path,
    device_name: &str,
    monitor: bool,
    pids: std::sync::Arc<std::sync::Mutex<Vec<u32>>>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    gain_db: Option<f64>,
) -> Result<()> {
    if monitor {
        let path_clone = path.to_path_buf();
        let cancel_clone = cancel.clone();
        let monitor_gain_db = gain_db;
        std::thread::spawn(move || {
            if let Err(e) = play_file_default_output(&path_clone, &cancel_clone, monitor_gain_db) {
                // Don't log if cancelled intentionally
                if !cancel_clone.load(std::sync::atomic::Ordering::SeqCst) {
                    tracing::warn!("Monitor playback failed: {}", e);
                }
            }
        });
    }

    // Use ffmpeg to decode any format → raw PCM, then pipe to paplay
    let mut ffmpeg_cmd = Command::new("ffmpeg");
    ffmpeg_cmd.args(["-i"]).arg(path);

    // Apply gain via ffmpeg volume filter if specified
    if let Some(db) = gain_db {
        ffmpeg_cmd.args(["-af", &format!("volume={}dB", db)]);
    }

    ffmpeg_cmd.args([
        "-f",
        "s16le",
        "-acodec",
        "pcm_s16le",
        "-ac",
        "1",
        "-ar",
        "48000",
        "-loglevel",
        "error",
        "pipe:1",
    ]);

    let mut ffmpeg = ffmpeg_cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn ffmpeg — is ffmpeg installed?")?;

    let ffmpeg_stdout = ffmpeg
        .stdout
        .take()
        .context("Failed to get ffmpeg stdout")?;

    let mut paplay = Command::new("paplay")
        .args([
            "--device",
            device_name,
            "--raw",
            "--format=s16le",
            "--rate=48000",
            "--channels=1",
        ])
        .stdin(ffmpeg_stdout)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn paplay")?;

    // Store PIDs so external code can kill them to stop playback
    {
        let mut pid_list = pids.lock().unwrap();
        pid_list.push(ffmpeg.id());
        pid_list.push(paplay.id());
    }

    let paplay_status = paplay.wait().context("Failed to wait for paplay")?;
    let _ = ffmpeg.wait();

    // Clear PIDs after completion
    {
        let mut pid_list = pids.lock().unwrap();
        pid_list.clear();
    }

    if !paplay_status.success() {
        // Don't report error if killed by signal (SIGTERM/SIGKILL = stopped by user)
        if let Some(code) = paplay_status.code() {
            anyhow::bail!("paplay exited with status {}", code);
        }
    }
    Ok(())
}

/// Kill active soundboard playback processes and cancel monitor.
pub fn stop_file_playback(
    pids: &std::sync::Arc<std::sync::Mutex<Vec<u32>>>,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    let pid_list = pids.lock().unwrap();
    for &pid in pid_list.iter() {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}

fn play_file_default_output(
    path: &std::path::Path,
    cancel: &std::sync::atomic::AtomicBool,
    gain_db: Option<f64>,
) -> Result<()> {
    let mut ffmpeg_cmd = Command::new("ffmpeg");
    ffmpeg_cmd.args(["-i"]).arg(path);

    if let Some(db) = gain_db {
        ffmpeg_cmd.args(["-af", &format!("volume={}dB", db)]);
    }

    ffmpeg_cmd.args([
        "-f",
        "s16le",
        "-acodec",
        "pcm_s16le",
        "-ac",
        "1",
        "-ar",
        "48000",
        "-loglevel",
        "error",
        "pipe:1",
    ]);

    let ffmpeg = ffmpeg_cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();

    match ffmpeg {
        Ok(mut ffmpeg_proc) => {
            let ffmpeg_stdout = ffmpeg_proc
                .stdout
                .take()
                .context("Failed to get ffmpeg stdout")?;
            let mut paplay = Command::new("paplay")
                .args(["--raw", "--format=s16le", "--rate=48000", "--channels=1"])
                .stdin(ffmpeg_stdout)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("Failed to spawn paplay for monitor")?;

            loop {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = paplay.kill();
                    let _ = ffmpeg_proc.kill();
                    let _ = paplay.wait();
                    let _ = ffmpeg_proc.wait();
                    return Ok(());
                }

                match paplay
                    .try_wait()
                    .context("Failed to wait for monitor paplay")?
                {
                    Some(status) => {
                        let _ = ffmpeg_proc.wait();
                        if !status.success() {
                            if let Some(code) = status.code() {
                                anyhow::bail!("paplay exited with status {}", code);
                            }
                        }
                        return Ok(());
                    }
                    None => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
        }
        Err(_) => {
            tracing::warn!("ffmpeg not found, using direct paplay for monitor");
            let mut child = Command::new("paplay")
                .arg(path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("Failed to spawn paplay for monitor")?;

            loop {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(());
                }
                match child
                    .try_wait()
                    .context("Failed to wait for monitor paplay")?
                {
                    Some(status) => {
                        if !status.success() {
                            if let Some(code) = status.code() {
                                anyhow::bail!("paplay exited with status {}", code);
                            }
                        }
                        return Ok(());
                    }
                    None => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
        }
    }
}

fn play_with_paplay(
    wav_data: &[u8],
    device_name: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    use std::io::Write;

    // Write WAV to a temp file (paplay doesn't support stdin).
    let tmp_path = std::env::temp_dir().join(format!(
        "zundux_tts_{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    {
        let mut f = std::fs::File::create(&tmp_path).context("Failed to create temp WAV file")?;
        f.write_all(wav_data)
            .context("Failed to write WAV to temp file")?;
    }

    // Use ffmpeg → paplay pipeline so any sample rate / format is normalised to
    // 48kHz s16le before hitting the virtual sink.  This matches how play_file
    // works and avoids PipeWire rejecting non-48kHz PCM on the null sink.
    let ffmpeg = Command::new("ffmpeg")
        .args(["-i"])
        .arg(&tmp_path)
        .args([
            "-f",
            "s16le",
            "-acodec",
            "pcm_s16le",
            "-ac",
            "1",
            "-ar",
            "48000",
            "-loglevel",
            "error",
            "pipe:1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();

    match ffmpeg {
        Ok(mut ffmpeg_proc) => {
            let ffmpeg_stdout = ffmpeg_proc
                .stdout
                .take()
                .context("Failed to get ffmpeg stdout")?;
            let mut paplay = Command::new("paplay")
                .args([
                    "--device",
                    device_name,
                    "--raw",
                    "--format=s16le",
                    "--rate=48000",
                    "--channels=1",
                ])
                .stdin(ffmpeg_stdout)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("Failed to spawn paplay")?;

            loop {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    tracing::info!("TTS stop: killing paplay+ffmpeg");
                    let _ = paplay.kill();
                    let _ = ffmpeg_proc.kill();
                    let _ = paplay.wait();
                    let _ = ffmpeg_proc.wait();
                    let _ = std::fs::remove_file(&tmp_path);
                    return Ok(());
                }
                match paplay.try_wait().context("Failed to wait for paplay")? {
                    Some(status) => {
                        let _ = ffmpeg_proc.wait();
                        let _ = std::fs::remove_file(&tmp_path);
                        if !status.success() {
                            if let Some(code) = status.code() {
                                anyhow::bail!("paplay exited with status {}", code);
                            }
                        }
                        return Ok(());
                    }
                    None => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
        }
        Err(_) => {
            // ffmpeg not available — fall back to direct paplay (original behaviour).
            tracing::warn!("ffmpeg not found, using direct paplay (format mismatch possible)");
            let mut child = Command::new("paplay")
                .args(["--device", device_name])
                .arg(&tmp_path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("Failed to spawn paplay")?;

            loop {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&tmp_path);
                    return Ok(());
                }
                match child.try_wait().context("Failed to wait for paplay")? {
                    Some(status) => {
                        let _ = std::fs::remove_file(&tmp_path);
                        if !status.success() {
                            if let Some(code) = status.code() {
                                anyhow::bail!("paplay exited with status {}", code);
                            }
                        }
                        return Ok(());
                    }
                    None => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
        }
    }
}
