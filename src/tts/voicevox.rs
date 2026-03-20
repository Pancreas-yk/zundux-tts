use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

use super::TtsEngine;
use super::types::{Speaker, SynthParams, UserDict};

pub struct VoicevoxEngine {
    client: Client,
    base_url: String,
}

impl VoicevoxEngine {
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn list_user_dict(&self) -> Result<UserDict> {
        let url = format!("{}/user_dict", self.base_url);
        let resp = self.client.get(&url).send().await
            .context("Failed to fetch user dictionary")?;
        let dict: UserDict = resp.json().await
            .context("Failed to parse user dictionary")?;
        Ok(dict)
    }

    pub async fn add_user_dict_word(&self, surface: &str, pronunciation: &str) -> Result<String> {
        let url = format!("{}/user_dict_word", self.base_url);
        let fullwidth_surface: String = surface.chars().map(halfwidth_to_fullwidth).collect();
        let katakana: String = pronunciation.chars().map(hiragana_to_katakana).collect();
        let resp = self.client.post(&url)
            .query(&[
                ("surface", fullwidth_surface.as_str()),
                ("pronunciation", katakana.as_str()),
                ("accent_type", "1"),
            ])
            .send().await
            .context("Failed to add dictionary word")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("VOICEVOX returned {} : {}", status, body);
        }
        let uuid: String = resp.json().await
            .context("Failed to parse add word response")?;
        Ok(uuid)
    }

    pub async fn delete_user_dict_word(&self, word_uuid: &str) -> Result<()> {
        let url = format!("{}/user_dict_word/{}", self.base_url, word_uuid);
        let resp = self.client.delete(&url).send().await
            .context("Failed to delete dictionary word")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("VOICEVOX returned {} : {}", status, body);
        }
        Ok(())
    }
}

#[async_trait]
impl TtsEngine for VoicevoxEngine {
    async fn list_speakers(&self) -> Result<Vec<Speaker>> {
        let url = format!("{}/speakers", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to VOICEVOX")?;
        let speakers: Vec<Speaker> = resp.json().await.context("Failed to parse speakers")?;
        Ok(speakers)
    }

    async fn synthesize(&self, text: &str, params: &SynthParams) -> Result<Vec<u8>> {
        // Convert half-width ASCII to full-width so VOICEVOX user dictionary entries match
        let fullwidth_text: String = text.chars().map(halfwidth_to_fullwidth).collect();
        // Step 1: Create audio query
        let query_url = format!("{}/audio_query", self.base_url);
        let resp = self
            .client
            .post(&query_url)
            .query(&[("text", fullwidth_text.as_str()), ("speaker", &params.speaker_id.to_string())])
            .send()
            .await
            .context("Failed to create audio query")?;

        let mut query: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse audio query response")?;

        // Step 2: Apply parameter overrides
        if let Some(obj) = query.as_object_mut() {
            obj.insert(
                "speedScale".to_string(),
                serde_json::json!(params.speed_scale),
            );
            obj.insert(
                "pitchScale".to_string(),
                serde_json::json!(params.pitch_scale),
            );
            obj.insert(
                "intonationScale".to_string(),
                serde_json::json!(params.intonation_scale),
            );
            obj.insert(
                "volumeScale".to_string(),
                serde_json::json!(params.volume_scale),
            );
        }

        // Step 3: Synthesize audio
        let synth_url = format!("{}/synthesis", self.base_url);
        let wav_bytes = self
            .client
            .post(&synth_url)
            .query(&[("speaker", &params.speaker_id.to_string())])
            .json(&query)
            .send()
            .await
            .context("Failed to synthesize audio")?
            .bytes()
            .await
            .context("Failed to read synthesis response")?;

        Ok(wav_bytes.to_vec())
    }

    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/version", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

/// Convert a hiragana character to katakana. Non-hiragana characters are passed through.
fn hiragana_to_katakana(c: char) -> char {
    // Hiragana range: U+3041..=U+3096, katakana offset: +0x60
    if ('\u{3041}'..='\u{3096}').contains(&c) {
        char::from_u32(c as u32 + 0x60).unwrap_or(c)
    } else {
        c
    }
}

/// Convert half-width ASCII to full-width equivalents for VOICEVOX dictionary matching.
fn halfwidth_to_fullwidth(c: char) -> char {
    match c {
        ' ' => '\u{3000}', // full-width space
        '!' ..= '~' => {
            // ASCII printable range U+0021..U+007E → full-width U+FF01..U+FF5E
            char::from_u32(c as u32 + 0xFEE0).unwrap_or(c)
        }
        _ => c,
    }
}
