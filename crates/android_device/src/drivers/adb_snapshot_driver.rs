use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use agent_protocol::{AndroidDeviceRef, AndroidObservation, AndroidPointPx, AndroidSizePx};
use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::drivers::{
    AndroidDeviceDriver, AndroidKey, DriverActionResult, EmulatorManager, LogcatFilter,
    LogcatResult, discover_android_sdk_paths,
};
use crate::observation::parse_ui_tree;

#[derive(Clone, Debug)]
pub struct AdbSnapshotDriver {
    adb: PathBuf,
    emulator: PathBuf,
    serial: Option<String>,
    avd: Option<String>,
    artifact_dir: PathBuf,
    cached_device: Arc<Mutex<Option<AndroidDeviceRef>>>,
    cached_screen_size: Arc<Mutex<Option<AndroidSizePx>>>,
}

impl AdbSnapshotDriver {
    pub fn new(project_root: impl AsRef<Path>, serial: Option<String>) -> Self {
        let sdk = discover_android_sdk_paths();
        Self {
            adb: sdk.adb,
            emulator: sdk.emulator,
            serial,
            avd: None,
            artifact_dir: project_root
                .as_ref()
                .join(".android-agent")
                .join("observations"),
            cached_device: Arc::new(Mutex::new(None)),
            cached_screen_size: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_adb_path(mut self, adb: impl Into<PathBuf>) -> Self {
        self.adb = adb.into();
        self
    }

    pub fn with_emulator_path(mut self, emulator: impl Into<PathBuf>) -> Self {
        self.emulator = emulator.into();
        self
    }

    pub fn with_avd(mut self, avd: Option<String>) -> Self {
        self.avd = avd;
        self
    }

    pub async fn ensure_emulator_ready(&self) -> Result<agent_protocol::AndroidDeviceRef, String> {
        if let Some(device) = self.cached_device.lock().await.clone() {
            return Ok(device);
        }

        let device = if let Some(serial) = &self.serial {
            AndroidDeviceRef {
                serial: serial.clone(),
                name: None,
                is_emulator: serial.starts_with("emulator-"),
            }
        } else {
            let listing = EmulatorManager::new(&self.adb, &self.emulator)
                .ensure_emulator_ready(self.avd.as_deref())
                .await?;
            AndroidDeviceRef {
                serial: listing.serial,
                name: self.avd.clone(),
                is_emulator: listing.is_emulator,
            }
        };
        *self.cached_device.lock().await = Some(device.clone());
        Ok(device)
    }

    async fn adb_output(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        let device = self.ensure_emulator_ready().await?;
        let output = self.adb_output_for_serial(&device.serial, args).await?;
        if output.status.success() {
            return Ok(output.stdout);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if should_refresh_device(&stderr) {
            *self.cached_device.lock().await = None;
            let device = self.ensure_emulator_ready().await?;
            let retry = self.adb_output_for_serial(&device.serial, args).await?;
            if retry.status.success() {
                return Ok(retry.stdout);
            }
            let retry_stderr = String::from_utf8_lossy(&retry.stderr);
            return Err(retry_stderr.trim().to_string());
        }
        Err(stderr)
    }

    async fn adb_output_for_serial(
        &self,
        serial: &str,
        args: &[&str],
    ) -> Result<std::process::Output, String> {
        let mut command = Command::new(&self.adb);
        command.arg("-s").arg(serial);
        command.args(args);
        command.output().await.map_err(|e| e.to_string())
    }

    async fn adb_shell(&self, args: &[&str]) -> Result<String, String> {
        let mut full = vec!["shell"];
        full.extend(args);
        let output = self.adb_output(&full).await?;
        Ok(String::from_utf8_lossy(&output).into_owned())
    }

    async fn current_package_activity(&self) -> (Option<String>, Option<String>) {
        let Ok(output) = self.adb_shell(&["dumpsys", "window", "windows"]).await else {
            return (None, None);
        };
        for line in output.lines() {
            if let Some(rest) = line
                .split("mCurrentFocus=")
                .nth(1)
                .or_else(|| line.split("mFocusedApp=").nth(1))
            {
                if let Some(component) = rest.split_whitespace().find(|part| part.contains('/')) {
                    let component = component.trim_matches(|c| c == '}' || c == '{');
                    let mut parts = component.split('/');
                    let package = parts
                        .next()
                        .filter(|s| !s.is_empty())
                        .map(ToOwned::to_owned);
                    let activity = parts
                        .next()
                        .filter(|s| !s.is_empty())
                        .map(ToOwned::to_owned);
                    if package.is_some() || activity.is_some() {
                        return (package, activity);
                    }
                }
            }
        }
        (None, None)
    }

    async fn screen_size(&self) -> AndroidSizePx {
        if let Some(size) = *self.cached_screen_size.lock().await {
            return size;
        }
        let Ok(output) = self.adb_shell(&["wm", "size"]).await else {
            return AndroidSizePx {
                width: 1080.0,
                height: 2400.0,
            };
        };
        for token in output.split_whitespace() {
            if let Some((w, h)) = token.split_once('x') {
                if let (Ok(width), Ok(height)) = (w.parse::<f32>(), h.parse::<f32>()) {
                    let size = AndroidSizePx { width, height };
                    *self.cached_screen_size.lock().await = Some(size);
                    return size;
                }
            }
        }
        let size = AndroidSizePx {
            width: 1080.0,
            height: 2400.0,
        };
        *self.cached_screen_size.lock().await = Some(size);
        size
    }
}

#[async_trait]
impl AndroidDeviceDriver for AdbSnapshotDriver {
    async fn observe(&self) -> Result<AndroidObservation, String> {
        let device = self.ensure_emulator_ready().await?;
        tokio::fs::create_dir_all(&self.artifact_dir)
            .await
            .map_err(|e| e.to_string())?;
        let observation_id = format!("obs_{}", chrono_like_timestamp_ms());
        let screenshot_path = self.artifact_dir.join(format!("{observation_id}.png"));
        let tree_path = self.artifact_dir.join(format!("{observation_id}.xml"));

        let screenshot = self.adb_output(&["exec-out", "screencap", "-p"]).await?;
        tokio::fs::write(&screenshot_path, screenshot)
            .await
            .map_err(|e| e.to_string())?;

        let _ = self
            .adb_shell(&["uiautomator", "dump", "/sdcard/window.xml"])
            .await;
        let xml = self
            .adb_output(&["exec-out", "cat", "/sdcard/window.xml"])
            .await
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())?;
        tokio::fs::write(&tree_path, &xml)
            .await
            .map_err(|e| e.to_string())?;

        let visible_targets = parse_ui_tree(&xml)?;
        let (package, activity) = self.current_package_activity().await;
        let screen = self.screen_size().await;
        Ok(AndroidObservation {
            observation_id,
            device: Some(AndroidDeviceRef {
                serial: device.serial,
                name: device.name,
                is_emulator: device.is_emulator,
            }),
            package,
            activity,
            screen,
            visible_targets,
            screenshot_ref: Some(format!("artifact://android/{}", screenshot_path.display())),
            ui_tree_ref: Some(format!("artifact://android/{}", tree_path.display())),
            timestamp_ms: chrono_like_timestamp_ms(),
        })
    }

    async fn tap(&self, point: AndroidPointPx) -> Result<DriverActionResult, String> {
        let started = Instant::now();
        self.adb_shell(&[
            "input",
            "tap",
            &format!("{:.0}", point.x),
            &format!("{:.0}", point.y),
        ])
        .await?;
        Ok(action_result(true, "Tapped point", started))
    }

    async fn type_text(&self, text: &str, sensitive: bool) -> Result<DriverActionResult, String> {
        let started = Instant::now();
        self.adb_shell(&["input", "text", &adb_input_text(text)])
            .await?;
        let summary = if sensitive {
            "Typed sensitive text".to_string()
        } else {
            format!("Typed {} chars", text.chars().count())
        };
        Ok(action_result(true, summary, started))
    }

    async fn swipe(
        &self,
        from: AndroidPointPx,
        to: AndroidPointPx,
        duration_ms: u64,
    ) -> Result<DriverActionResult, String> {
        let started = Instant::now();
        self.adb_shell(&[
            "input",
            "swipe",
            &format!("{:.0}", from.x),
            &format!("{:.0}", from.y),
            &format!("{:.0}", to.x),
            &format!("{:.0}", to.y),
            &duration_ms.to_string(),
        ])
        .await?;
        Ok(action_result(true, "Swiped", started))
    }

    async fn press_key(&self, key: AndroidKey) -> Result<DriverActionResult, String> {
        let started = Instant::now();
        self.adb_shell(&["input", "keyevent", key.adb_keyevent()])
            .await?;
        Ok(action_result(true, format!("Pressed {:?}", key), started))
    }

    async fn launch_app(
        &self,
        package: &str,
        activity: Option<&str>,
    ) -> Result<DriverActionResult, String> {
        let started = Instant::now();
        if let Some(activity) = activity {
            self.adb_shell(&["am", "start", "-n", &format!("{package}/{activity}")])
                .await?;
        } else {
            self.adb_shell(&[
                "monkey",
                "-p",
                package,
                "-c",
                "android.intent.category.LAUNCHER",
                "1",
            ])
            .await?;
        }
        Ok(action_result(true, format!("Launched {package}"), started))
    }

    async fn read_logcat(&self, filter: LogcatFilter) -> Result<LogcatResult, String> {
        let max_lines = filter.max_lines.clamp(1, 500);
        let output = self
            .adb_output(&["logcat", "-d", "-t", &max_lines.to_string()])
            .await?;
        let mut text = String::from_utf8_lossy(&output).into_owned();
        if let Some(package) = filter.package {
            text = text
                .lines()
                .filter(|line| line.contains(&package))
                .collect::<Vec<_>>()
                .join("\n");
        }
        let truncated = text.lines().count() >= max_lines;
        Ok(LogcatResult {
            output: text,
            truncated,
        })
    }
}

fn action_result(
    success: bool,
    summary: impl Into<String>,
    started: Instant,
) -> DriverActionResult {
    DriverActionResult {
        success,
        summary: summary.into(),
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

fn adb_input_text(text: &str) -> String {
    text.replace('%', "%25")
        .replace(' ', "%s")
        .replace('"', "\\\"")
        .replace('\'', "\\'")
}

fn chrono_like_timestamp_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn should_refresh_device(stderr: &str) -> bool {
    let stderr = stderr.to_ascii_lowercase();
    stderr.contains("device offline")
        || stderr.contains("device not found")
        || stderr.contains("more than one device")
}

#[cfg(test)]
mod tests {
    use super::{adb_input_text, should_refresh_device};

    #[test]
    fn encodes_adb_input_text_spaces() {
        assert_eq!(adb_input_text("hello world"), "hello%sworld");
    }

    #[test]
    fn refreshes_cached_device_on_common_adb_errors() {
        assert!(should_refresh_device("error: device offline"));
        assert!(should_refresh_device("error: device not found"));
        assert!(!should_refresh_device("Security exception"));
    }
}
