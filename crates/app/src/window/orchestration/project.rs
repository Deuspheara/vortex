//! Project orchestration — open/toggle/delete project, branch cycling.

use std::path::PathBuf;

use gpui::{Context, Window};

use super::super::AgentWindow;
use crate::agent::paths::git_local_branches;
use crate::agent::{AgentBridge, ui_conversation_id, ui_project_id};
use crate::features::shell::components::tree_row::project_expand_key;
use crate::features::shell::state::{Conversation, ConversationId, Project, ProjectId};

impl AgentWindow {
    pub fn trust_selected_project(&mut self, cx: &mut Context<Self>) {
        let Some(project_id) = self.selected_project().map(|project| project.id.clone()) else {
            return;
        };

        let proto_id = crate::agent::proto_project_id(&project_id);
        let Ok(stored) = self.agent_bridge.set_project_trusted(&proto_id, true) else {
            return;
        };

        if let Some(project) = self
            .projects
            .iter_mut()
            .find(|project| project.id == project_id)
        {
            project.trusted = stored.trusted;
        }
        self.refresh_indexing_state(cx);
        self.sync_sidebar_view(cx);
        cx.notify();
    }

    pub fn toggle_project(&mut self, project_id: ProjectId, cx: &mut Context<Self>) {
        let key = project_expand_key(&project_id);
        if self.expanded_items.contains(&key) {
            self.expanded_items.remove(&key);
        } else {
            self.expanded_items.insert(key);
        }
        self.sync_sidebar_view(cx);
    }

    pub fn cycle_project_branch(&mut self, project_id: ProjectId, cx: &mut Context<Self>) {
        let (root, branches, next_branch) = {
            let Some(project) = self.projects.iter().find(|p| p.id == project_id) else {
                return;
            };
            let branches = git_local_branches(std::path::Path::new(&project.root_path));
            let idx = branches
                .iter()
                .position(|b| *b == project.git_branch)
                .unwrap_or(0);
            (
                project.root_path.clone(),
                branches.clone(),
                branches[(idx + 1) % branches.len()].clone(),
            )
        };

        if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            project.git_branch = next_branch.clone();
        }
        self.branch_items_cache = Some((root, branches));

        self.sync_branch_chips_for_project(&project_id, &next_branch);

        cx.notify();
    }

    pub fn open_project_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bridge = self.agent_bridge.clone();
        let entity = cx.weak_entity();

        // Use the async dialog so the native NSOpenPanel runs as a non-blocking
        // sheet inside the Cocoa event loop.  The synchronous FileDialog would
        // block the main thread while the AppCell RefCell is borrowed, causing
        // a "RefCell already borrowed" panic if macOS processes events during
        // the nested run loop.
        let future = rfd::AsyncFileDialog::new().set_parent(window).pick_folder();

        cx.spawn(async move |_weak, cx| {
            let picked = future.await;
            let Some(handle) = picked else { return };
            let path = handle.path().to_path_buf();

            // Re-acquire the AgentWindow on the main thread via defer, which
            // runs after the current update cycle completes and the AppCell
            // borrow has been released.
            let _ = cx.update(|app| {
                app.defer(move |app| {
                    if let Some(view) = entity.upgrade() {
                        view.update(app, |view, cx| {
                            view.finish_open_project(path, &bridge, cx);
                        });
                    }
                });
            });
        })
        .detach();
    }

    fn finish_open_project(&mut self, path: PathBuf, bridge: &AgentBridge, cx: &mut Context<Self>) {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_string();

        let stored = match bridge.upsert_project(&path, &name) {
            Ok(p) => p,
            Err(err) => {
                tracing::error!("failed to open project: {err}");
                return;
            }
        };

        let project_id = ui_project_id(&stored.id);
        let git_branch = bridge.git_branch_for_path(&stored.root_path);
        let is_new = !self.projects.iter().any(|p| p.id == project_id);
        // A freshly opened/refreshed project may expose different branches.
        self.invalidate_branch_items_cache();

        if is_new {
            let project = Project::new(
                project_id.0.clone(),
                &stored.name,
                &stored.root_path,
                &git_branch,
                stored.trusted,
            );
            self.projects.push(project);
        } else if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            project.name = stored.name.clone();
            project.root_path = stored.root_path.clone();
            project.git_branch = git_branch;
            project.trusted = stored.trusted;
        }

        let session = match bridge.create_session(&stored.id, "New Conversation") {
            Ok(s) => s,
            Err(err) => {
                tracing::error!("failed to create session: {err}");
                return;
            }
        };

        let conv_id = ui_conversation_id(&session.id);
        let mut conv =
            Conversation::new(conv_id.0.clone(), project_id.clone(), &session.title, "now");
        conv.context_chips = self
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .map(Self::context_chips_for_project)
            .unwrap_or_default();

        if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            project.conversations.push(conv_id.clone());
        }
        self.conversations.push(conv);

        self.selected_project_id = Some(project_id.clone());
        self.selected_conversation_id = Some(conv_id.clone());
        self.refresh_branch_items_for_project(&project_id);
        self.expanded_items.insert(project_expand_key(&project_id));

        if let Some(thread) = &self.thread_view {
            thread.update(cx, |view, cx| {
                view.set_conversation(conv_id.clone(), vec![], cx);
            });
        }

        self.refresh_indexing_state(cx);
        self.sync_sidebar_view(cx);
        cx.notify();
    }

    pub fn confirm_delete_project(
        &mut self,
        project_id: ProjectId,
        name: String,
        session_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pid = project_id.clone();
        cx.defer_in(window, move |view, window, cx| {
            let description = if session_count == 1 {
                format!(
                    "Delete project \"{name}\" and its conversation? This cannot be undone."
                )
            } else {
                format!(
                    "Delete project \"{name}\" and {session_count} conversations? This cannot be undone."
                )
            };
            let confirmed = rfd::MessageDialog::new()
                .set_title("Delete project")
                .set_description(description)
                .set_level(rfd::MessageLevel::Warning)
                .set_buttons(rfd::MessageButtons::OkCancel)
                .set_parent(window)
                .show();
            if confirmed == rfd::MessageDialogResult::Ok {
                view.delete_project(pid, cx);
            }
        });
    }

    pub fn delete_project(&mut self, project_id: ProjectId, cx: &mut Context<Self>) {
        self.clear_sidebar_drag_state(cx);

        let conv_ids: Vec<ConversationId> = self
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .map(|p| p.conversations.clone())
            .unwrap_or_default();
        let was_selected_project = self.selected_project_id.as_ref() == Some(&project_id);
        let deleted_selected_session = self
            .selected_conversation_id
            .as_ref()
            .is_some_and(|id| conv_ids.contains(id));

        if deleted_selected_session && self.active_run_id.is_some() {
            self.cancel_active_run(cx);
        }

        for conv_id in &conv_ids {
            self.simulations_running.remove(conv_id);
            self.running_conversations.remove(conv_id);
            self.collapsed_sessions.remove(&conv_id.0);
            self.thread_item_indices.remove(conv_id);
        }

        self.conversations.retain(|c| c.project_id != project_id);
        self.projects.retain(|p| p.id != project_id);
        self.expanded_items.remove(&project_expand_key(&project_id));
        self.invalidate_branch_items_cache();

        let proto_id = crate::agent::proto_project_id(&project_id);
        if let Err(err) = self.agent_bridge.delete_project(&proto_id) {
            tracing::error!("failed to delete project: {err}");
        }

        if was_selected_project || deleted_selected_session {
            if let Some(project) = self.projects.first() {
                if let Some(conv_id) = project.conversations.first().cloned() {
                    self.select_conversation(conv_id, cx);
                    return;
                }
                self.create_conversation_in_project(project.id.clone(), cx);
                return;
            }

            self.selected_project_id = None;
            self.selected_conversation_id = None;
            self.bootstrap_workspace_session(cx);
            return;
        }

        self.sync_sidebar_view(cx);
        cx.notify();
    }
}
