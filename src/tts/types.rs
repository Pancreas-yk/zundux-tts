use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Speaker {
    pub name: String,
    pub speaker_uuid: String,
    pub styles: Vec<Style>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    pub name: String,
    pub id: u32,
}

#[derive(Debug, Clone)]
pub struct SynthParams {
    pub speaker_id: u32,
    pub speed_scale: f64,
    pub pitch_scale: f64,
    pub intonation_scale: f64,
    pub volume_scale: f64,
    /// Overrides the engine's default reference audio path (Voiceger only).
    pub aux_ref_audio: Option<String>,
    /// Voiceger only: force reference-free synthesis (`ref_free=true`).
    pub voiceger_ref_free: bool,
}

impl SynthParams {
    pub fn from_config(config: &crate::config::AppConfig) -> Self {
        Self {
            speaker_id: config.speaker_id,
            speed_scale: config.synth_params.speed_scale,
            pitch_scale: config.synth_params.pitch_scale,
            intonation_scale: config.synth_params.intonation_scale,
            volume_scale: config.synth_params.volume_scale,
            aux_ref_audio: None,
            voiceger_ref_free: config.voiceger_ref_free,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDictWord {
    pub surface: String,
    pub pronunciation: String,
    pub accent_type: u32,
}

pub type UserDict = HashMap<String, UserDictWord>;
