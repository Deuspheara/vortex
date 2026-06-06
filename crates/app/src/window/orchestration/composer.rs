//! Composer orchestration — composer_scope, branch_items, on_composer_*, pending actions.

use std::path::{Path, PathBuf};

use gpui::{ClipboardEntry, Context, Entity, Window};

use super::super::AgentWindow;
use crate::agent::paths::git_branch_info;
use crate::features::agent_activity::components::approval::ApprovalCardProps;
use crate::features::agent_activity::components::pending_action_bar::{
    PatchActionProps, PendingActionBarProps,
};
use crate::features::composer::state::{PendingImageAttachment, PendingImageSource};
use crate::features::shell::state::{ChipKind, Project, ProjectId};

const SUPPORTED_IMAGE_MIME_TYPES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

impl AgentWindow {
    pub(crate) fn pending_action_bar_row_count(&self) -> u8 {
        let patch = self.diff_panel.pending_patch_id.is_some()
            && !self.diff_panel.applied
            && !self.safety_mode.auto_applies_patches();
        let approval = false;
        (patch as u8) + (approval as u8)
    }

    pub(crate) fn pending_composer_actions(
        &self,
        entity: Entity<Self>,
    ) -> (PendingActionBarProps, Option<ApprovalCardProps>) {
        let sticky_approval = self.pending_thread_approval.clone().map(|pending| {
            let reject_entity = entity.clone();
            let approve_entity = entity.clone();
            let approve_always_entity = entity.clone();
            ApprovalCardProps {
                title: pending.title,
                risk: pending.risk,
                can_act: self.has_pending_approval(),
                allow_always_label: pending.allow_always_label,
                on_deny: Box::new(move |app| {
                    reject_entity.update(app, |view, cx| {
                        view.reject_pending(None, cx);
                    });
                }),
                on_approve: Box::new(move |app| {
                    approve_entity.update(app, |view, cx| {
                        view.approve_pending(cx);
                    });
                }),
                on_approve_always: Some(Box::new(move |app| {
                    approve_always_entity.update(app, |view, cx| {
                        view.approve_pending_always(cx);
                    });
                })),
            }
        });

        let approval = None;

        let patch = if self.diff_panel.pending_patch_id.is_some()
            && !self.diff_panel.applied
            && !self.safety_mode.auto_applies_patches()
        {
            let files_changed = self.diff_panel.files.len();
            let (additions, deletions) = self
                .diff_panel
                .files
                .iter()
                .fold((0usize, 0usize), |(a, d), file| {
                    (a + file.added, d + file.removed)
                });
            let open_entity = entity.clone();
            let cancel_entity = entity;
            Some(PatchActionProps {
                summary: format!(
                    "Pending changes · {files_changed} files · +{additions} −{deletions}"
                ),
                on_open: Box::new(move |app| {
                    open_entity.update(app, |view, cx| {
                        view.open_diff_panel(cx);
                    });
                }),
                on_cancel: Box::new(move |app| {
                    cancel_entity.update(app, |view, cx| {
                        view.reject_pending_patch(Some("Cancelled by user".into()), cx);
                    });
                }),
            })
        } else {
            None
        };

        (PendingActionBarProps { approval, patch }, sticky_approval)
    }

    pub fn composer_scope(&self) -> (String, String, Option<ProjectId>) {
        let _profile = crate::shared::render_profile::span("composer_scope");
        if let Some(project) = self.selected_composer_project() {
            return (
                project.name.clone(),
                project.git_branch.clone(),
                Some(project.id.clone()),
            );
        }

        ("Select project".into(), "main".into(), None)
    }

    fn selected_composer_project(&self) -> Option<&Project> {
        if let Some(cid) = &self.selected_conversation_id {
            if let Some(conv) = self.conversations.iter().find(|c| c.id == *cid) {
                if let Some(project) = self.projects.iter().find(|p| p.id == conv.project_id) {
                    return Some(project);
                }
            }
        }

        if let Some(pid) = &self.selected_project_id {
            if let Some(project) = self.projects.iter().find(|p| p.id == *pid) {
                return Some(project);
            }
        }

        None
    }

    pub fn branch_items_for_selected_project(&self) -> Vec<String> {
        let _profile = crate::shared::render_profile::span("branch_items_for_selected_project");
        let Some(project) = self.selected_composer_project() else {
            return vec!["main".into()];
        };

        if let Some((cached_root, items)) = &self.branch_items_cache {
            if *cached_root == project.root_path {
                return items.clone();
            }
        }

        vec![project.git_branch.clone()]
    }

    pub fn refresh_branch_items_for_project(&mut self, project_id: &ProjectId) {
        let Some(root) = self
            .projects
            .iter()
            .find(|project| &project.id == project_id)
            .map(|project| project.root_path.clone())
        else {
            return;
        };

        let branch_info = git_branch_info(std::path::Path::new(&root));
        self.branch_items_cache = Some((root, branch_info.branches));
        if let Some(project) = self
            .projects
            .iter_mut()
            .find(|project| &project.id == project_id)
        {
            project.git_branch = branch_info.current.clone();
        }
        self.sync_branch_chips_for_project(project_id, &branch_info.current);
    }

    pub fn invalidate_branch_items_cache(&mut self) {
        self.branch_items_cache = None;
    }

    pub fn sync_branch_chips_for_project(&mut self, project_id: &ProjectId, branch: &str) {
        for conv in &mut self.conversations {
            if &conv.project_id != project_id {
                continue;
            }
            for chip in &mut conv.context_chips {
                if chip.kind == ChipKind::Branch {
                    chip.label = branch.to_string();
                }
            }
        }
    }

    pub fn open_image_attachment_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.weak_entity();
        let future = rfd::AsyncFileDialog::new()
            .set_parent(window)
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
            .pick_files();

        cx.spawn(async move |_weak, cx| {
            let Some(handles) = future.await else { return };
            let paths: Vec<PathBuf> = handles
                .into_iter()
                .map(|handle| handle.path().to_path_buf())
                .collect();
            let _ = cx.update(|app| {
                app.defer(move |app| {
                    if let Some(view) = entity.upgrade() {
                        view.update(app, |view, cx| {
                            view.add_image_attachment_paths(paths, cx);
                        });
                    }
                });
            });
        })
        .detach();
    }

    pub fn add_image_attachment_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut added = 0usize;
        for path in paths {
            let Some(mime_type) = image_mime_for_path(&path) else {
                self.composer_error = Some(format!(
                    "Unsupported image type: {}",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("attachment")
                ));
                continue;
            };
            let size_bytes = match std::fs::metadata(&path) {
                Ok(metadata) => metadata.len(),
                Err(err) => {
                    self.composer_error = Some(format!("Could not read image metadata: {err}"));
                    continue;
                }
            };
            let display_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("image")
                .to_string();
            self.pending_image_attachments.push(PendingImageAttachment {
                id: format!("img-{}", uuid::Uuid::new_v4()),
                source: PendingImageSource::File(path),
                mime_type: mime_type.to_string(),
                display_name,
                size_bytes,
            });
            added += 1;
        }
        if added > 0 {
            self.composer_error = None;
        }
        cx.notify();
    }

    pub fn add_clipboard_image_attachment(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(clipboard) = cx.read_from_clipboard() else {
            return false;
        };
        let Some(image) = clipboard.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(image) => Some(image.clone()),
            ClipboardEntry::String(_) => None,
        }) else {
            return false;
        };
        let mime_type = image.format().mime_type();
        if !SUPPORTED_IMAGE_MIME_TYPES.contains(&mime_type) {
            self.composer_error = Some(format!("Unsupported pasted image type: {mime_type}"));
            cx.notify();
            return true;
        }
        self.pending_image_attachments.push(PendingImageAttachment {
            id: format!("img-{}", uuid::Uuid::new_v4()),
            source: PendingImageSource::Clipboard(image.bytes().to_vec()),
            mime_type: mime_type.to_string(),
            display_name: format!("Pasted image {}", self.pending_image_attachments.len() + 1),
            size_bytes: image.bytes().len() as u64,
        });
        self.composer_error = None;
        cx.notify();
        true
    }

    pub fn remove_image_attachment(&mut self, id: &str, cx: &mut Context<Self>) {
        self.pending_image_attachments
            .retain(|attachment| attachment.id != id);
        if self.pending_image_attachments.is_empty() {
            self.composer_error = None;
        }
        cx.notify();
    }

    pub fn on_composer_project_selected(&mut self, project_name: String, cx: &mut Context<Self>) {
        let Some(project) = self.projects.iter().find(|p| p.name == project_name) else {
            return;
        };
        let project_id = project.id.clone();

        self.selected_project_id = Some(project_id.clone());

        if let Some(conv_id) = project.conversations.first().cloned() {
            self.select_conversation(conv_id, cx);
        } else {
            self.create_conversation_in_project(project_id, cx);
        }

        cx.notify();
    }

    pub fn on_composer_branch_selected(&mut self, branch: String, cx: &mut Context<Self>) {
        let Some(project_id) = self
            .selected_composer_project()
            .map(|project| project.id.clone())
        else {
            return;
        };

        let root = self
            .projects
            .iter_mut()
            .find(|p| p.id == project_id)
            .map(|project| {
                project.git_branch = branch.clone();
                project.root_path.clone()
            });

        if let Some(root) = root {
            match &mut self.branch_items_cache {
                Some((cached_root, items)) if *cached_root == root => {
                    if !items.iter().any(|item| item == &branch) {
                        items.insert(0, branch.clone());
                    }
                }
                _ => {
                    self.branch_items_cache = Some((root, vec![branch.clone()]));
                }
            }
        } else if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            project.git_branch = branch.clone();
        }

        self.sync_branch_chips_for_project(&project_id, &branch);

        cx.notify();
    }
}

fn image_mime_for_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}
