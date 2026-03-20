use anyhow::{bail, Context, Result};

const MAX_DEVICE_NAME_LEN: usize = 64;
pub const DEFAULT_DEVICE_NAME: &str = "ZunduxMic";
pub const MAX_CONFIG_FILE_SIZE: u64 = 1_048_576; // 1 MB

/// Validate PulseAudio device name: `[a-zA-Z0-9_-]+`, max 64 chars.
#[must_use]
pub fn is_valid_device_name(name: &str) -> bool {
    !name.is_empty()
        // .len() is byte count; safe here because the allowlist is ASCII-only
        && name.len() <= MAX_DEVICE_NAME_LEN
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub const DEFAULT_VOICEVOX_URL: &str = "http://127.0.0.1:50021";

/// Validate voicevox_url: must be http, host must be localhost/127.0.0.1/[::1].
pub fn is_valid_voicevox_url(url_str: &str) -> Result<()> {
    let parsed = url::Url::parse(url_str).map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;
    if parsed.scheme() != "http" {
        bail!(
            "VOICEVOX URL must use http scheme, got: {}",
            parsed.scheme()
        );
    }
    match parsed.host_str() {
        Some("127.0.0.1") | Some("localhost") | Some("[::1]") => Ok(()),
        Some(host) => bail!("VOICEVOX URL must point to localhost, got: {}", host),
        None => bail!("VOICEVOX URL has no host"),
    }
}

/// Validate PulseAudio source/sink name from pactl output.
/// Allows `[a-zA-Z0-9_.@:-]+`, max 256 chars.
/// More permissive than `is_valid_device_name` because PA names contain dots, colons, etc.
#[must_use]
pub fn is_valid_pa_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '@'))
}

pub fn check_config_file_size(path: &std::path::Path) -> Result<()> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("reading metadata for '{}'", path.display()))?;
    if metadata.len() > MAX_CONFIG_FILE_SIZE {
        bail!(
            "Config file too large: {} bytes (max {} bytes)",
            metadata.len(),
            MAX_CONFIG_FILE_SIZE
        );
    }
    Ok(())
}
