use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AndroidDeviceRef {
    pub serial: String,
    pub name: Option<String>,
    pub is_emulator: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidPointPx {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidSizePx {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidRectPx {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl AndroidRectPx {
    pub fn center(self) -> AndroidPointPx {
        AndroidPointPx {
            x: (self.left + self.right) / 2.0,
            y: (self.top + self.bottom) / 2.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidUiNode {
    pub text: Option<String>,
    pub resource_id: Option<String>,
    pub content_desc: Option<String>,
    pub class_name: String,
    pub package: Option<String>,
    pub clickable: bool,
    pub enabled: bool,
    pub visible: bool,
    pub bounds: AndroidRectPx,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidObservation {
    pub observation_id: String,
    pub device: Option<AndroidDeviceRef>,
    pub package: Option<String>,
    pub activity: Option<String>,
    pub screen: AndroidSizePx,
    #[serde(default)]
    pub visible_targets: Vec<AndroidUiNode>,
    pub screenshot_ref: Option<String>,
    pub ui_tree_ref: Option<String>,
    pub timestamp_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidVisibleTargetEvidence {
    pub id: String,
    pub label: String,
    pub text: Option<String>,
    pub resource_id: Option<String>,
    pub content_desc: Option<String>,
    pub clickable: bool,
    pub enabled: bool,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidActionEvidence {
    pub action: String,
    pub target: Option<String>,
    pub status: String,
    pub before_observation: Option<String>,
    pub after_observation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidToolEvidence {
    pub observation_id: String,
    pub package: Option<String>,
    pub activity: Option<String>,
    #[serde(default)]
    pub visible_targets: Vec<AndroidVisibleTargetEvidence>,
    pub action: Option<AndroidActionEvidence>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AndroidControlMode {
    Agent,
    Manual,
    Paused,
}

impl Default for AndroidControlMode {
    fn default() -> Self {
        Self::Agent
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AndroidActionPhase {
    Planned,
    MovingCursor,
    Pressing,
    WaitingForUi,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidActionVisualization {
    pub label: String,
    pub reason: Option<String>,
    pub confidence: Option<String>,
    pub target_bounds: Option<AndroidRectPx>,
    pub from: Option<AndroidPointPx>,
    pub to: Option<AndroidPointPx>,
    pub phase: AndroidActionPhase,
    pub actor: AndroidActionActor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AndroidActionActor {
    Agent,
    User,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidSessionState {
    pub device: Option<AndroidDeviceRef>,
    pub status: String,
    pub current_app: Option<String>,
    pub current_activity: Option<String>,
    pub control_mode: AndroidControlMode,
    pub latest_observation: Option<AndroidObservation>,
    pub current_action: Option<AndroidActionVisualization>,
    #[serde(default)]
    pub recent_actions: Vec<AndroidActionTrace>,
    pub active_journey: Option<AndroidJourney>,
}

impl Default for AndroidSessionState {
    fn default() -> Self {
        Self {
            device: None,
            status: "Not connected".into(),
            current_app: None,
            current_activity: None,
            control_mode: AndroidControlMode::Agent,
            latest_observation: None,
            current_action: None,
            recent_actions: Vec::new(),
            active_journey: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AndroidJourneyStatus {
    Planned,
    Running,
    Passed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidJourney {
    pub id: String,
    pub title: String,
    pub goal: String,
    #[serde(default)]
    pub steps: Vec<JourneyStep>,
    pub status: AndroidJourneyStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JourneyStep {
    pub instruction: String,
    pub expected_result: Option<String>,
    #[serde(default)]
    pub actions: Vec<AndroidActionTrace>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    pub result: StepResult,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepResult {
    pub status: AndroidJourneyStatus,
    pub summary: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidActionTrace {
    pub action_id: String,
    pub action: String,
    pub target: Option<String>,
    pub reason: Option<String>,
    pub confidence: Option<String>,
    pub before_observation: Option<String>,
    pub after_observation: Option<String>,
    pub settle: Option<UiSettleResult>,
    pub status: String,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSettleResult {
    pub stable: bool,
    pub duration_ms: u64,
    pub screenshot_changed: bool,
    pub tree_changed: bool,
    pub package_changed: bool,
    pub activity_changed: bool,
}
