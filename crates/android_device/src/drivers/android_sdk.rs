use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndroidSdkPaths {
    pub sdk_root: Option<PathBuf>,
    pub adb: PathBuf,
    pub emulator: PathBuf,
    pub android_cli: PathBuf,
}

pub fn discover_android_sdk_paths() -> AndroidSdkPaths {
    let mut roots = Vec::new();
    if let Ok(root) = std::env::var("ANDROID_HOME") {
        roots.push(PathBuf::from(root));
    }
    if let Ok(root) = std::env::var("ANDROID_SDK_ROOT") {
        roots.push(PathBuf::from(root));
    }
    if let Some(home) = home_dir() {
        roots.push(home.join("Library").join("Android").join("sdk"));
        roots.push(home.join("Android").join("Sdk"));
    }

    for root in roots {
        let adb = root.join("platform-tools").join(exe("adb"));
        let emulator = root.join("emulator").join(exe("emulator"));
        let android_cli = find_android_cli(&root).unwrap_or_else(|| PathBuf::from(exe("android")));
        if adb.exists() && emulator.exists() {
            return AndroidSdkPaths {
                sdk_root: Some(root),
                adb,
                emulator,
                android_cli,
            };
        }
    }

    AndroidSdkPaths {
        sdk_root: None,
        adb: PathBuf::from(exe("adb")),
        emulator: PathBuf::from(exe("emulator")),
        android_cli: PathBuf::from(exe("android")),
    }
}

fn find_android_cli(root: &std::path::Path) -> Option<PathBuf> {
    let candidates = [
        root.join("cmdline-tools")
            .join("latest")
            .join("bin")
            .join(exe("android")),
        root.join("cmdline-tools").join("bin").join(exe("android")),
        root.join("tools").join("bin").join(exe("android")),
    ];
    candidates.into_iter().find(|path| path.exists())
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_uses_command_names() {
        let paths = AndroidSdkPaths {
            sdk_root: None,
            adb: PathBuf::from(exe("adb")),
            emulator: PathBuf::from(exe("emulator")),
            android_cli: PathBuf::from(exe("android")),
        };
        assert!(paths.adb.to_string_lossy().contains("adb"));
        assert!(paths.emulator.to_string_lossy().contains("emulator"));
    }
}
