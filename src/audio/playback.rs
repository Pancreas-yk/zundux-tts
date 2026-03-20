use anyhow::{Context, Result};
use rodio::{OutputStream, Sink};
use std::io::Cursor;
use std::process::Command;

/// Try to play WAV data through rodio by finding the virtual device via cpal.
/// Falls back to paplay subprocess if rodio can't find the device.
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

    match play_with_rodio_cancellable(&wav_data, device_name, &cancel) {
        Ok(()) => Ok(()),
        Err(e) => {
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                return Ok(());
            }
            tracing::warn!("rodio playback failed ({}), falling back to paplay", e);
            play_with_paplay(&wav_data, device_name, &cancel)
        }
    }
}

/// Play WAV data on the default output device with cancel support.
fn play_on_default_output_cancellable(
    wav_data: &[u8],
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let (_stream, handle) = OutputStream::try_default().context("Failed to open default output")?;
    let sink = Sink::try_new(&handle).context("Failed to create sink for monitor")?;
    let cursor = Cursor::new(wav_data.to_vec());
    let source = rodio::Decoder::new(cursor).context("Failed to decode WAV for monitor")?;
    sink.append(source);
    while !sink.empty() {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            sink.stop();
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

fn play_with_rodio_cancellable(
    wav_data: &[u8],
    device_name: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};

    let host = rodio::cpal::default_host();
    let target_device = host
        .output_devices()
        .context("Failed to enumerate output devices")?
        .find(|d| {
            d.name()
                .map(|n| n.contains(device_name))
                .unwrap_or(false)
        })
        .context("Virtual device not found in cpal devices")?;

    let (_stream, handle) =
        OutputStream::try_from_device(&target_device).context("Failed to open output stream")?;
    let sink = Sink::try_new(&handle).context("Failed to create sink")?;

    let cursor = Cursor::new(wav_data.to_vec());
    let source = rodio::Decoder::new(cursor).context("Failed to decode WAV data")?;
    sink.append(source);
    while !sink.empty() {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            sink.stop();
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(())
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
) -> Result<()> {
    if monitor {
        let path_clone = path.to_path_buf();
        let cancel_clone = cancel.clone();
        std::thread::spawn(move || {
            if let Err(e) = play_file_default_output(&path_clone, &cancel_clone) {
                // Don't log if cancelled intentionally
                if !cancel_clone.load(std::sync::atomic::Ordering::SeqCst) {
                    tracing::warn!("Monitor playback failed: {}", e);
                }
            }
        });
    }

    // Use ffmpeg to decode any format → raw PCM, then pipe to paplay
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-i"])
        .arg(path)
        .args([
            "-f", "s16le",
            "-acodec", "pcm_s16le",
            "-ac", "1",
            "-ar", "48000",
            "-loglevel", "error",
            "pipe:1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn ffmpeg — is ffmpeg installed?")?;

    let ffmpeg_stdout = ffmpeg.stdout.take().context("Failed to get ffmpeg stdout")?;

    let mut paplay = Command::new("paplay")
        .args([
            "--device", device_name,
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
    let mut pid_list = pids.lock().unwrap();
    for &pid in pid_list.iter() {
        if pid == 0 {
            continue;
        }
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
    pid_list.clear();
}

fn play_file_default_output(
    path: &std::path::Path,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let (_stream, handle) =
        OutputStream::try_default().context("Failed to open default output")?;
    let sink = Sink::try_new(&handle).context("Failed to create sink")?;
    let file = std::fs::File::open(path).context("Failed to open audio file")?;
    let source = rodio::Decoder::new(std::io::BufReader::new(file))
        .context("Failed to decode audio file")?;
    sink.append(source);
    // Poll instead of sleep_until_end so we can respond to cancel
    while !sink.empty() {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            sink.stop();
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

fn play_with_paplay(
    wav_data: &[u8],
    device_name: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("paplay")
        .args(["--device", device_name, "--raw", "--format=s16le", "--rate=24000", "--channels=1"])
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to spawn paplay")?;

    // Strip WAV header (44 bytes) to get raw PCM for paplay --raw
    let pcm_data = if wav_data.len() > 44 && &wav_data[0..4] == b"RIFF" {
        &wav_data[44..]
    } else {
        wav_data
    };

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(pcm_data).context("Failed to write to paplay stdin")?;
    }
    drop(child.stdin.take());

    // Poll for cancel while waiting for paplay to finish
    loop {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            tracing::info!("TTS stop: killing paplay (pid {})", child.id());
            let _ = child.kill(); // SIGKILL — more reliable than SIGTERM on PipeWire
            let _ = child.wait();
            return Ok(());
        }
        match child.try_wait().context("Failed to wait for paplay")? {
            Some(status) => {
                if !status.success() {
                    if let Some(code) = status.code() {
                        anyhow::bail!("paplay exited with status {}", code);
                    }
                }
                return Ok(());
            }
            None => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}
