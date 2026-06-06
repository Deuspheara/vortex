use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;

use crate::fs::{VirtualDirEntry, VirtualFs, VirtualMetadata};
use crate::{ShellError, ShellResult, VirtualPath};

#[derive(Debug, Default)]
pub struct InMemoryFs {
    files: RwLock<BTreeMap<VirtualPath, Vec<u8>>>,
    dirs: RwLock<BTreeSet<VirtualPath>>,
}

impl InMemoryFs {
    pub fn new() -> Self {
        let fs = Self::default();
        fs.dirs.write().unwrap().insert(VirtualPath::root());
        fs
    }

    fn ensure_parent_dirs(&self, path: &VirtualPath) {
        let mut dirs = self.dirs.write().unwrap();
        let mut current = path.parent();
        while let Some(dir) = current {
            dirs.insert(dir.clone());
            current = dir.parent();
        }
        dirs.insert(VirtualPath::root());
    }

    pub fn exists(&self, path: &VirtualPath) -> bool {
        self.files.read().unwrap().contains_key(path) || self.dirs.read().unwrap().contains(path)
    }
}

impl VirtualFs for InMemoryFs {
    fn read_file(&self, path: &VirtualPath) -> ShellResult<Vec<u8>> {
        if self.dirs.read().unwrap().contains(path) {
            return Err(ShellError::IsDirectory(format!("{path}: is a directory")));
        }
        self.files
            .read()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| ShellError::NotFound(format!("{path}: no such file")))
    }

    fn write_file(&self, path: &VirtualPath, data: &[u8]) -> ShellResult<()> {
        self.ensure_parent_dirs(path);
        self.files
            .write()
            .unwrap()
            .insert(path.clone(), data.to_vec());
        Ok(())
    }

    fn metadata(&self, path: &VirtualPath) -> ShellResult<VirtualMetadata> {
        if let Some(data) = self.files.read().unwrap().get(path) {
            return Ok(VirtualMetadata {
                is_dir: false,
                is_file: true,
                len: data.len() as u64,
            });
        }
        if self.dirs.read().unwrap().contains(path) {
            return Ok(VirtualMetadata {
                is_dir: true,
                is_file: false,
                len: 0,
            });
        }
        Err(ShellError::NotFound(format!(
            "{path}: no such file or directory"
        )))
    }

    fn list_dir(&self, path: &VirtualPath) -> ShellResult<Vec<VirtualDirEntry>> {
        if !self.dirs.read().unwrap().contains(path) {
            return Err(ShellError::NotDirectory(format!("{path}: not a directory")));
        }
        let mut names = BTreeMap::<String, VirtualDirEntry>::new();
        for dir in self.dirs.read().unwrap().iter() {
            if let Some(parent) = dir.parent() {
                if &parent == path && dir != path {
                    names.insert(
                        dir.name().into(),
                        VirtualDirEntry {
                            path: dir.clone(),
                            name: dir.name().into(),
                            metadata: VirtualMetadata {
                                is_dir: true,
                                is_file: false,
                                len: 0,
                            },
                        },
                    );
                }
            }
        }
        for (file, data) in self.files.read().unwrap().iter() {
            if let Some(parent) = file.parent() {
                if &parent == path {
                    names.insert(
                        file.name().into(),
                        VirtualDirEntry {
                            path: file.clone(),
                            name: file.name().into(),
                            metadata: VirtualMetadata {
                                is_dir: false,
                                is_file: true,
                                len: data.len() as u64,
                            },
                        },
                    );
                }
            }
        }
        Ok(names.into_values().collect())
    }

    fn create_dir_all(&self, path: &VirtualPath) -> ShellResult<()> {
        self.ensure_parent_dirs(path);
        self.dirs.write().unwrap().insert(path.clone());
        Ok(())
    }

    fn remove_file(&self, path: &VirtualPath) -> ShellResult<()> {
        self.files
            .write()
            .unwrap()
            .remove(path)
            .map(|_| ())
            .ok_or_else(|| ShellError::NotFound(format!("{path}: no such file")))
    }

    fn remove_dir_all(&self, path: &VirtualPath) -> ShellResult<()> {
        if path.is_root() {
            return Err(ShellError::AccessDenied("refusing to remove /".into()));
        }
        self.files
            .write()
            .unwrap()
            .retain(|p, _| !p.as_str().starts_with(&format!("{}/", path.as_str())));
        self.dirs
            .write()
            .unwrap()
            .retain(|p| p != path && !p.as_str().starts_with(&format!("{}/", path.as_str())));
        Ok(())
    }

    fn rename(&self, from: &VirtualPath, to: &VirtualPath) -> ShellResult<()> {
        if let Some(data) = self.files.write().unwrap().remove(from) {
            self.write_file(to, &data)?;
            return Ok(());
        }
        Err(ShellError::NotFound(format!("{from}: no such file")))
    }
}
