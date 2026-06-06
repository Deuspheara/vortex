use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::fs::{InMemoryFs, VirtualDirEntry, VirtualFs, VirtualMetadata, WorkspaceFs};
use crate::{ShellError, ShellResult, VirtualPath};

#[derive(Debug)]
pub struct OverlayFs {
    workspace: WorkspaceFs,
    memory: Arc<InMemoryFs>,
    tombstones: RwLock<BTreeMap<VirtualPath, bool>>,
}

impl OverlayFs {
    pub fn new(project_root: impl Into<PathBuf>, max_file_read_bytes: usize) -> Self {
        let memory = Arc::new(InMemoryFs::new());
        memory.create_dir_all(&VirtualPath::from("/tmp")).ok();
        memory.create_dir_all(&VirtualPath::from("/home")).ok();
        memory.create_dir_all(&VirtualPath::from("/workspace")).ok();
        Self {
            workspace: WorkspaceFs::new(project_root, true, max_file_read_bytes),
            memory,
            tombstones: RwLock::new(BTreeMap::new()),
        }
    }

    fn is_memory_mount(path: &VirtualPath) -> bool {
        path.as_str() == "/tmp"
            || path.as_str().starts_with("/tmp/")
            || path.as_str() == "/home"
            || path.as_str().starts_with("/home/")
    }

    fn is_workspace(path: &VirtualPath) -> bool {
        path.as_str() == "/workspace" || path.as_str().starts_with("/workspace/")
    }

    fn ensure_supported(path: &VirtualPath) -> ShellResult<()> {
        if Self::is_memory_mount(path) || Self::is_workspace(path) {
            return Ok(());
        }
        Err(ShellError::AccessDenied(format!(
            "{path}: unsupported virtual path"
        )))
    }

    fn tombstoned(&self, path: &VirtualPath) -> bool {
        self.tombstones.read().unwrap().contains_key(path)
    }
}

impl VirtualFs for OverlayFs {
    fn read_file(&self, path: &VirtualPath) -> ShellResult<Vec<u8>> {
        Self::ensure_supported(path)?;
        if self.tombstoned(path) {
            return Err(ShellError::NotFound(format!("{path}: no such file")));
        }
        if self.memory.exists(path) {
            return self.memory.read_file(path);
        }
        if Self::is_workspace(path) {
            return self.workspace.read_file(path);
        }
        self.memory.read_file(path)
    }

    fn write_file(&self, path: &VirtualPath, data: &[u8]) -> ShellResult<()> {
        Self::ensure_supported(path)?;
        self.tombstones.write().unwrap().remove(path);
        self.memory.write_file(path, data)
    }

    fn metadata(&self, path: &VirtualPath) -> ShellResult<VirtualMetadata> {
        Self::ensure_supported(path)?;
        if self.tombstoned(path) {
            return Err(ShellError::NotFound(format!("{path}: no such file")));
        }
        if self.memory.exists(path) {
            return self.memory.metadata(path);
        }
        if Self::is_workspace(path) {
            return self.workspace.metadata(path);
        }
        self.memory.metadata(path)
    }

    fn list_dir(&self, path: &VirtualPath) -> ShellResult<Vec<VirtualDirEntry>> {
        Self::ensure_supported(path)?;
        let mut by_name = BTreeMap::<String, VirtualDirEntry>::new();
        if Self::is_workspace(path) {
            for entry in self.workspace.list_dir(path).unwrap_or_default() {
                if !self.tombstoned(&entry.path) {
                    by_name.insert(entry.name.clone(), entry);
                }
            }
        }
        for entry in self.memory.list_dir(path).unwrap_or_default() {
            if !self.tombstoned(&entry.path) {
                by_name.insert(entry.name.clone(), entry);
            }
        }
        Ok(by_name.into_values().collect())
    }

    fn create_dir_all(&self, path: &VirtualPath) -> ShellResult<()> {
        Self::ensure_supported(path)?;
        self.tombstones.write().unwrap().remove(path);
        self.memory.create_dir_all(path)
    }

    fn remove_file(&self, path: &VirtualPath) -> ShellResult<()> {
        Self::ensure_supported(path)?;
        if path.as_str() == "/workspace" || path.as_str() == "/tmp" || path.as_str() == "/home" {
            return Err(ShellError::AccessDenied(format!(
                "{path}: refusing to remove mount root"
            )));
        }
        if self.memory.exists(path) {
            let _ = self.memory.remove_file(path);
        }
        self.tombstones.write().unwrap().insert(path.clone(), true);
        Ok(())
    }

    fn remove_dir_all(&self, path: &VirtualPath) -> ShellResult<()> {
        Self::ensure_supported(path)?;
        if path.as_str() == "/workspace" || path.as_str() == "/tmp" || path.as_str() == "/home" {
            return Err(ShellError::AccessDenied(format!(
                "{path}: refusing to remove mount root"
            )));
        }
        let _ = self.memory.remove_dir_all(path);
        self.tombstones.write().unwrap().insert(path.clone(), true);
        Ok(())
    }

    fn rename(&self, from: &VirtualPath, to: &VirtualPath) -> ShellResult<()> {
        let data = self.read_file(from)?;
        self.write_file(to, &data)?;
        self.remove_file(from)?;
        Ok(())
    }
}
