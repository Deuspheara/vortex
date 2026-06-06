use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::sleep;

use crate::drivers::discover_android_sdk_paths;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidDeviceListing {
    pub serial: String,
    pub state: String,
    pub is_emulator: bool,
}

#[derive(Clone, Debug)]
pub struct EmulatorManager {
    adb: PathBuf,
    emulator: PathBuf,
}

impl Default for EmulatorManager {
    fn default() -> Self {
        let sdk = discover_android_sdk_paths();
        Self {
            adb: sdk.adb,
            emulator: sdk.emulator,
        }
    }
}

impl EmulatorManager {
    pub fn new(adb: impl Into<PathBuf>, emulator: impl Into<PathBuf>) -> Self {
        Self {
            adb: adb.into(),
            emulator: emulator.into(),
        }
    }

    pub async fn list_devices(&self) -> Result<Vec<AndroidDeviceListing>, String> {
        let output = Command::new(&self.adb)
            .arg("devices")
            .output()
            .await
            .map_err(|e| format!("Could not run adb at {}: {e}", self.adb.display()))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(parse_adb_devices(&String::from_utf8_lossy(&output.stdout)))
    }

    pub async fn list_avds(&self) -> Result<Vec<String>, String> {
        let output = Command::new(&self.emulator)
            .arg("-list-avds")
            .output()
            .await
            .map_err(|e| {
                format!(
                    "Could not run emulator at {}: {e}. Install Android Studio or Android SDK; Vortex looks in ANDROID_HOME, ANDROID_SDK_ROOT, and ~/Library/Android/sdk.",
                    self.emulator.display()
                )
            })?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }

    pub async fn ensure_emulator_ready(
        &self,
        preferred_avd: Option<&str>,
    ) -> Result<AndroidDeviceListing, String> {
        if let Some(device) = self.preferred_running_device().await? {
            return Ok(device);
        }

        let avd = match preferred_avd {
            Some(avd) if !avd.trim().is_empty() => avd.trim().to_string(),
            _ => self
                .list_avds()
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "No running Android device found, and no AVDs are available. Create an AVD once in Android Studio or Android CLI, then Vortex can start it automatically.".to_string()
                })?,
        };
        self.start_avd(&avd).await?;
        self.wait_for_boot(Duration::from_secs(120)).await
    }

    async fn preferred_running_device(&self) -> Result<Option<AndroidDeviceListing>, String> {
        let devices = self.list_devices().await?;
        Ok(devices
            .iter()
            .find(|device| device.is_emulator && device.state == "device")
            .cloned()
            .or_else(|| devices.into_iter().find(|device| device.state == "device")))
    }

    async fn start_avd(&self, avd: &str) -> Result<(), String> {
        Command::new(&self.emulator)
            .arg(format!("@{avd}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn wait_for_boot(&self, timeout: Duration) -> Result<AndroidDeviceListing, String> {
        let started = Instant::now();
        loop {
            if started.elapsed() > timeout {
                return Err("Timed out waiting for Android emulator to boot".into());
            }
            if let Some(device) = self.preferred_running_device().await? {
                if self.boot_completed(&device.serial).await.unwrap_or(false) {
                    return Ok(device);
                }
            }
            sleep(Duration::from_millis(1_000)).await;
        }
    }

    async fn boot_completed(&self, serial: &str) -> Result<bool, String> {
        let output = Command::new(&self.adb)
            .arg("-s")
            .arg(serial)
            .args(["shell", "getprop", "sys.boot_completed"])
            .output()
            .await
            .map_err(|e| e.to_string())?;
        Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "1")
    }
}

pub fn parse_adb_devices(output: &str) -> Vec<AndroidDeviceListing> {
    output
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?.to_string();
            let state = parts.next()?.to_string();
            Some(AndroidDeviceListing {
                is_emulator: serial.starts_with("emulator-"),
                serial,
                state,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adb_devices() {
        let devices =
            parse_adb_devices("List of devices attached\nemulator-5554\tdevice\nABC123\toffline\n");
        assert_eq!(devices.len(), 2);
        assert!(devices[0].is_emulator);
        assert_eq!(devices[1].state, "offline");
    }
}
