use std::fmt;

pub type ShellResult<T> = Result<T, ShellError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellError {
    Parse(String),
    AccessDenied(String),
    NotFound(String),
    NotDirectory(String),
    IsDirectory(String),
    InvalidInput(String),
    Unsupported(String),
    LimitExceeded(String),
    Io(String),
}

impl ShellError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Parse(_) | Self::Unsupported(_) => 2,
            Self::AccessDenied(_) => 1,
            Self::NotFound(_) => 1,
            Self::NotDirectory(_) => 1,
            Self::IsDirectory(_) => 1,
            Self::InvalidInput(_) => 1,
            Self::LimitExceeded(_) => 1,
            Self::Io(_) => 1,
        }
    }
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg)
            | Self::AccessDenied(msg)
            | Self::NotFound(msg)
            | Self::NotDirectory(msg)
            | Self::IsDirectory(msg)
            | Self::InvalidInput(msg)
            | Self::Unsupported(msg)
            | Self::LimitExceeded(msg)
            | Self::Io(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for ShellError {}

impl From<std::io::Error> for ShellError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}
