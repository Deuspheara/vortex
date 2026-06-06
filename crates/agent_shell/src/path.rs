use std::fmt;

use crate::{ShellError, ShellResult};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualPath(String);

impl VirtualPath {
    pub fn root() -> Self {
        Self("/".into())
    }

    pub fn workspace() -> Self {
        Self("/workspace".into())
    }

    pub fn normalize(cwd: &VirtualPath, input: &str) -> ShellResult<Self> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(ShellError::InvalidInput("empty path".into()));
        }
        let mut parts = Vec::<&str>::new();
        if !raw.starts_with('/') {
            parts.extend(cwd.0.split('/').filter(|p| !p.is_empty()));
        }
        for part in raw.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                p => parts.push(p),
            }
        }
        let path = if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        };
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0 == "/"
    }

    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }
        let trimmed = self.0.trim_end_matches('/');
        let ix = trimmed.rfind('/')?;
        if ix == 0 {
            Some(Self::root())
        } else {
            Some(Self(trimmed[..ix].to_string()))
        }
    }

    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or("/")
    }

    pub fn join(&self, child: &str) -> ShellResult<Self> {
        Self::normalize(self, child)
    }
}

impl fmt::Display for VirtualPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for VirtualPath {
    fn from(value: &str) -> Self {
        Self::normalize(&Self::root(), value).unwrap_or_else(|_| Self::root())
    }
}
