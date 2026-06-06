//! Transcript display mode — controls timeline density.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TranscriptMode {
    /// Minimal journal: user turns, final summaries, errors, approvals.
    Summary,
    /// Default work journal: compact tool rows with expandable thinking summaries.
    #[default]
    Normal,
    /// Debug-friendly: raw tool I/O and full reasoning steps.
    Verbose,
}

impl TranscriptMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Normal => "Normal",
            Self::Verbose => "Verbose",
        }
    }

    pub fn shows_reasoning_rows(self) -> bool {
        matches!(self, Self::Normal | Self::Verbose)
    }

    pub fn shows_tool_output_rows(self) -> bool {
        matches!(self, Self::Verbose)
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Summary => Self::Normal,
            Self::Normal => Self::Verbose,
            Self::Verbose => Self::Summary,
        }
    }
}
