pub mod commands;
pub mod error;
pub mod event;
pub mod fs;
pub mod limits;
pub mod parser;
pub mod path;
pub mod policy;
pub mod shell;

pub use commands::{BuiltinCommand, CommandContext, CommandOutput, CommandRegistry};
pub use error::{ShellError, ShellResult};
pub use fs::{OverlayFs, VirtualDirEntry, VirtualFs, VirtualMetadata, WorkspaceFs};
pub use limits::ExecutionLimits;
pub use parser::{ParsedCommand, parse_script};
pub use path::VirtualPath;
pub use policy::ShellPolicy;
pub use shell::{ExecRequest, ExecResult, Shell};
