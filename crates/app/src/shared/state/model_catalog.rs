//! Provider / model catalog for composer Select pickers.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{AnyElement, FontWeight, Hsla, SharedString, Task, div, prelude::*, px};
use gpui_component::IndexPath;
use gpui_component::select::{SearchableVec, SelectDelegate, SelectGroup, SelectItem, SelectState};
use gpui_component::{Icon, IconName, Sizable, Size, h_flex};

use crate::tokens::Tokens;

/// Per-token pricing for cost estimation.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModelPricing {
    pub prompt_per_token: f64,
    pub completion_per_token: f64,
}

/// Compact provider entry for the composer toolbar.
#[derive(Clone)]
pub struct ProviderOption {
    pub name: SharedString,
    pub icon: IconName,
}

impl ProviderOption {
    pub fn new(name: impl Into<SharedString>, icon: IconName) -> Self {
        Self {
            name: name.into(),
            icon,
        }
    }
}

impl SelectItem for ProviderOption {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(self.icon.clone()).with_size(Size::Small))
                .child(div().text_sm().child(self.name.clone()))
                .into_any_element(),
        )
    }

    fn matches(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(&query.to_lowercase())
    }
}

/// Rich model entry — compact in the trigger, detailed in the popup menu.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ModelOption {
    pub provider: SharedString,
    pub name: SharedString,
    pub context: SharedString,
    pub capability: SharedString,
    pub latency_hint: SharedString,
    pub cost_hint: SharedString,
    pub prompt_per_token: Option<f64>,
    pub completion_per_token: Option<f64>,
    pub supports_image_input: bool,
}

impl ModelOption {
    #[allow(dead_code)]
    pub fn id(&self) -> SharedString {
        format!("{}::{}", self.provider, self.name).into()
    }

    pub fn pricing(&self) -> Option<ModelPricing> {
        match (self.prompt_per_token, self.completion_per_token) {
            (Some(p), Some(c)) => Some(ModelPricing {
                prompt_per_token: p,
                completion_per_token: c,
            }),
            _ => None,
        }
    }

    pub fn context_tokens(&self) -> Option<u64> {
        parse_context_tokens(&self.context)
    }
}

fn badge(text: SharedString, fg: Hsla) -> impl IntoElement {
    div()
        .text_xs()
        .px_1p5()
        .py_0p5()
        .rounded_md()
        .bg(Tokens::surface_active())
        .text_color(fg)
        .child(text)
}

impl SelectItem for ModelOption {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        format!("{} · {}", self.provider, self.name).into()
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.provider.as_ref().to_lowercase().contains(&q)
            || self.name.as_ref().to_lowercase().contains(&q)
            || self.capability.as_ref().to_lowercase().contains(&q)
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(
            div()
                .text_sm()
                .font_weight(FontWeight::NORMAL)
                .text_color(Tokens::text_secondary())
                .overflow_hidden()
                .text_ellipsis()
                .child(self.name.clone())
                .into_any_element(),
        )
    }

    fn render(&self, _: &mut gpui::Window, _: &mut gpui::App) -> impl gpui::IntoElement {
        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(self.name.clone()),
            )
            .when(!self.context.is_empty(), |el| {
                el.child(badge(self.context.clone(), Tokens::text_tertiary()))
            })
            .when(!self.cost_hint.is_empty(), |el| {
                el.child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(Tokens::text_faint())
                        .child(self.cost_hint.clone()),
                )
            })
    }
}

/// All supported LLM providers.
pub fn provider_options() -> Vec<ProviderOption> {
    vec![
        ProviderOption::new("Anthropic", IconName::Bot),
        ProviderOption::new("OpenAI", IconName::Star),
        ProviderOption::new("Google", IconName::Globe),
        ProviderOption::new("Local", IconName::SquareTerminal),
        ProviderOption::new("OpenRouter", IconName::ExternalLink),
    ]
}

fn model(
    provider: &str,
    name: &str,
    context: &str,
    capability: &str,
    latency: &str,
    cost: &str,
) -> ModelOption {
    ModelOption {
        provider: provider.to_string().into(),
        name: name.to_string().into(),
        context: context.to_string().into(),
        capability: capability.to_string().into(),
        latency_hint: latency.to_string().into(),
        cost_hint: cost.to_string().into(),
        prompt_per_token: None,
        completion_per_token: None,
        supports_image_input: model_name_supports_image_input(provider, name),
    }
}

fn model_name_supports_image_input(provider: &str, name: &str) -> bool {
    let value = format!("{provider}/{name}").to_ascii_lowercase();
    value.contains("openrouter/auto")
        || value.contains("gpt-4o")
        || value.contains("gemini")
        || value.contains("claude-sonnet")
        || value.contains("claude-opus")
}

fn static_openrouter_models() -> Vec<ModelOption> {
    vec![
        model("OpenRouter", "openrouter/auto", "200k", "", "~1.0s", "$$"),
        model(
            "OpenRouter",
            "anthropic/claude-sonnet-4",
            "200k",
            "thinking",
            "~1.2s",
            "$$",
        ),
        model(
            "OpenRouter",
            "google/gemini-2.5-flash-preview",
            "1M",
            "fast",
            "~0.4s",
            "$",
        ),
    ]
}

/// Models for a single provider (flat list).
pub fn models_for_provider(provider: &str, openrouter_models: &[ModelOption]) -> Vec<ModelOption> {
    match provider {
        "Anthropic" => vec![
            model(
                "Anthropic",
                "Claude Sonnet 4.5",
                "200k",
                "thinking",
                "~1.2s",
                "$$",
            ),
            model(
                "Anthropic",
                "Claude Opus 4",
                "200k",
                "thinking",
                "~2.5s",
                "$$$",
            ),
            model("Anthropic", "Claude Haiku", "200k", "fast", "~0.6s", "$"),
        ],
        "OpenAI" => vec![
            model("OpenAI", "GPT-4o", "128k", "", "~1.0s", "$$"),
            model("OpenAI", "GPT-4o mini", "128k", "fast", "~0.5s", "$"),
            model("OpenAI", "o3-mini", "200k", "thinking", "~3.0s", "$$"),
        ],
        "Google" => vec![
            model("Google", "Gemini 2.5 Pro", "1M", "thinking", "~1.5s", "$$"),
            model("Google", "Gemini 2.5 Flash", "1M", "fast", "~0.4s", "$"),
        ],
        "Local" => vec![
            model("Local", "Local Model", "32k", "", "~0.2s", "free"),
            model("Local", "Ollama", "128k", "", "~0.3s", "free"),
        ],
        "OpenRouter" => {
            if openrouter_models.is_empty() {
                static_openrouter_models()
            } else {
                openrouter_models.to_vec()
            }
        }
        _ => vec![model(provider, "Unknown Model", "", "", "", "")],
    }
}

/// Format per-million token prices for display.
pub fn format_price_hint(prompt_per_token: f64, completion_per_token: f64) -> String {
    format!(
        "${}/M in · ${}/M out",
        format_per_million(prompt_per_token),
        format_per_million(completion_per_token),
    )
}

fn format_per_million(per_token: f64) -> String {
    let per_million = per_token * 1_000_000.0;
    if per_million >= 10.0 {
        format!("{per_million:.0}")
    } else if per_million >= 1.0 {
        format!("{per_million:.1}")
    } else {
        format!("{per_million:.2}")
    }
}

pub fn format_context_length(tokens: Option<u64>) -> String {
    match tokens {
        Some(n) if n >= 1_000_000 => format!("{}M", n / 1_000_000),
        Some(n) if n >= 1_000 => format!("{}K", n / 1_000),
        Some(n) => n.to_string(),
        None => String::new(),
    }
}

pub fn format_context_label(tokens: u64) -> String {
    format_context_length(Some(tokens))
}

fn parse_context_tokens(context: &str) -> Option<u64> {
    let s = context.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    if let Some(num) = s.strip_suffix('m') {
        return num.parse::<u64>().ok().map(|n| n * 1_000_000);
    }
    if let Some(num) = s.strip_suffix('k') {
        return num.parse::<u64>().ok().map(|n| n * 1_000);
    }
    s.parse().ok()
}

/// Build pricing map from model options (keyed by model slug/name).
pub fn pricing_map_from_models(models: &[ModelOption]) -> HashMap<String, ModelPricing> {
    models
        .iter()
        .filter_map(|m| m.pricing().map(|p| (m.name.to_string(), p)))
        .collect()
}

/// Estimate USD cost from token counts and pricing.
/// When OpenRouter does not return `usage.cost`, apply conservative cache discounts.
pub fn estimate_cost_usd(
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    pricing: &ModelPricing,
) -> f64 {
    let cache_read = cache_read_tokens.min(input_tokens);
    let uncached_input = input_tokens.saturating_sub(cache_read);
    const CACHE_READ_DISCOUNT: f64 = 0.1;
    const CACHE_WRITE_PREMIUM: f64 = 1.25;

    uncached_input as f64 * pricing.prompt_per_token
        + cache_read as f64 * pricing.prompt_per_token * CACHE_READ_DISCOUNT
        + cache_write_tokens as f64 * pricing.prompt_per_token * CACHE_WRITE_PREMIUM
        + output_tokens as f64 * pricing.completion_per_token
}

pub fn format_cost_usd(cost: f64) -> String {
    if cost >= 0.01 {
        format!("~${cost:.2}")
    } else if cost >= 0.001 {
        format!("~${cost:.3}")
    } else if cost > 0.0 {
        format!("~${cost:.4}")
    } else {
        "~$0".into()
    }
}

/// Find pricing for a model slug from the combined catalog.
pub fn pricing_for_model(
    provider: &str,
    model: &str,
    openrouter_models: &[ModelOption],
    extra_pricing: &HashMap<String, ModelPricing>,
) -> Option<ModelPricing> {
    let slug = openrouter_model_slug(provider, model);
    if let Some(p) = extra_pricing.get(&slug) {
        return Some(*p);
    }
    models_for_provider(provider, openrouter_models)
        .into_iter()
        .find(|m| m.name.as_ref() == model)
        .and_then(|m| m.pricing())
}

/// Context window tokens for the selected model.
pub fn context_for_model(provider: &str, model: &str, openrouter_models: &[ModelOption]) -> u64 {
    models_for_provider(provider, openrouter_models)
        .into_iter()
        .find(|m| m.name.as_ref() == model)
        .and_then(|m| m.context_tokens())
        .unwrap_or(200_000)
}

/// Index-based searchable delegate for the model Select.
///
/// Avoids cloning every [`ModelOption`] on each keystroke — the default
/// [`SearchableVec`] path clones all matches and recomputes `title()` per item,
/// which blocks the UI thread on large OpenRouter catalogs.
#[derive(Clone)]
pub struct FastSearchableModels {
    items: Vec<ModelOption>,
    search_keys: Arc<[Arc<str>]>,
    matched_indices: Vec<usize>,
}

impl FastSearchableModels {
    pub fn new(items: Vec<ModelOption>) -> Self {
        let search_keys: Arc<[Arc<str>]> = items.iter().map(model_search_key).collect();
        let matched_indices: Vec<usize> = (0..items.len()).collect();
        Self {
            items,
            search_keys,
            matched_indices,
        }
    }
}

pub fn model_search_key(model: &ModelOption) -> Arc<str> {
    Arc::from(
        format!(
            "{} {} {} {}",
            model.provider, model.name, model.capability, model.context
        )
        .to_lowercase(),
    )
}

impl SelectDelegate for FastSearchableModels {
    type Item = ModelOption;

    fn items_count(&self, _: usize) -> usize {
        self.matched_indices.len()
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.matched_indices
            .get(ix.row)
            .and_then(|&i| self.items.get(i))
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SelectItem<Value = V>,
        V: PartialEq,
    {
        self.matched_indices
            .iter()
            .enumerate()
            .find_map(|(row, &i)| {
                self.items
                    .get(i)
                    .filter(|item| item.value() == value)
                    .map(|_| IndexPath::new(row))
            })
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut gpui::Window,
        _: &mut gpui::Context<SelectState<Self>>,
    ) -> Task<()> {
        self.matched_indices = filter_model_indices(&self.search_keys, query);
        Task::ready(())
    }
}

fn filter_model_indices(search_keys: &[Arc<str>], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..search_keys.len()).collect();
    }
    search_keys
        .iter()
        .enumerate()
        .filter_map(|(i, key)| key.contains(&q).then_some(i))
        .collect()
}

#[cfg(test)]
mod fast_search_tests {
    use super::*;

    #[test]
    fn filter_matches_model_name() {
        let keys: Vec<Arc<str>> = vec![
            Arc::from("anthropic claude sonnet 4.5"),
            Arc::from("anthropic claude haiku"),
        ];
        let matched = filter_model_indices(&keys, "haiku");
        assert_eq!(matched, vec![1]);
    }

    #[test]
    fn empty_query_returns_all() {
        let keys: Vec<Arc<str>> = vec![Arc::from("a"), Arc::from("b")];
        assert_eq!(filter_model_indices(&keys, ""), vec![0, 1]);
    }
}

/// Grouped searchable delegate for the model Select.
#[allow(dead_code)]
pub type SearchableModelGroups = SearchableVec<SelectGroup<ModelOption>>;

/// All models grouped by provider for the searchable model Select.
#[allow(dead_code)]
pub fn all_models_grouped(openrouter_models: &[ModelOption]) -> Vec<SelectGroup<ModelOption>> {
    provider_options()
        .into_iter()
        .map(|p| {
            SelectGroup::new(p.name.clone())
                .items(models_for_provider(&p.name.to_string(), openrouter_models))
        })
        .collect()
}

/// Default provider on startup (mock / offline).
pub const DEFAULT_PROVIDER: &str = "Anthropic";

pub const OPENROUTER_PROVIDER: &str = "OpenRouter";

/// Default model for the default provider.
pub fn default_model() -> String {
    models_for_provider(DEFAULT_PROVIDER, &[])
        .into_iter()
        .next()
        .map(|m| m.name.to_string())
        .unwrap_or_else(|| "Claude Sonnet 4.5".into())
}

pub fn openrouter_default_model() -> String {
    models_for_provider(OPENROUTER_PROVIDER, &[])
        .into_iter()
        .next()
        .map(|m| m.name.to_string())
        .unwrap_or_else(|| "openrouter/auto".into())
}

/// Map composer selection to an OpenRouter model slug for the runtime API.
pub fn openrouter_model_slug(provider: &str, model: &str) -> String {
    if model.contains('/') {
        return model.to_string();
    }

    if provider == OPENROUTER_PROVIDER {
        return model.to_string();
    }

    match (provider, model) {
        ("Anthropic", "Claude Sonnet 4.5") => "anthropic/claude-sonnet-4".into(),
        ("Anthropic", "Claude Opus 4") => "anthropic/claude-opus-4".into(),
        ("Anthropic", "Claude Haiku") => "anthropic/claude-haiku-4".into(),
        ("OpenAI", "GPT-4o") => "openai/gpt-4o".into(),
        ("OpenAI", "GPT-4o mini") => "openai/gpt-4o-mini".into(),
        ("OpenAI", "o3-mini") => "openai/o3-mini".into(),
        ("Google", "Gemini 2.5 Pro") => "google/gemini-2.5-pro-preview".into(),
        ("Google", "Gemini 2.5 Flash") => "google/gemini-2.5-flash-preview".into(),
        _ => "openrouter/auto".into(),
    }
}

/// Find provider index by name.
#[allow(dead_code)]
pub fn provider_index_for_name(name: &str) -> usize {
    provider_options()
        .iter()
        .position(|p| p.name.as_ref() == name)
        .unwrap_or(0)
}

/// Find model index path in flat list by model name.
pub fn model_index_for_name(
    provider: &str,
    name: &str,
    openrouter_models: &[ModelOption],
) -> Option<IndexPath> {
    let models = models_for_provider(provider, openrouter_models);
    models
        .iter()
        .position(|m| m.name.as_ref() == name)
        .map(|row| IndexPath::new(row))
}

/// Backward-compat flat provider names.
#[allow(dead_code)]
pub fn providers() -> Vec<String> {
    provider_options()
        .into_iter()
        .map(|p| p.name.to_string())
        .collect()
}

/// Build a catalog entry from an OpenRouter API model.
pub fn model_option_from_openrouter(info: &agent_models::OpenRouterModelInfo) -> ModelOption {
    ModelOption {
        provider: OPENROUTER_PROVIDER.into(),
        name: info.id.clone().into(),
        context: format_context_length(info.context_length).into(),
        capability: if info.supports_image_input {
            "vision".into()
        } else {
            SharedString::default()
        },
        latency_hint: SharedString::default(),
        cost_hint: format_price_hint(info.prompt_per_token, info.completion_per_token).into(),
        prompt_per_token: Some(info.prompt_per_token),
        completion_per_token: Some(info.completion_per_token),
        supports_image_input: info.supports_image_input,
    }
}
