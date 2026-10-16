//! Transcript display mode — controls timeline density.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TranscriptMode {
    /// Calm default: goal, plan, current action, important receipts, blockers, final answer.
    #[default]
    Focus,
    /// Full semantic activity log without raw command output.
    Activity,
    /// Debug-friendly: raw tool I/O and full reasoning steps.
    Debug,
}

impl TranscriptMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Focus => "Focus",
            Self::Activity => "Activity",
            Self::Debug => "Debug",
        }
    }

    pub fn shows_reasoning_rows(self) -> bool {
        matches!(self, Self::Activity | Self::Debug)
    }

    pub fn shows_tool_detail_rows(self) -> bool {
        matches!(self, Self::Activity | Self::Debug)
    }

    pub fn shows_tool_output_rows(self) -> bool {
        matches!(self, Self::Debug)
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Focus => Self::Activity,
            Self::Activity => Self::Debug,
            Self::Debug => Self::Focus,
        }
    }
}
