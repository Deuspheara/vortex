use agent_protocol::IconToken;
use gpui::Hsla;
use gpui_component::IconName;

use crate::features::shell::state::{SessionStep, StepKind, StepStatus};
use crate::shared::state::ToolCatalog;
use crate::tokens::{Tokens, icons};

#[allow(dead_code)]
pub fn step_icon(catalog: &ToolCatalog, step: &SessionStep) -> (IconName, Hsla) {
    match step.status {
        StepStatus::Running => (icons::LOADER, Tokens::accent()),
        StepStatus::Failed => (icons::X_MARK, Tokens::danger()),
        StepStatus::Done => match &step.kind {
            StepKind::Thought => (icons::BOT, Tokens::text_secondary()),
            StepKind::Tool(name) => (icon_token_to_gpui(catalog.icon(name)), Tokens::success()),
            StepKind::Diff => (icons::GIT_COMPARE, Tokens::success()),
        },
    }
}

#[allow(dead_code)]
pub fn icon_token_to_gpui(token: IconToken) -> IconName {
    match token {
        IconToken::File => icons::FILE_TEXT,
        IconToken::Folder => icons::FOLDER,
        IconToken::Search => icons::SEARCH,
        IconToken::Terminal => icons::TERMINAL,
        IconToken::Pencil => icons::PENCIL,
        IconToken::GitCompare => icons::GIT_COMPARE,
        IconToken::Bot => icons::BOT,
        IconToken::Checklist => icons::CHECKLIST,
        IconToken::Globe => icons::GLOBE,
        IconToken::Question => icons::QUESTION,
    }
}
