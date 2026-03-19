use anyhow::{Context, Result};
use std::process::{Child, Command, Stdio};

pub struct UrlPlayer {
    child: Option<Child>,
    pipeline_pids: Option<(u32, u32)>,
}

impl UrlPlayer {
    pub fn new() -> Self {
        Self {
            child: None,
            pipeline_pids: None,
        }
    }

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

    pub fn check_dependencies() -> (bool, bool) {
        let ytdlp = Command::new("yt-dlp")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let ffmpeg = Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        (ytdlp, ffmpeg)
    }

    pub fn play(&mut self, url: &str, device_name: &str) -> Result<()> {
        self.stop();

        // Validate device name
        if !crate::validation::is_valid_device_name(device_name) {
            anyhow::bail!("Invalid device name for playback: {}", device_name);
        }

        // Build pipeline: yt-dlp | ffmpeg | paplay
        // Each process gets arguments directly — no shell involved
        let mut ytdlp = Command::new("yt-dlp")
            .args(["-o", "-", "-f", "bestaudio", url])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start yt-dlp")?;

        let ytdlp_stdout = ytdlp
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture yt-dlp stdout"))?;

        let mut ffmpeg = Command::new("ffmpeg")
            .args([
                "-i",
                "pipe:0",
                "-f",
                "wav",
                "-acodec",
                "pcm_s16le",
                "-ar",
                "24000",
                "-ac",
                "1",
                "pipe:1",
            ])
            .stdin(ytdlp_stdout)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start ffmpeg")?;

        let ffmpeg_stdout = ffmpeg
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture ffmpeg stdout"))?;

        let paplay = Command::new("paplay")
            .args(["--device", device_name])
            .stdin(ffmpeg_stdout)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start paplay")?;

        // Store the final process (paplay) as the child to manage
        self.pipeline_pids = Some((ytdlp.id(), ffmpeg.id()));
        self.child = Some(paplay);

        // Detach yt-dlp and ffmpeg — they'll be killed when paplay's stdin closes
        std::thread::spawn(move || {
            let _ = ytdlp.wait();
        });
        std::thread::spawn(move || {
            let _ = ffmpeg.wait();
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Kill pipeline processes if still running
        if let Some((ytdlp_pid, ffmpeg_pid)) = self.pipeline_pids.take() {
            for pid in [ytdlp_pid, ffmpeg_pid] {
                let _ = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .status();
            }
        }
    }
}

impl Drop for UrlPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}
