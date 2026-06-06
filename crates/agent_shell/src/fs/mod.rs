mod memory;
mod overlay;
mod workspace;

pub use memory::InMemoryFs;
pub use overlay::OverlayFs;
pub use workspace::WorkspaceFs;

use crate::{ShellResult, VirtualPath};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualMetadata {
    pub is_dir: bool,
    pub is_file: bool,
    pub len: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualDirEntry {
    pub path: VirtualPath,
    pub name: String,
    pub metadata: VirtualMetadata,
}

pub trait VirtualFs: Send + Sync {
    fn read_file(&self, path: &VirtualPath) -> ShellResult<Vec<u8>>;
    fn write_file(&self, path: &VirtualPath, data: &[u8]) -> ShellResult<()>;
    fn metadata(&self, path: &VirtualPath) -> ShellResult<VirtualMetadata>;
    fn list_dir(&self, path: &VirtualPath) -> ShellResult<Vec<VirtualDirEntry>>;
    fn create_dir_all(&self, path: &VirtualPath) -> ShellResult<()>;
    fn remove_file(&self, path: &VirtualPath) -> ShellResult<()>;
    fn remove_dir_all(&self, path: &VirtualPath) -> ShellResult<()>;
    fn rename(&self, from: &VirtualPath, to: &VirtualPath) -> ShellResult<()>;
}
