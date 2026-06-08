//! Settings layout — workspace setup, models, appearance, and preferences.

use std::rc::Rc;
use std::sync::Arc;

use agent_protocol::AgentMode;
use gpui::{Entity, FontWeight, IntoElement, SharedString, Window, div, prelude::*, px};
use gpui_component::Icon;
use gpui_component::ThemeMode;

use crate::features::settings::state::SettingsSection;
use crate::shared::components::buttons::btn_ghost_icon;
use crate::shared::components::buttons::btn_ghost_label;
use crate::shared::components::dropdown::{
    DropdownAnchor, DropdownItem, PickerDropdownProps, picker_dropdown,
};
use crate::shared::components::workspace_readiness::{
    WorkspaceReadinessCardProps, workspace_readiness_card,
};
use crate::shared::state::TranscriptMode;
use crate::shared::state::WorkspaceReadiness;
use crate::shared::state::model_catalog::{DEFAULT_PROVIDER, provider_options};
use crate::tokens::Tokens;
use crate::tokens::icons;
use crate::tokens::motion::page_fade_in;
use crate::ui::agent_window::AgentWindow;

fn section_icon(section: SettingsSection) -> gpui_component::IconName {
    match section {
        SettingsSection::Start => icons::CHECKLIST,
        SettingsSection::Models => icons::BOT,
        SettingsSection::Safety => icons::SHIELD_CHECK,
        SettingsSection::Appearance => icons::SUN,
        SettingsSection::Transcript => icons::MESSAGE_SQUARE,
        SettingsSection::Project => icons::FOLDER,
        SettingsSection::About => icons::QUESTION,
    }
}

/// Props for the settings page.
pub struct SettingsProps {
    pub dark_mode: bool,
    pub active_theme: String,
    pub themes: Vec<SharedString>,
    pub selected_section: SettingsSection,
    pub selected_provider: String,
    pub selected_model: String,
    pub safety_mode: AgentMode,
    pub transcript_mode: TranscriptMode,
    pub selected_subagent_model: Option<String>,
    pub model_items: Arc<[String]>,
    pub model_search_keys: Arc<[Arc<str>]>,
    pub workspace_readiness: WorkspaceReadiness,
    pub entity: Entity<AgentWindow>,
    pub on_transcript_mode: Option<Box<dyn Fn(TranscriptMode, &mut gpui::App) + 'static>>,
    pub on_open_project: Option<Box<dyn Fn(&mut Window, &mut gpui::App) + 'static>>,
    pub on_trust_project: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_open_context: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
}

pub fn render_settings(props: SettingsProps) -> impl IntoElement {
    let entity = props.entity;
    let themes = props.themes;
    let active_theme = props.active_theme;
    let dark = props.dark_mode;
    let selected_section = props.selected_section;
    let selected_provider = props.selected_provider;
    let selected_model = props.selected_model;
    let safety_mode = props.safety_mode;
    let transcript_mode = props.transcript_mode;
    let selected_subagent_model = props.selected_subagent_model;
    let model_items = props.model_items;
    let model_search_keys = props.model_search_keys;
    let workspace_readiness = props.workspace_readiness;
    let on_transcript_mode = props.on_transcript_mode;
    let on_open_project = props.on_open_project.map(Rc::from);
    let on_trust_project = props.on_trust_project.map(Rc::from);
    let on_open_context = props.on_open_context.map(Rc::from);

    page_fade_in(
        div()
            .id("settings-page")
            .h_full()
            .w_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(Tokens::main_bg())
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .px(Tokens::thread_padding_x())
                            .pt(Tokens::spacing_6())
                            .pb(Tokens::spacing_8())
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(820.0))
                                    .flex()
                                    .flex_col()
                                    .gap(Tokens::spacing_5())
                                    .child(render_header(entity.clone()))
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .items_start()
                                            .gap(Tokens::spacing_6())
                                            .child(div().w(px(176.0)).flex_shrink_0().child(
                                                render_section_nav(
                                                    selected_section,
                                                    entity.clone(),
                                                ),
                                            ))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .max_w(px(620.0))
                                                    .child(render_section_content(
                                                        selected_section,
                                                        SettingsSectionContentProps {
                                                            dark,
                                                            active_theme,
                                                            themes,
                                                            selected_provider,
                                                            selected_model,
                                                            safety_mode,
                                                            transcript_mode,
                                                            selected_subagent_model,
                                                            model_items,
                                                            model_search_keys,
                                                            workspace_readiness,
                                                            entity,
                                                            on_transcript_mode,
                                                            on_open_project,
                                                            on_trust_project,
                                                            on_open_context,
                                                        },
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            ),
    )
}

fn render_header(entity: Entity<AgentWindow>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .w_full()
        .max_w(px(820.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .child(
                    btn_ghost_icon("settings-back", icons::CHEVRON_LEFT).on_click(
                        move |_, _, app: &mut gpui::App| {
                            entity.update(app, |view, cx| view.close_settings(cx));
                        },
                    ),
                )
                .child(
                    div()
                        .text_size(Tokens::text_lg())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::text_primary())
                        .child("Settings"),
                ),
        )
        .child(
            div()
                .text_size(Tokens::text_sm())
                .line_height(Tokens::text_sm_leading())
                .text_color(Tokens::text_tertiary())
                .child("Workspace setup, model choices, appearance, and agent guardrails."),
        )
}

fn render_section_nav(selected: SettingsSection, entity: Entity<AgentWindow>) -> impl IntoElement {
    div().flex().flex_col().gap(Tokens::spacing_0p5()).children(
        SettingsSection::ALL.into_iter().map(|section| {
            let is_selected = selected == section;
            let section_entity = entity.clone();
            div()
                .id(gpui::ElementId::from(SharedString::from(format!(
                    "settings-section-{}",
                    section.label().to_ascii_lowercase()
                ))))
                .h(px(Tokens::ROW_HEIGHT_MD))
                .px(Tokens::spacing_2())
                .rounded(Tokens::radius_xs())
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .cursor_pointer()
                .when(is_selected, |el| el.bg(Tokens::surface_active()))
                .when(!is_selected, |el| {
                    el.hover(|s| s.bg(Tokens::surface_hover()))
                })
                .on_click(move |_, _, app: &mut gpui::App| {
                    section_entity.update(app, |view, cx| view.set_settings_section(section, cx));
                })
                .child(
                    Icon::new(section_icon(section))
                        .size(px(14.0))
                        .text_color(if is_selected {
                            Tokens::text_primary()
                        } else {
                            Tokens::text_tertiary()
                        }),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(if is_selected {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if is_selected {
                            Tokens::text_primary()
                        } else {
                            Tokens::text_secondary()
                        })
                        .child(section.label()),
                )
                .into_any_element()
        }),
    )
}

struct SettingsSectionContentProps {
    dark: bool,
    active_theme: String,
    themes: Vec<SharedString>,
    selected_provider: String,
    selected_model: String,
    safety_mode: AgentMode,
    transcript_mode: TranscriptMode,
    selected_subagent_model: Option<String>,
    model_items: Arc<[String]>,
    model_search_keys: Arc<[Arc<str>]>,
    workspace_readiness: WorkspaceReadiness,
    entity: Entity<AgentWindow>,
    on_transcript_mode: Option<Box<dyn Fn(TranscriptMode, &mut gpui::App) + 'static>>,
    on_open_project: Option<Rc<dyn Fn(&mut Window, &mut gpui::App)>>,
    on_trust_project: Option<Rc<dyn Fn(&mut gpui::App)>>,
    on_open_context: Option<Rc<dyn Fn(&mut gpui::App)>>,
}

fn render_section_content(
    section: SettingsSection,
    props: SettingsSectionContentProps,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_5())
        .when(section == SettingsSection::Start, |el| {
            el.child(render_workspace_readiness_section(
                props.workspace_readiness,
                props.on_open_project.clone(),
                props.on_trust_project,
                props.on_open_context,
            ))
        })
        .when(section == SettingsSection::Models, |el| {
            el.child(render_provider_section(
                props.selected_provider.clone(),
                props.entity.clone(),
            ))
            .child(render_model_section(
                props.selected_model.clone(),
                props.model_items.clone(),
                props.model_search_keys.clone(),
                props.entity.clone(),
            ))
            .child(render_subagent_model_section(
                props.selected_subagent_model.clone(),
                props.model_items.clone(),
                props.model_search_keys.clone(),
                props.entity.clone(),
            ))
        })
        .when(section == SettingsSection::Appearance, |el| {
            el.child(render_appearance_section(props.dark, props.entity.clone()))
                .child(render_themes_section(
                    &props.themes,
                    &props.active_theme,
                    props.entity.clone(),
                ))
        })
        .when(section == SettingsSection::Safety, |el| {
            el.child(render_agent_mode_section(
                props.safety_mode,
                props.entity.clone(),
            ))
        })
        .when(section == SettingsSection::Transcript, |el| {
            el.child(render_transcript_mode_section(
                props.transcript_mode,
                props.on_transcript_mode,
            ))
        })
        .when(section == SettingsSection::Project, |el| {
            el.child(render_project_section(props.on_open_project))
        })
        .when(section == SettingsSection::About, |el| {
            el.child(render_about_section())
        })
}

fn render_agent_mode_section(mode: AgentMode, entity: Entity<AgentWindow>) -> impl IntoElement {
    let modes = [
        (
            AgentMode::ApplyWithApproval,
            "Apply with approval",
            "Default: patches and commands need approval",
        ),
        (
            AgentMode::ReadOnlyInspect,
            "Read only",
            "Inspect project files and virtual bash only",
        ),
        (
            AgentMode::SuggestPatch,
            "Suggest patches",
            "Propose diffs without applying",
        ),
        (
            AgentMode::PlanOnly,
            "Plan",
            "Read-only planning; produces a reviewed implementation plan",
        ),
        (
            AgentMode::AutoSafe,
            "Auto safe",
            "Auto-run low-risk tools; approval for the rest",
        ),
    ];

    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label("Agent mode"))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1p5())
                .children(modes.into_iter().map(|(m, title, subtitle)| {
                    mode_row(title, subtitle, mode.clone() == m, m, entity.clone())
                })),
        )
}

fn mode_row(
    title: &str,
    subtitle: &str,
    selected: bool,
    mode: AgentMode,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let title = title.to_string();
    let subtitle = subtitle.to_string();

    div()
        .id(gpui::ElementId::from(SharedString::from(format!(
            "mode-{title}"
        ))))
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_XL))
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .rounded(Tokens::radius_xs())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .cursor_pointer()
        .when(selected, |el| el.bg(Tokens::surface_active()))
        .when(!selected, |el| el.hover(|s| s.bg(Tokens::surface_hover())))
        .on_click(move |_, _, app: &mut gpui::App| {
            entity.update(app, |view, cx| view.set_safety_mode(mode.clone(), cx));
        })
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_0p5())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(subtitle),
                ),
        )
        .when(selected, |el| {
            el.child(
                Icon::new(icons::CHECK)
                    .size(px(14.0))
                    .text_color(Tokens::accent()),
            )
        })
}

fn render_provider_section(selected: String, entity: Entity<AgentWindow>) -> impl IntoElement {
    let selected = if selected.is_empty() {
        DEFAULT_PROVIDER.to_string()
    } else {
        selected
    };
    let options = provider_options();
    let selected_icon = options
        .iter()
        .find(|opt| opt.name.as_ref() == selected)
        .map(|opt| opt.icon.clone());
    let items = options
        .into_iter()
        .map(|opt| DropdownItem {
            label: opt.name.to_string(),
            icon: Some(opt.icon),
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label("Provider"))
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child("Choose the provider used for new agent runs."),
        )
        .child(
            div()
                .w_full()
                .max_w(px(360.0))
                .child(picker_dropdown(PickerDropdownProps {
                    id: "settings-provider".into(),
                    label: selected.clone(),
                    items,
                    selected: Some(selected),
                    anchor: DropdownAnchor::Below,
                    menu_min_width: 220.0,
                    trigger_icon: selected_icon,
                    searchable: false,
                    search_texts: None,
                    search_placeholder: None,
                    on_select: Rc::new(move |_, provider, app| {
                        entity.update(app, |view, cx| {
                            view.on_provider_selected(provider, cx);
                        });
                    }),
                })),
        )
}

fn render_model_section(
    selected: String,
    model_items: Arc<[String]>,
    model_search_keys: Arc<[Arc<str>]>,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let selected_label = if selected.is_empty() {
        model_items
            .first()
            .cloned()
            .unwrap_or_else(|| "Default model".to_string())
    } else {
        selected
    };
    let items: Vec<_> = model_items
        .iter()
        .map(|name| DropdownItem {
            label: name.clone(),
            icon: Some(icons::BOT),
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label("Model"))
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child("Default model for the parent agent run."),
        )
        .child(
            div()
                .w_full()
                .max_w(px(360.0))
                .child(picker_dropdown(PickerDropdownProps {
                    id: "settings-model".into(),
                    label: selected_label.clone(),
                    items,
                    selected: Some(selected_label),
                    anchor: DropdownAnchor::Below,
                    menu_min_width: 300.0,
                    trigger_icon: Some(icons::BOT),
                    searchable: true,
                    search_texts: Some(model_search_keys.iter().cloned().collect()),
                    search_placeholder: Some("Search models...".into()),
                    on_select: Rc::new(move |_, model, app| {
                        entity.update(app, |view, cx| {
                            view.on_model_selected(model, cx);
                        });
                    }),
                })),
        )
}

fn render_subagent_model_section(
    selected: Option<String>,
    model_items: Arc<[String]>,
    model_search_keys: Arc<[Arc<str>]>,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let fallback_label = "Use parent model".to_string();
    let selected_label = selected.clone().unwrap_or_else(|| fallback_label.clone());
    let mut items = vec![DropdownItem {
        label: fallback_label.clone(),
        icon: Some(icons::BOT),
    }];
    items.extend(model_items.iter().map(|name| DropdownItem {
        label: name.clone(),
        icon: Some(icons::BOT),
    }));
    let mut search_texts = vec![Arc::from("use parent model fallback")];
    search_texts.extend(model_search_keys.iter().cloned());

    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label("Subagent model"))
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child("Model used for delegated child runs. Parent model is used when unset."),
        )
        .child(
            div()
                .w_full()
                .max_w(px(360.0))
                .child(picker_dropdown(PickerDropdownProps {
                    id: "settings-subagent-model".into(),
                    label: selected_label.clone(),
                    items,
                    selected: Some(selected_label),
                    anchor: DropdownAnchor::Below,
                    menu_min_width: 300.0,
                    trigger_icon: Some(icons::BOT),
                    searchable: true,
                    search_texts: Some(search_texts),
                    search_placeholder: Some("Search models...".into()),
                    on_select: Rc::new(move |_, model, app| {
                        let next = if model == fallback_label {
                            None
                        } else {
                            Some(model)
                        };
                        entity.update(app, |view, cx| {
                            view.on_subagent_model_selected(next, cx);
                        });
                    }),
                })),
        )
}

fn render_workspace_readiness_section(
    readiness: WorkspaceReadiness,
    on_open_project: Option<Rc<dyn Fn(&mut Window, &mut gpui::App)>>,
    on_trust_project: Option<Rc<dyn Fn(&mut gpui::App)>>,
    on_open_context: Option<Rc<dyn Fn(&mut gpui::App)>>,
) -> impl IntoElement {
    let show_trust = readiness.has_project && !readiness.project_trusted;
    let show_context = readiness.has_project;

    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label("Start here"))
        .child(workspace_readiness_card(WorkspaceReadinessCardProps {
            readiness,
            on_open_settings: None,
            on_open_project,
            on_trust_project: if show_trust { on_trust_project } else { None },
            on_open_context: if show_context { on_open_context } else { None },
        }))
}

fn render_appearance_section(dark: bool, entity: Entity<AgentWindow>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label("Appearance"))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1p5())
                .child(appearance_row(
                    "Dark",
                    "Dimmed surfaces for low-light environments",
                    icons::MOON,
                    Tokens::accent(),
                    dark,
                    ThemeMode::Dark,
                    entity.clone(),
                ))
                .child(appearance_row(
                    "Light",
                    "Bright surfaces for daytime use",
                    icons::SUN,
                    Tokens::warning(),
                    !dark,
                    ThemeMode::Light,
                    entity,
                )),
        )
}

fn appearance_row(
    title: &str,
    subtitle: &str,
    icon: gpui_component::IconName,
    icon_color: gpui::Hsla,
    selected: bool,
    mode: ThemeMode,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let title = title.to_string();
    let subtitle = subtitle.to_string();

    div()
        .id(gpui::ElementId::from(SharedString::from(format!(
            "appearance-{title}"
        ))))
        .w_full()
        .h(px(Tokens::ROW_HEIGHT_XL))
        .px(Tokens::spacing_3())
        .rounded(Tokens::radius_xs())
        .when(selected, |el| el.bg(Tokens::surface_active()))
        .when(!selected, |el| el.hover(|s| s.bg(Tokens::surface_hover())))
        .flex()
        .items_center()
        .gap(Tokens::spacing_3())
        .cursor_pointer()
        .child(
            div()
                .size(px(32.0))
                .rounded(Tokens::radius_md())
                .bg(icon_color.opacity(if selected { 0.18 } else { 0.08 }))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).size(px(16.0)).text_color(if selected {
                    icon_color
                } else {
                    icon_color.opacity(0.85)
                })),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(subtitle),
                ),
        )
        .when(selected, |el| {
            el.child(
                Icon::new(icons::CHECK)
                    .size(px(14.0))
                    .text_color(Tokens::accent()),
            )
        })
        .on_click(move |_, window, app: &mut gpui::App| {
            entity.update(app, |view, cx| {
                view.set_appearance_mode(mode, window, cx);
            });
        })
}

fn render_themes_section(
    themes: &[SharedString],
    active: &str,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let cards: Vec<_> = themes
        .iter()
        .map(|name| {
            theme_card(name.as_ref(), name.as_ref() == active, entity.clone()).into_any_element()
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label(
            "Color theme",
        ))
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child("Themes for the current appearance mode. Drop .json files in ~/.vortex/themes/ to add more."),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(Tokens::spacing_1p5())
                .children(cards),
        )
}

fn theme_card(name: &str, selected: bool, entity: Entity<AgentWindow>) -> impl IntoElement {
    let name_owned = name.to_string();
    let label = name_owned.clone();
    let swatches = theme_swatches(&name_owned);

    div()
        .id(gpui::ElementId::from(SharedString::from(format!(
            "theme-{name_owned}"
        ))))
        .w_full()
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2p5())
        .rounded(Tokens::radius_xs())
        .when(selected, |el| el.bg(Tokens::surface_active()))
        .when(!selected, |el| el.hover(|s| s.bg(Tokens::surface_hover())))
        .cursor_pointer()
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_3())
                .child(
                    div()
                        .flex()
                        .gap(px(3.0))
                        .children(swatches.into_iter().map(|color| {
                            div()
                                .size(px(18.0))
                                .rounded(Tokens::radius_xs())
                                .border_1()
                                .border_color(color.opacity(0.35))
                                .bg(color)
                                .into_any_element()
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_primary())
                        .child(label),
                )
                .when(selected, |el| {
                    el.child(
                        Icon::new(icons::CHECK)
                            .size(px(14.0))
                            .text_color(Tokens::accent()),
                    )
                }),
        )
        .on_click(move |_, window, app: &mut gpui::App| {
            let theme_name = name_owned.clone();
            entity.update(app, |view, cx| {
                view.apply_color_theme(&theme_name, Some(window), cx);
            });
        })
}

fn theme_swatches(name: &str) -> [gpui::Hsla; 4] {
    match name {
        "Vortex Light" => [
            gpui::rgb(0xf8f9fb).into(),
            gpui::rgb(0xf0f1f5).into(),
            gpui::rgb(0x5b6be6).into(),
            gpui::rgb(0x1a1d26).into(),
        ],
        "Vortex Paper" => [
            gpui::rgb(0xfafaf9).into(),
            gpui::rgb(0xf5f5f4).into(),
            gpui::rgb(0x0d9488).into(),
            gpui::rgb(0x1c1917).into(),
        ],
        "Vortex Midnight" => [
            gpui::rgb(0x06070a).into(),
            gpui::rgb(0x101218).into(),
            gpui::rgb(0xa78bfa).into(),
            gpui::rgb(0xe8e9ed).into(),
        ],
        "Vortex Nord" => [
            gpui::rgb(0x2e3440).into(),
            gpui::rgb(0x3b4252).into(),
            gpui::rgb(0x88c0d0).into(),
            gpui::rgb(0xeceff4).into(),
        ],
        _ => [
            gpui::rgb(0x0b0d0e).into(),
            gpui::rgb(0x090a0b).into(),
            gpui::rgb(0x7aa2ff).into(),
            gpui::rgb(0xe6e6e6).into(),
        ],
    }
}

fn render_about_section() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label("About"))
        .child(
            div().py(Tokens::spacing_1()).child(
                div()
                    .text_size(Tokens::text_sm())
                    .text_color(Tokens::text_secondary())
                    .child("Vortex Agent UI — community themes live in ~/.vortex/themes/"),
            ),
        )
}

// ── Transcript mode ──

fn render_transcript_mode_section(
    mode: TranscriptMode,
    on_select: Option<Box<dyn Fn(TranscriptMode, &mut gpui::App) + 'static>>,
) -> impl IntoElement {
    let on_select: Option<std::rc::Rc<dyn Fn(TranscriptMode, &mut gpui::App) + 'static>> =
        on_select.map(|f| std::rc::Rc::from(f));
    let modes = [
        (
            TranscriptMode::Summary,
            "Summary",
            "Minimal journal: user turns, final summaries, errors, approvals",
        ),
        (
            TranscriptMode::Normal,
            "Normal",
            "Default work journal: compact tool rows with expandable thinking summaries",
        ),
        (
            TranscriptMode::Verbose,
            "Verbose",
            "Debug-friendly: raw tool I/O and full reasoning steps",
        ),
    ];

    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(
            crate::shared::components::section_label::settings_section_label("Transcript density"),
        )
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child("Controls how much detail appears in the conversation thread"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1p5())
                .children(modes.into_iter().map(|(m, title, subtitle)| {
                    let selected = mode == m;
                    let cb = on_select.clone();
                    transcript_mode_row(title, subtitle, selected, m, cb)
                })),
        )
}

fn transcript_mode_row(
    title: &str,
    subtitle: &str,
    selected: bool,
    mode: TranscriptMode,
    on_select: Option<std::rc::Rc<dyn Fn(TranscriptMode, &mut gpui::App) + 'static>>,
) -> impl IntoElement {
    let title = title.to_string();
    let subtitle = subtitle.to_string();

    div()
        .id(gpui::ElementId::from(SharedString::from(format!(
            "transcript-{title}"
        ))))
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_XL))
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .rounded(Tokens::radius_xs())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .when(selected, |el| el.bg(Tokens::surface_active()))
        .when(!selected, |el| el.hover(|s| s.bg(Tokens::surface_hover())))
        .when(on_select.is_some(), |el| el.cursor_pointer())
        .when_some(on_select, |el, cb| {
            el.on_click(move |_, _, app: &mut gpui::App| {
                cb(mode, app);
            })
        })
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_0p5())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(subtitle),
                ),
        )
        .when(selected, |el| {
            el.child(
                Icon::new(icons::CHECK)
                    .size(px(14.0))
                    .text_color(Tokens::accent()),
            )
        })
}

// ── Project folder ──

fn render_project_section(
    on_open_project: Option<Rc<dyn Fn(&mut Window, &mut gpui::App)>>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(crate::shared::components::section_label::settings_section_label("Project"))
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child("Open a project folder to start working"),
        )
        .when_some(on_open_project, |el, cb| {
            el.child(
                btn_ghost_label("open-project-folder", "Open project folder…")
                    .icon(icons::OPEN_IDE)
                    .on_click(move |_, window, app: &mut gpui::App| cb(window, app)),
            )
        })
}
