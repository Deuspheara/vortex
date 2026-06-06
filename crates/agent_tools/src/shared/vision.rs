use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisionInspectRequest {
    pub image_path: String,
    pub question: String,
    pub context: Option<Value>,
    pub output_schema: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisionObservation {
    pub label: String,
    pub value: String,
    pub evidence: Option<String>,
    pub confidence: String,
    pub bbox: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisionInspectResponse {
    pub summary: String,
    pub observations: Vec<VisionObservation>,
    pub uncertain: Vec<String>,
    pub suggested_next_action: Option<String>,
}

#[async_trait]
#[allow(dead_code)]
pub trait VisionPort: Send + Sync {
    async fn inspect(&self, request: VisionInspectRequest)
    -> Result<VisionInspectResponse, String>;
}

pub struct UnconfiguredVisionPort;

#[async_trait]
impl VisionPort for UnconfiguredVisionPort {
    async fn inspect(
        &self,
        _request: VisionInspectRequest,
    ) -> Result<VisionInspectResponse, String> {
        Err("no vision backend configured".into())
    }
}

#[cfg(test)]
pub struct FakeVisionPort;

#[cfg(test)]
#[async_trait]
impl VisionPort for FakeVisionPort {
    async fn inspect(
        &self,
        request: VisionInspectRequest,
    ) -> Result<VisionInspectResponse, String> {
        Ok(VisionInspectResponse {
            summary: format!("inspected {}", request.image_path),
            observations: vec![VisionObservation {
                label: "question".into(),
                value: request.question,
                evidence: None,
                confidence: "high".into(),
                bbox: None,
            }],
            uncertain: Vec::new(),
            suggested_next_action: None,
        })
    }
}
