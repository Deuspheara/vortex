//! Centralised icon constants for the Vortex agentic UI.
//!
//! All icons map to gpui_component::IconName variants.
//! Many are defined for future use; dead-code warnings are expected.

#![allow(dead_code)]

use gpui_component::IconName;

// ── Navigation & chrome ────────────────────────────────────
pub const PLUS: IconName = IconName::Plus;
pub const SEARCH: IconName = IconName::Search;
pub const HISTORY: IconName = IconName::Calendar;
pub const CLOCK: IconName = IconName::Calendar;
pub const SETTINGS: IconName = IconName::Settings;
pub const PANEL_LEFT: IconName = IconName::PanelLeft;
pub const MORE_HORIZONTAL: IconName = IconName::Ellipsis;
pub const EXTERNAL_LINK: IconName = IconName::ExternalLink;
pub const DELETE: IconName = IconName::Delete;
pub const APP_WINDOW: IconName = IconName::WindowMaximize;
pub const OPEN_IDE: IconName = IconName::SquareTerminal;
pub const PANEL_LEFT_CLOSE: IconName = IconName::PanelLeftClose;
pub const PANEL_RIGHT: IconName = IconName::PanelRight;
pub const PANEL_RIGHT_CLOSE: IconName = IconName::PanelRightClose;
pub const PANEL_BOTTOM: IconName = IconName::PanelBottom;
pub const PANEL_BOTTOM_OPEN: IconName = IconName::PanelBottomOpen;

// ── Projects & files ───────────────────────────────────────
pub const FOLDER: IconName = IconName::Folder;
pub const FOLDER_GIT: IconName = IconName::Folder;
pub const MESSAGE_SQUARE: IconName = IconName::Inbox;
pub const FILE_TEXT: IconName = IconName::File;
pub const FILE_CODE: IconName = IconName::File;

// ── Agents & models ────────────────────────────────────────
pub const BOT: IconName = IconName::Bot;
pub const CPU: IconName = IconName::Bot;
pub const CLOUD: IconName = IconName::Globe;
pub const LAPTOP: IconName = IconName::LayoutDashboard;

// ── Composer ───────────────────────────────────────────────
pub const ARROW_UP: IconName = IconName::ArrowUp;
pub const MIC: IconName = IconName::Bell;
pub const COPY: IconName = IconName::Copy;
pub const PAPERCLIP: IconName = IconName::Copy;
pub const AT_SIGN: IconName = IconName::Info;
pub const SLASH: IconName = IconName::Minus;
pub const WAND: IconName = IconName::Star;

// ── Tool activity ──────────────────────────────────────────
pub const PENCIL: IconName = IconName::File;
pub const TERMINAL: IconName = IconName::SquareTerminal;
pub const GIT_COMPARE: IconName = IconName::Replace;
pub const GIT_BRANCH: IconName = IconName::GitHub;
pub const GLOBE: IconName = IconName::Globe;
pub const LOADER: IconName = IconName::LoaderCircle;
pub const CHECKLIST: IconName = IconName::GalleryVerticalEnd;
pub const QUESTION: IconName = IconName::Info;

// ── Status ─────────────────────────────────────────────────
pub const CHECK: IconName = IconName::Check;
pub const X_MARK: IconName = IconName::Close;
pub const SHIELD_CHECK: IconName = IconName::CircleCheck;
pub const TRIANGLE_ALERT: IconName = IconName::TriangleAlert;

// ── Expand / collapse ──────────────────────────────────────
pub const CHEVRON_DOWN: IconName = IconName::ChevronDown;
pub const CHEVRON_RIGHT: IconName = IconName::ChevronRight;
pub const CHEVRON_LEFT: IconName = IconName::ChevronLeft;
pub const CLOSE: IconName = IconName::Close;
pub const MOON: IconName = IconName::Moon;
pub const SUN: IconName = IconName::Sun;
