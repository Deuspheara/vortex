use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceProvenance {
    pub source_url: Option<String>,
    pub final_url: Option<String>,
    pub provider: String,
    pub fetched_at: DateTime<Utc>,
    pub extraction_mode: String,
}

impl SourceProvenance {
    pub fn new(
        source_url: Option<String>,
        final_url: Option<String>,
        provider: impl Into<String>,
        extraction_mode: impl Into<String>,
    ) -> Self {
        Self {
            source_url,
            final_url,
            provider: provider.into(),
            fetched_at: Utc::now(),
            extraction_mode: extraction_mode.into(),
        }
    }
}
