use anyhow::{Context, Result};
use std::process::{Child, Command, Stdio};

pub struct UrlPlayer {
    child: Option<Child>,
}

impl UrlPlayer {
    pub fn new() -> Self {
        Self { child: None }
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
        let ytdlp = Command::new("yt-dlp").arg("--version")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().map(|s| s.success()).unwrap_or(false);
        let ffmpeg = Command::new("ffmpeg").arg("-version")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().map(|s| s.success()).unwrap_or(false);
        (ytdlp, ffmpeg)
    }

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
