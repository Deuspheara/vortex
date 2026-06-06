//! Ledger of agent-launched shell commands (compact summaries, not full stdout).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use agent_protocol::ContextAttachment;
use gpui::{Image, ImageFormat};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingImageSource {
    File(PathBuf),
    Clipboard(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingImageAttachment {
    pub id: String,
    pub source: PendingImageSource,
    pub mime_type: String,
    pub display_name: String,
    pub size_bytes: u64,
}

impl PendingImageAttachment {
    pub fn to_context_attachment(&self) -> ContextAttachment {
        match &self.source {
            PendingImageSource::File(path) => {
                ContextAttachment::image_file(path.clone(), self.mime_type.clone(), self.size_bytes)
            }
            PendingImageSource::Clipboard(bytes) => ContextAttachment::image_bytes(
                bytes.clone(),
                self.mime_type.clone(),
                self.display_name.clone(),
            ),
        }
    }

    pub fn preview_image(&self) -> Option<Arc<Image>> {
        match &self.source {
            PendingImageSource::File(_) => None,
            PendingImageSource::Clipboard(bytes) => Some(Arc::new(Image::from_bytes(
                image_format_for_mime_type(&self.mime_type)?,
                bytes.clone(),
            ))),
        }
    }
}

pub fn image_format_for_mime_type(mime_type: &str) -> Option<ImageFormat> {
    ImageFormat::from_mime_type(mime_type)
}

/// Record of a single command execution for thread/inspector summaries.
#[derive(Clone, Debug)]
pub struct CommandRun {
    pub id: String,
    pub session_id: Option<String>,
    pub command: String,
    pub cwd: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub output_excerpt: Option<String>,
}

impl CommandRun {
    pub fn summary_line(&self) -> String {
        let exit = self
            .exit_code
            .map(|c| format!("exit {c}"))
            .unwrap_or_else(|| "running".to_string());
        let duration = match (self.ended_at_ms, self.started_at_ms) {
            (Some(end), start) if end >= start => {
                format!("{:.1}s", (end - start) as f64 / 1000.0)
            }
            _ => String::new(),
        };
        if duration.is_empty() {
            format!("Ran `{}` · {exit}", self.command)
        } else {
            format!("Ran `{}` · {exit} · {duration}", self.command)
        }
    }
}

/// In-memory command run ledger keyed by tool/thread item id.
#[derive(Clone, Debug, Default)]
pub struct CommandRunLedger {
    runs: HashMap<String, CommandRun>,
}

impl CommandRunLedger {
    pub fn insert(&mut self, run: CommandRun) {
        self.runs.insert(run.id.clone(), run);
    }

    pub fn get(&self, id: &str) -> Option<&CommandRun> {
        self.runs.get(id)
    }

    pub fn finish(
        &mut self,
        id: &str,
        exit_code: Option<i32>,
        excerpt: Option<String>,
        ended_at_ms: i64,
    ) {
        if let Some(run) = self.runs.get_mut(id) {
            run.exit_code = exit_code;
            run.output_excerpt = excerpt;
            run.ended_at_ms = Some(ended_at_ms);
        }
    }
}

/// Cap stdout stored in artifacts / excerpts (~2 KB).
pub fn excerpt_output(output: &str) -> String {
    const MAX: usize = 2048;
    if output.len() <= MAX {
        output.to_string()
    } else {
        format!("{}…", &output[..MAX])
    }
}
