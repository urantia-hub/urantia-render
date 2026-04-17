use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct AudioEntry {
    pub format: Option<String>,
    pub url: Option<String>,
    pub duration: Option<f64>,
    pub bitrate: Option<u32>,
    #[serde(rename = "fileSize")]
    pub file_size: Option<u64>,
}

// Manifest structure: globalId -> model -> voice -> AudioEntry
type RawManifest = HashMap<String, HashMap<String, HashMap<String, AudioEntry>>>;

pub struct AudioManifest {
    data: RawManifest,
}

impl AudioManifest {
    pub fn from_json(json_str: &str) -> Result<Self> {
        let data: RawManifest = serde_json::from_str(json_str)?;
        Ok(Self { data })
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let json_str = std::fs::read_to_string(path)?;
        Self::from_json(&json_str)
    }

    /// Get the nova TTS duration for a globalId
    pub fn get_duration(&self, global_id: &str) -> Option<f64> {
        self.data
            .get(global_id)?
            .get(crate::config::AUDIO_MODEL)?
            .get(crate::config::AUDIO_VOICE)?
            .duration
    }

    pub fn entry_count(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
impl AudioManifest {
    /// Build a test AudioManifest from a map of globalId → duration (seconds).
    /// Only the duration is populated; other AudioEntry fields are None.
    pub fn from_durations_for_test(durations: HashMap<String, f64>) -> Self {
        let mut data: HashMap<String, HashMap<String, HashMap<String, AudioEntry>>> =
            HashMap::new();
        for (global_id, dur) in durations {
            let mut voice_map = HashMap::new();
            voice_map.insert(
                crate::config::AUDIO_VOICE.to_string(),
                AudioEntry {
                    format: None,
                    url: None,
                    duration: Some(dur),
                    bitrate: None,
                    file_size: None,
                },
            );
            let mut model_map = HashMap::new();
            model_map.insert(crate::config::AUDIO_MODEL.to_string(), voice_map);
            data.insert(global_id, model_map);
        }
        Self { data }
    }
}
