pub mod adb_snapshot_driver;
pub mod android_cli_driver;
pub mod android_sdk;
pub mod emulator_manager;

use agent_protocol::{AndroidObservation, AndroidPointPx};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use adb_snapshot_driver::AdbSnapshotDriver;
pub use android_cli_driver::AndroidCliDriver;
pub use android_sdk::{AndroidSdkPaths, discover_android_sdk_paths};
pub use emulator_manager::{AndroidDeviceListing, EmulatorManager};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidKey {
    Back,
    Home,
    Enter,
}

impl AndroidKey {
    pub fn adb_keyevent(self) -> &'static str {
        match self {
            Self::Back => "KEYCODE_BACK",
            Self::Home => "KEYCODE_HOME",
            Self::Enter => "KEYCODE_ENTER",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverActionResult {
    pub success: bool,
    pub summary: String,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogcatFilter {
    pub package: Option<String>,
    pub max_lines: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogcatResult {
    pub output: String,
    pub truncated: bool,
}

#[async_trait]
pub trait AndroidDeviceDriver: Send + Sync {
    async fn observe(&self) -> Result<AndroidObservation, String>;
    async fn tap(&self, point: AndroidPointPx) -> Result<DriverActionResult, String>;
    async fn type_text(&self, text: &str, sensitive: bool) -> Result<DriverActionResult, String>;
    async fn swipe(
        &self,
        from: AndroidPointPx,
        to: AndroidPointPx,
        duration_ms: u64,
    ) -> Result<DriverActionResult, String>;
    async fn press_key(&self, key: AndroidKey) -> Result<DriverActionResult, String>;
    async fn launch_app(
        &self,
        package: &str,
        activity: Option<&str>,
    ) -> Result<DriverActionResult, String>;
    async fn read_logcat(&self, filter: LogcatFilter) -> Result<LogcatResult, String>;
}
