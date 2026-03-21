use std::path::Path;
use std::process::Command;

/// Result of loudness analysis for a single audio file.
#[derive(Debug, Clone)]
pub struct LoudnessInfo {
    pub lufs: f64,
    pub peak_dbfs: f64,
}

#[derive(Debug)]
pub enum LoudnessError {
    DecodeFailed(String),
    InvalidFormat(String),
}

impl std::fmt::Display for LoudnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DecodeFailed(msg) => write!(f, "decode failed: {}", msg),
            Self::InvalidFormat(msg) => write!(f, "invalid format: {}", msg),
        }
    }
}

impl std::error::Error for LoudnessError {}

/// Biquad filter coefficients for second-order IIR filter.
struct BiquadCoeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

/// Biquad filter state.
struct BiquadState {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BiquadState {
    fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, coeffs: &BiquadCoeffs, input: f64) -> f64 {
        let output =
            coeffs.b0 * input + coeffs.b1 * self.x1 + coeffs.b2 * self.x2
                - coeffs.a1 * self.y1
                - coeffs.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

/// K-weighting pre-filter coefficients for 48kHz (ITU-R BS.1770).
/// High-shelf boost ~+4dB above 1.5kHz to model head-related transfer.
fn pre_filter_48k() -> BiquadCoeffs {
    BiquadCoeffs {
        b0: 1.53512485958697,
        b1: -2.69169618940638,
        b2: 1.19839281085285,
        a1: -1.69065929318241,
        a2: 0.73248077421585,
    }
}

/// K-weighting RLB (revised low-frequency B-curve) filter for 48kHz.
/// High-pass ~-3dB at 38Hz to de-emphasize sub-bass.
fn rlb_filter_48k() -> BiquadCoeffs {
    BiquadCoeffs {
        b0: 1.0,
        b1: -2.0,
        b2: 1.0,
        a1: -1.99004745483398,
        a2: 0.99007225036621,
    }
}

/// Decode an audio file to raw PCM samples (f64, mono, 48kHz) using ffmpeg.
fn decode_to_pcm(path: &Path) -> Result<Vec<f64>, LoudnessError> {
    let output = Command::new("ffmpeg")
        .args(["-i"])
        .arg(path)
        .args([
            "-f", "s16le", "-acodec", "pcm_s16le", "-ac", "1", "-ar", "48000", "-loglevel",
            "error", "pipe:1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| LoudnessError::DecodeFailed(format!("ffmpeg spawn failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LoudnessError::DecodeFailed(format!(
            "ffmpeg failed: {}",
            stderr.chars().take(200).collect::<String>()
        )));
    }

    if output.stdout.is_empty() {
        return Err(LoudnessError::InvalidFormat(
            "ffmpeg produced no output".to_string(),
        ));
    }

    let samples: Vec<f64> = output
        .stdout
        .chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample as f64 / i16::MAX as f64
        })
        .collect();

    Ok(samples)
}

/// Analyze the loudness of an audio file. Returns LUFS and peak dBFS.
///
/// Uses simplified (ungated) ITU-R BS.1770 K-weighting.
/// Sufficient for short sound effects typical in a soundboard.
pub fn analyze_loudness(path: &Path) -> Result<LoudnessInfo, LoudnessError> {
    let samples = decode_to_pcm(path)?;

    if samples.is_empty() {
        return Err(LoudnessError::InvalidFormat("no samples".to_string()));
    }

    // Apply K-weighting filters
    let pre = pre_filter_48k();
    let rlb = rlb_filter_48k();
    let mut pre_state = BiquadState::new();
    let mut rlb_state = BiquadState::new();

    let mut sum_sq = 0.0_f64;
    let mut peak = 0.0_f64;

    for &sample in &samples {
        peak = peak.max(sample.abs());
        let filtered = pre_state.process(&pre, sample);
        let filtered = rlb_state.process(&rlb, filtered);
        sum_sq += filtered * filtered;
    }

    let mean_sq = sum_sq / samples.len() as f64;
    let lufs = if mean_sq > 0.0 {
        -0.691 + 10.0 * mean_sq.log10()
    } else {
        -70.0 // silence
    };

    let peak_dbfs = if peak > 0.0 {
        20.0 * peak.log10()
    } else {
        -96.0
    };

    Ok(LoudnessInfo { lufs, peak_dbfs })
}

/// Calculate gain in dB needed to reach target LUFS from current LUFS.
pub fn calculate_gain_db(current_lufs: f64, target_lufs: f64) -> f64 {
    target_lufs - current_lufs
}
