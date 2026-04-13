use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::types::{Speaker, Style, SynthParams};
use super::TtsEngine;

/// (display_name, filename) — reference audio files in the `reference/` directory.
pub const VOICEGER_EMOTIONS: &[(&str, &str)] = &[
    ("ノーマル", "01_ref_emoNormal026.wav"),
    ("甘え", "02_ref_emoAma026.wav"),
    ("ツン", "03_ref_emoTsun026.wav"),
    ("セクシー", "04_ref_emoSexy026.wav"),
    ("さっさ", "05_ref_emoSasa026.wav"),
    ("ぼそぼそ", "06_ref_emoMurmur026.wav"),
    ("ヒーロー", "07_ref_emoHero026.wav"),
    ("泣き", "08_ref_emoSobbing026.wav"),
];

/// (lang_code, display_name, style_id)
pub const VOICEGER_LANGUAGES: &[(&str, &str, u32)] = &[
    ("ja", "日本語", 0),
    ("en", "English", 1),
    ("zh", "中文", 2),
    ("ko", "한국어", 3),
    ("yue", "粤語", 4),
];

pub struct VoicegerEngine {
    client: Client,
    base_url: String,
    ref_audio_path: String,
    prompt_text: String,
    prompt_lang: String,
    /// Optional paths to Zundamon fine-tuned model weights.
    /// When set, they are loaded on the first successful health check.
    gpt_weights_path: Option<String>,
    sovits_weights_path: Option<String>,
    /// Tracks whether Zundamon weights have been loaded into the running server.
    weights_loaded: Arc<AtomicBool>,
}

impl VoicegerEngine {
    pub fn new(base_url: &str, ref_audio_path: &str, prompt_text: &str, prompt_lang: &str) -> Self {
        // Auto-detect Zundamon model weights relative to ref_audio_path.
        let base_dir = std::path::Path::new(ref_audio_path)
            .parent()
            .and_then(|p| p.parent());
        // Use the BASE GPT (T2S) model for text → semantic tokens (fine-tuned GPT causes EOS too early).
        // Use the Zundamon fine-tuned SoVITS model for semantic tokens → Zundamon's voice.
        let (gpt, sov) = if let Some(base) = base_dir {
            let g = base.join(
                "GPT-SoVITS/GPT_SoVITS/pretrained_models/gsv-v2final-pretrained/s1bert25hz-5kh-longer-epoch=12-step=369668.ckpt",
            );
            let s = base
                .join("GPT-SoVITS/zundamon_models/SoVITS_weights_v2/zudamon_style_1_e8_s96.pth");
            (
                g.exists().then(|| g.to_string_lossy().into_owned()),
                s.exists().then(|| s.to_string_lossy().into_owned()),
            )
        } else {
            (None, None)
        };

        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            ref_audio_path: ref_audio_path.to_string(),
            prompt_text: prompt_text.to_string(),
            prompt_lang: prompt_lang.to_string(),
            gpt_weights_path: gpt,
            sovits_weights_path: sov,
            weights_loaded: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Load Zundamon fine-tuned weights into the running server.
    pub async fn load_zundamon_weights(&self) {
        use reqwest::Url;
        for (endpoint, path_opt) in [
            ("/set_gpt_weights", &self.gpt_weights_path),
            ("/set_sovits_weights", &self.sovits_weights_path),
        ] {
            if let Some(path) = path_opt {
                let url = format!("{}{}", self.base_url, endpoint);
                if let Ok(mut u) = Url::parse(&url) {
                    u.query_pairs_mut().append_pair("weights_path", path);
                    let _ = self.client.get(u).send().await;
                }
            }
        }
    }

    /// Apply pitch and volume adjustments to a WAV via ffmpeg.
    fn ffmpeg_adjust(wav: &[u8], pitch_scale: f64, volume_scale: f64) -> Result<Vec<u8>> {
        use std::io::Write;
        use std::process::Command;

        let tmp_in = std::env::temp_dir().join(format!(
            "zundux_vgr_in_{}.wav",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        let tmp_out = tmp_in.with_extension("out.wav");
        {
            let mut f = std::fs::File::create(&tmp_in).context("Failed to create ffmpeg input")?;
            f.write_all(wav).context("Failed to write ffmpeg input")?;
        }

        // Build filter chain: pitch via asetrate+aresample, volume multiplier.
        let mut filters = Vec::new();
        if (pitch_scale - 0.0).abs() > 1e-4 {
            // pitch_scale range -0.15..0.15 → treat each unit as 1 semitone shift
            // rate multiplier = 2^(pitch_scale)
            let rate_factor = 2.0_f64.powf(pitch_scale);
            filters.push(format!("asetrate=32000*{:.6},aresample=32000", rate_factor));
        }
        if (volume_scale - 1.0).abs() > 1e-4 {
            filters.push(format!("volume={:.4}", volume_scale));
        }
        let filter_str = filters.join(",");

        let status = Command::new("ffmpeg")
            .args(["-y", "-i"])
            .arg(&tmp_in)
            .args(["-af", &filter_str, "-loglevel", "error"])
            .arg(&tmp_out)
            .status()
            .context("Failed to spawn ffmpeg for pitch/volume")?;

        let _ = std::fs::remove_file(&tmp_in);

        if !status.success() {
            let _ = std::fs::remove_file(&tmp_out);
            anyhow::bail!("ffmpeg exited with {}", status);
        }

        let result = std::fs::read(&tmp_out).context("Failed to read ffmpeg output")?;
        let _ = std::fs::remove_file(&tmp_out);
        Ok(result)
    }

    /// Map a speaker_id (from SynthParams) to a Voiceger language code.
    pub fn lang_for_speaker_id(speaker_id: u32) -> &'static str {
        VOICEGER_LANGUAGES
            .iter()
            .find(|(_, _, id)| *id == speaker_id)
            .map(|(code, _, _)| *code)
            .unwrap_or("ja")
    }

    /// Very short ASCII-only input tends to echo/bleed reference speech in some GPT-SoVITS setups.
    /// Very short Japanese snippets can also be unstable with reference mode.
    /// Use ref-free mode for those cases.
    fn should_auto_ref_free(text: &str) -> bool {
        let t = text.trim();
        if t.is_empty() {
            return false;
        }
        let ascii_alpha_count = t.chars().filter(|c| c.is_ascii_alphabetic()).count();
        // Allow common short-token punctuation users often type in chat snippets.
        // Restricting to this small set keeps auto-ref-free targeted and predictable.
        let ascii_only = t.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || c.is_ascii_whitespace()
                || matches!(c, '-' | '_' | '.' | ',' | '!' | '?')
        });
        if ascii_only && ascii_alpha_count > 0 && ascii_alpha_count <= 3 {
            return true;
        }

        // Short Japanese fragments (e.g. 「いいかな？」) are often more stable without reference audio.
        // Count only meaningful chars (ignore spaces and punctuation).
        let mut jp_core_len = 0usize;
        let mut has_japanese = false;
        for c in t.chars() {
            if c.is_whitespace()
                || matches!(
                    c,
                    '。' | '、'
                        | '！'
                        | '？'
                        | '…'
                        | 'ー'
                        | '〜'
                        | '～'
                        | '・'
                        | '「'
                        | '」'
                        | '『'
                        | '』'
                        | '（'
                        | '）'
                        | '('
                        | ')'
                        | '.'
                        | ','
                        | '!'
                        | '?'
                )
            {
                continue;
            }
            let is_japanese = matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}');
            has_japanese |= is_japanese;
            jp_core_len += 1;
        }

        has_japanese && jp_core_len > 0 && jp_core_len <= 8
    }

    /// Returns true when whole text or any sentence-like clause should be synthesized in ref-free mode.
    fn should_auto_ref_free_in_text(text: &str) -> bool {
        if Self::should_auto_ref_free(text) {
            return true;
        }

        let mut clause_start = 0usize;
        for (idx, ch) in text.char_indices() {
            if matches!(ch, '。' | '！' | '？' | '!' | '?' | '\n') {
                let end = idx + ch.len_utf8();
                if Self::should_auto_ref_free(&text[clause_start..end]) {
                    return true;
                }
                clause_start = end;
            }
        }

        clause_start < text.len() && Self::should_auto_ref_free(&text[clause_start..])
    }
}

#[async_trait]
impl TtsEngine for VoicegerEngine {
    async fn list_speakers(&self) -> Result<Vec<Speaker>> {
        let styles: Vec<Style> = VOICEGER_LANGUAGES
            .iter()
            .map(|(_, name, id)| Style {
                name: name.to_string(),
                id: *id,
            })
            .collect();

        Ok(vec![Speaker {
            name: "ずんだもん (Voiceger)".to_string(),
            speaker_uuid: "voiceger-zundamon".to_string(),
            styles,
        }])
    }

    async fn synthesize(&self, text: &str, params: &SynthParams) -> Result<Vec<u8>> {
        let text_lang = Self::lang_for_speaker_id(params.speaker_id);

        // Map intonation_scale (0–2, default 1) → temperature (0.5–1.5, default 1).
        // Higher intonation = higher temperature = more expressive generation.
        let temperature = (0.5 + params.intonation_scale * 0.5).clamp(0.1, 2.0);

        let ref_audio = params
            .aux_ref_audio
            .as_deref()
            .unwrap_or(&self.ref_audio_path);
        let ref_audio_available = !ref_audio.trim().is_empty();
        // Force ref_free when no reference audio is configured, or when text qualifies.
        let ref_free = !ref_audio_available
            || params.voiceger_ref_free
            || Self::should_auto_ref_free_in_text(text);

        let url = format!("{}/tts", self.base_url);
        // api_v2.py requires ref_audio_path and prompt_lang unconditionally.
        // ref_free mode is emulated by sending an empty prompt_text: TTS.py sets
        // no_prompt_text=True when prompt_text is empty, skipping reference audio
        // conditioning while still accepting the path for audio feature extraction.
        let effective_ref_audio = if ref_audio_available {
            ref_audio.to_string()
        } else {
            // Fallback: send the configured path even if empty; server will error
            // with a clearer message than a missing-field 400.
            self.ref_audio_path.clone()
        };
        let prompt_text_to_send = if ref_free {
            String::new()
        } else {
            self.prompt_text.clone()
        };
        let body = serde_json::json!({
            "text": text,
            "text_lang": text_lang,
            "ref_audio_path": effective_ref_audio,
            "prompt_text": prompt_text_to_send,
            "prompt_lang": &self.prompt_lang,
            "speed_factor": params.speed_scale,
            "temperature": temperature,
            "streaming_mode": false,
            "media_type": "wav",
        });

        tracing::info!(
            "Voiceger synthesize: text={:?} lang={} temp={:.2} ref={} ref_free={}",
            text,
            text_lang,
            temperature,
            ref_audio,
            ref_free
        );

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Voicegerサーバーへの接続に失敗しました")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            // Parse {"message":"...","Exception":"..."} for a readable error.
            let err_msg = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_text) {
                let msg = v["message"].as_str().unwrap_or("unknown error");
                if let Some(exc) = v["Exception"].as_str() {
                    // Show last non-empty line of traceback (most informative part).
                    let last = exc
                        .lines()
                        .rev()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or(exc);
                    tracing::error!("Voiceger {} exception:\n{}", status, exc);
                    format!("{}: {}", msg, last.trim())
                } else {
                    msg.to_string()
                }
            } else {
                body_text
            };
            anyhow::bail!("Voiceger {}: {}", status, err_msg);
        }

        let wav_bytes = resp
            .bytes()
            .await
            .context("Voicegerの音声レスポンスの読み取りに失敗しました")?;

        let wav = wav_bytes.to_vec();

        // Apply pitch and volume via ffmpeg if they differ from defaults.
        // pitch_scale: VOICEVOX range -0.15..0.15 → treat as semitone fraction
        //   (multiply by 12 to get semitones, e.g. 0.15 → ~1.8 semitones)
        // volume_scale: 1.0 = unity gain
        let needs_pitch = (params.pitch_scale - 0.0).abs() > 1e-4;
        let needs_volume = (params.volume_scale - 1.0).abs() > 1e-4;
        if needs_pitch || needs_volume {
            match Self::ffmpeg_adjust(&wav, params.pitch_scale, params.volume_scale) {
                Ok(adjusted) => return Ok(adjusted),
                Err(e) => tracing::warn!(
                    "ffmpeg pitch/volume adjustment failed ({}), using raw audio",
                    e
                ),
            }
        }

        Ok(wav)
    }

    async fn health_check(&self) -> Result<bool> {
        // The GPT-SoVITS API has no dedicated health endpoint — GET / returns 404.
        // Any HTTP response (even 4xx) means the server is running.
        let url = format!("{}/", self.base_url);
        match self.client.get(&url).send().await {
            Ok(_) => {
                // Load Zundamon weights only once per engine instance.
                if !self.weights_loaded.load(Ordering::Relaxed) {
                    self.load_zundamon_weights().await;
                    self.weights_loaded.store(true, Ordering::Relaxed);
                }
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_speakers_returns_one_speaker_with_five_styles() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = VoicegerEngine::new("http://localhost:9880", "", "", "ja");
        let speakers = rt.block_on(engine.list_speakers()).unwrap();

        assert_eq!(speakers.len(), 1);
        let speaker = &speakers[0];
        assert_eq!(speaker.styles.len(), 5);
    }

    #[test]
    fn style_ids_match_language_order() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = VoicegerEngine::new("http://localhost:9880", "", "", "ja");
        let speakers = rt.block_on(engine.list_speakers()).unwrap();
        let styles = &speakers[0].styles;

        assert_eq!(styles[0].id, 0); // ja
        assert_eq!(styles[1].id, 1); // en
        assert_eq!(styles[2].id, 2); // zh
        assert_eq!(styles[3].id, 3); // ko
        assert_eq!(styles[4].id, 4); // yue
    }

    #[test]
    fn lang_for_speaker_id_maps_correctly() {
        assert_eq!(VoicegerEngine::lang_for_speaker_id(0), "ja");
        assert_eq!(VoicegerEngine::lang_for_speaker_id(1), "en");
        assert_eq!(VoicegerEngine::lang_for_speaker_id(2), "zh");
        assert_eq!(VoicegerEngine::lang_for_speaker_id(3), "ko");
        assert_eq!(VoicegerEngine::lang_for_speaker_id(4), "yue");
    }

    #[test]
    fn lang_for_unknown_speaker_id_defaults_to_ja() {
        assert_eq!(VoicegerEngine::lang_for_speaker_id(99), "ja");
    }

    #[test]
    fn should_auto_ref_free_for_short_ascii_tokens() {
        assert!(VoicegerEngine::should_auto_ref_free("wa"));
        assert!(VoicegerEngine::should_auto_ref_free("ok!"));
        assert!(!VoicegerEngine::should_auto_ref_free("hello"));
        assert!(!VoicegerEngine::should_auto_ref_free(""));
    }

    #[test]
    fn should_auto_ref_free_for_short_japanese_snippets() {
        assert!(VoicegerEngine::should_auto_ref_free("いいかな？"));
        assert!(VoicegerEngine::should_auto_ref_free("ねえ、どう？"));
        assert!(!VoicegerEngine::should_auto_ref_free(
            "これは短文ではないので通常モードで読む"
        ));
    }

    #[test]
    fn should_auto_ref_free_when_long_text_contains_short_clause() {
        assert!(VoicegerEngine::should_auto_ref_free_in_text(
            "これは長文です。少し説明します。いいかな？ありがとう。"
        ));
        assert!(!VoicegerEngine::should_auto_ref_free_in_text(
            "これはそれなりに長い文で、短い挿入句もないため通常モードのままです。"
        ));
    }
}
