/// Apply echo effect to WAV data.
/// Expects standard WAV format (RIFF header).
/// Returns new WAV bytes with echo applied.
pub fn apply_echo(wav_data: &[u8], delay_ms: u32, decay: f64) -> Vec<u8> {
    if wav_data.len() <= 44 || &wav_data[0..4] != b"RIFF" {
        return wav_data.to_vec();
    }

    let sample_rate = u32::from_le_bytes([wav_data[24], wav_data[25], wav_data[26], wav_data[27]]);
    let bits_per_sample = u16::from_le_bytes([wav_data[34], wav_data[35]]);

    if bits_per_sample != 16 {
        return wav_data.to_vec();
    }

    let header = &wav_data[..44];
    let pcm_data = &wav_data[44..];

    let mut samples: Vec<i16> = pcm_data
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    let delay_samples = (sample_rate as usize * delay_ms as usize) / 1000;

    // Extend samples so echo tail can decay naturally.
    // Number of extra repeats until echo is inaudible (below -60dB).
    let repeats = if decay > 0.0 && decay < 1.0 {
        (-60.0_f64 / (20.0 * decay.log10())).ceil() as usize
    } else {
        0
    };
    let tail_len = delay_samples * repeats;
    samples.resize(samples.len() + tail_len, 0);

    for i in delay_samples..samples.len() {
        let echo = (samples[i - delay_samples] as f64 * decay) as i64;
        let mixed = samples[i] as i64 + echo;
        samples[i] = mixed.clamp(i16::MIN as i64, i16::MAX as i64) as i16;
    }

    let mut result = header.to_vec();
    for sample in &samples {
        result.extend_from_slice(&sample.to_le_bytes());
    }

    let data_size = (samples.len() * 2) as u32;
    result[40..44].copy_from_slice(&data_size.to_le_bytes());
    let riff_size = (result.len() - 8) as u32;
    result[4..8].copy_from_slice(&riff_size.to_le_bytes());

    result
}
