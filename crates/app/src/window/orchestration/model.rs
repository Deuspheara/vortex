//! Model orchestration — on_provider_selected, on_model_selected, apply_openrouter_models.

use std::sync::Arc;

use gpui::Context;

use super::super::{AgentWindow, ModelPickerCache};
use crate::shared::state::{
    model_option_from_openrouter, model_search_key, models_for_provider, pricing_map_from_models,
};

impl AgentWindow {
    pub fn on_provider_selected(&mut self, provider: String, cx: &mut Context<Self>) {
        self.selected_provider = provider;
        self.invalidate_model_picker_cache();
        if let Some(first) =
            models_for_provider(&self.selected_provider, &self.openrouter_models).first()
        {
            self.selected_model = first.name.to_string();
        }
        self.status.model = self.selected_model.clone();
        self.refresh_token_usage_display();
        cx.notify();
    }

    pub fn on_model_selected(&mut self, model: String, cx: &mut Context<Self>) {
        self.selected_model = model;
        self.status.model = self.selected_model.clone();
        self.refresh_token_usage_display();
        cx.notify();
    }

    pub fn on_subagent_model_selected(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        self.selected_subagent_model = model;
        cx.notify();
    }

    pub fn apply_openrouter_models(
        &mut self,
        models: Vec<agent_models::OpenRouterModelInfo>,
        cx: &mut Context<Self>,
    ) {
        self.openrouter_models = models.iter().map(model_option_from_openrouter).collect();
        self.model_pricing = pricing_map_from_models(&self.openrouter_models);
        self.openrouter_models_revision = self.openrouter_models_revision.wrapping_add(1);
        self.invalidate_model_picker_cache();
        cx.notify();
    }

    pub fn model_picker_items_for_selected_provider(&mut self) -> (Arc<[String]>, Arc<[Arc<str>]>) {
        let _profile =
            crate::shared::render_profile::span("model_picker_items_for_selected_provider");
        let cache_valid = self.model_picker_cache.as_ref().is_some_and(|cache| {
            cache.provider == self.selected_provider
                && cache.openrouter_revision == self.openrouter_models_revision
        });
        if !cache_valid {
            self.refresh_model_picker_cache();
        }

        let cache = self
            .model_picker_cache
            .as_ref()
            .expect("model picker cache refreshed above");
        (cache.items.clone(), cache.search_keys.clone())
    }

    fn invalidate_model_picker_cache(&mut self) {
        self.model_picker_cache = None;
    }

    fn refresh_model_picker_cache(&mut self) {
        let models = models_for_provider(&self.selected_provider, &self.openrouter_models);
        let items = models
            .iter()
            .map(|model| model.name.to_string())
            .collect::<Vec<_>>();
        let search_keys = models.iter().map(model_search_key).collect::<Vec<_>>();
        self.model_picker_cache = Some(ModelPickerCache {
            provider: self.selected_provider.clone(),
            openrouter_revision: self.openrouter_models_revision,
            items: Arc::from(items),
            search_keys: Arc::from(search_keys),
        });
    }

    pub fn selected_model_supports_image_input(&self) -> bool {
        models_for_provider(&self.selected_provider, &self.openrouter_models)
            .into_iter()
            .find(|model| model.name.as_ref() == self.selected_model)
            .map(|model| model.supports_image_input)
            .unwrap_or(false)
    }
}
