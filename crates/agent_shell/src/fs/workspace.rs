use std::fs;
use std::path::{Path, PathBuf};

use crate::fs::{VirtualDirEntry, VirtualFs, VirtualMetadata};
use crate::{ShellError, ShellResult, VirtualPath};

#[derive(Clone, Debug)]
pub struct WorkspaceFs {
    root: PathBuf,
    read_only: bool,
    max_file_read_bytes: usize,
}

impl WorkspaceFs {
    pub fn new(root: impl Into<PathBuf>, read_only: bool, max_file_read_bytes: usize) -> Self {
        Self {
            root: root.into(),
            read_only,
            max_file_read_bytes,
        }
    }

    fn root(&self) -> ShellResult<PathBuf> {
        self.root
            .canonicalize()
            .map_err(|e| ShellError::Io(format!("workspace root: {e}")))
    }

    fn host_path(&self, path: &VirtualPath, allow_missing: bool) -> ShellResult<PathBuf> {
        let root = self.root()?;
        let Some(rest) = path.as_str().strip_prefix("/workspace") else {
            return Err(ShellError::AccessDenied(format!(
                "{path}: unsupported virtual mount"
            )));
        };
        let rest = rest.trim_start_matches('/');
        let joined = root.join(rest);
        if joined.exists() {
            let canonical = joined.canonicalize()?;
            if !canonical.starts_with(&root) {
                return Err(ShellError::AccessDenied(format!(
                    "{path}: symlink escapes workspace"
                )));
            }
            return Ok(canonical);
        }
        if !allow_missing {
            return Err(ShellError::NotFound(format!(
                "{path}: no such file or directory"
            )));
        }
        let parent = joined
            .parent()
            .ok_or_else(|| ShellError::InvalidInput(format!("{path}: invalid path")))?;
        let parent = canonical_existing_ancestor(parent)?;
        if !parent.starts_with(&root) {
            return Err(ShellError::AccessDenied(format!(
                "{path}: escapes workspace"
            )));
        }
        Ok(joined)
    }
}

fn canonical_existing_ancestor(path: &Path) -> ShellResult<PathBuf> {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            return Ok(current.canonicalize()?);
        }
        current = current
            .parent()
            .ok_or_else(|| ShellError::AccessDenied("path has no existing ancestor".into()))?
            .to_path_buf();
    }
}

impl VirtualFs for WorkspaceFs {
    fn read_file(&self, path: &VirtualPath) -> ShellResult<Vec<u8>> {
        let host = self.host_path(path, false)?;
        let metadata = fs::metadata(&host)?;
        if metadata.is_dir() {
            return Err(ShellError::IsDirectory(format!("{path}: is a directory")));
        }
        if metadata.len() as usize > self.max_file_read_bytes {
            return Err(ShellError::LimitExceeded(format!(
                "{path}: file exceeds max read size"
            )));
        }
        Ok(fs::read(host)?)
    }

    fn write_file(&self, path: &VirtualPath, data: &[u8]) -> ShellResult<()> {
        if self.read_only {
            return Err(ShellError::AccessDenied("workspace is read-only".into()));
        }
        let host = self.host_path(path, true)?;
        if let Some(parent) = host.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(host, data)?;
        Ok(())
    }

    fn metadata(&self, path: &VirtualPath) -> ShellResult<VirtualMetadata> {
        let host = self.host_path(path, false)?;
        let metadata = fs::metadata(host)?;
        Ok(VirtualMetadata {
            is_dir: metadata.is_dir(),
            is_file: metadata.is_file(),
            len: metadata.len(),
        })
    }

    fn list_dir(&self, path: &VirtualPath) -> ShellResult<Vec<VirtualDirEntry>> {
        let host = self.host_path(path, false)?;
        if !fs::metadata(&host)?.is_dir() {
            return Err(ShellError::NotDirectory(format!("{path}: not a directory")));
        }
        let root = self.root()?;
        let mut entries = Vec::new();
        for item in fs::read_dir(host)? {
            let item = item?;
            let canonical = match item.path().canonicalize() {
                Ok(path) if path.starts_with(&root) => path,
                _ => continue,
            };
            let metadata = fs::metadata(&canonical)?;
            let rel = canonical.strip_prefix(&root).unwrap_or(&canonical);
            let virtual_path = if rel.as_os_str().is_empty() {
                VirtualPath::workspace()
            } else {
                VirtualPath::normalize(
                    &VirtualPath::workspace(),
                    &rel.to_string_lossy().replace('\\', "/"),
                )?
            };
            entries.push(VirtualDirEntry {
                name: item.file_name().to_string_lossy().into_owned(),
                path: virtual_path,
                metadata: VirtualMetadata {
                    is_dir: metadata.is_dir(),
                    is_file: metadata.is_file(),
                    len: metadata.len(),
                },
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn create_dir_all(&self, path: &VirtualPath) -> ShellResult<()> {
        if self.read_only {
            return Err(ShellError::AccessDenied("workspace is read-only".into()));
        }
        fs::create_dir_all(self.host_path(path, true)?)?;
        Ok(())
    }

    fn remove_file(&self, path: &VirtualPath) -> ShellResult<()> {
        if self.read_only {
            return Err(ShellError::AccessDenied("workspace is read-only".into()));
        }
        fs::remove_file(self.host_path(path, false)?)?;
        Ok(())
    }

    fn remove_dir_all(&self, path: &VirtualPath) -> ShellResult<()> {
        if self.read_only {
            return Err(ShellError::AccessDenied("workspace is read-only".into()));
        }
        fs::remove_dir_all(self.host_path(path, false)?)?;
        Ok(())
    }

    fn rename(&self, from: &VirtualPath, to: &VirtualPath) -> ShellResult<()> {
        if self.read_only {
            return Err(ShellError::AccessDenied("workspace is read-only".into()));
        }
        fs::rename(self.host_path(from, false)?, self.host_path(to, true)?)?;
        Ok(())
    }
}
