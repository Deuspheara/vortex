//! Model orchestration — on_provider_selected, on_model_selected, apply_openrouter_models.

use std::sync::Arc;

use gpui::Context;

use super::super::{AgentWindow, ModelPickerCache};
use crate::shared::state::{
    model_option_from_openrouter, model_search_key, models_for_provider, pricing_map_from_models,
};

impl AgentWindow {
    pub fn apply_codex_status(
        &mut self,
        status: agent_codex::BackendStatus,
        cx: &mut Context<Self>,
    ) {
        self.openrouter_models
            .retain(|m| m.provider.as_ref() != "Codex");
        for model in &status.models {
            let efforts = if model.efforts.is_empty() {
                vec![String::new()]
            } else {
                model.efforts.clone()
            };
            for effort in efforts {
                self.openrouter_models
                    .push(crate::shared::state::ModelOption {
                        provider: "Codex".into(),
                        name: if effort.is_empty() {
                            model.id.clone()
                        } else {
                            format!("{} · {}", model.id, effort)
                        }
                        .into(),
                        context: "".into(),
                        capability: "Local Codex agent".into(),
                        latency_hint: "".into(),
                        cost_hint: "Your Codex account".into(),
                        prompt_per_token: None,
                        completion_per_token: None,
                        supports_image_input: model.images,
                    });
            }
        }
        self.openrouter_models_revision = self.openrouter_models_revision.wrapping_add(1);
        self.invalidate_model_picker_cache();
        let preference = self
            .agent_bridge
            .runtime
            .store
            .runtime_preference()
            .ok()
            .flatten();
        let has_history = self.selected_conversation_id.as_ref().is_some_and(|id| {
            !self
                .agent_bridge
                .runtime
                .store
                .load_session_events(&crate::agent::proto_session_id(id))
                .unwrap_or_default()
                .is_empty()
        });
        if status.state == agent_codex::ConnectionState::Ready
            && preference.is_none()
            && !has_history
        {
            self.selected_provider = "Codex".into();
        }
        if self.selected_provider == "Codex" && status.state == agent_codex::ConnectionState::Ready
        {
            let choices = models_for_provider("Codex", &self.openrouter_models);
            if !choices
                .iter()
                .any(|m| m.name.as_ref() == self.selected_model)
                && let Some(model) = status.models.first()
            {
                self.selected_model = if model.default_effort.is_empty() {
                    model.id.clone()
                } else {
                    format!("{} · {}", model.id, model.default_effort)
                };
            }
        }
        self.status.model = self.selected_model.clone();
        cx.notify();
    }

    pub(crate) fn restore_session_backend(&mut self) {
        let Some(cid) = &self.selected_conversation_id else {
            return;
        };
        let session = crate::agent::proto_session_id(cid);
        let store = &self.agent_bridge.runtime.store;
        if let Ok(Some(binding)) = store.session_backend(&session) {
            if binding.backend == "codex" {
                self.selected_provider = "Codex".into();
                if let Some(model) = binding.model {
                    self.selected_model = if let Some(effort) = binding.effort {
                        format!("{model} · {effort}")
                    } else {
                        model
                    };
                }
            } else if self.selected_provider == "Codex" {
                self.selected_provider = "OpenRouter".into();
                self.selected_model = binding
                    .model
                    .unwrap_or_else(crate::shared::state::openrouter_default_model);
            }
        } else if !store
            .load_session_events(&session)
            .unwrap_or_default()
            .is_empty()
            && self.selected_provider == "Codex"
        {
            self.selected_provider = "OpenRouter".into();
            self.selected_model = crate::shared::state::openrouter_default_model();
        }
        self.invalidate_model_picker_cache();
        self.status.model = self.selected_model.clone();
    }

    pub fn on_provider_selected(&mut self, provider: String, cx: &mut Context<Self>) {
        if let Some(cid) = &self.selected_conversation_id {
            let session = crate::agent::proto_session_id(cid);
            let store = &self.agent_bridge.runtime.store;
            if !store
                .load_session_events(&session)
                .unwrap_or_default()
                .is_empty()
            {
                let existing = store
                    .session_backend(&session)
                    .ok()
                    .flatten()
                    .map(|b| b.backend)
                    .unwrap_or_else(|| "native".into());
                if (provider == "Codex") != (existing == "codex") {
                    self.composer_error=Some("Start a new conversation to switch between Codex and Vortex's native agent.".into());
                    cx.notify();
                    return;
                }
            }
        }
        if let Err(e) = self
            .agent_bridge
            .runtime
            .store
            .save_runtime_preference(&provider)
        {
            self.composer_error = Some(e);
            cx.notify();
            return;
        }
        self.selected_provider = provider;
        if self.selected_provider == "Codex" {
            self.safety_mode = agent_protocol::AgentMode::ApplyWithApproval;
        }
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
        self.openrouter_models
            .retain(|m| m.provider.as_ref() == "Codex");
        self.openrouter_models
            .extend(models.iter().map(model_option_from_openrouter));
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
