use std::path::PathBuf;

use tokio::process::Command;

use crate::drivers::discover_android_sdk_paths;

#[derive(Clone, Debug)]
pub struct AndroidCliDriver {
    android: PathBuf,
    cwd: PathBuf,
}

impl AndroidCliDriver {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        let sdk = discover_android_sdk_paths();
        Self {
            android: sdk.android_cli,
            cwd: cwd.into(),
        }
    }

    pub fn with_android_path(mut self, android: impl Into<PathBuf>) -> Self {
        self.android = android.into();
        self
    }

    pub async fn run(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new(&self.android)
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .await
            .map_err(|e| {
                format!(
                    "Could not run Android CLI at {}: {e}",
                    self.android.display()
                )
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            Ok(stdout.trim().to_string())
        } else {
            Err(if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            })
        }
    }
}
