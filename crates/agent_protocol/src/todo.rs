use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl Default for TodoStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// A single persistent plan/todo entry surfaced by the `todo_write` tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub status: TodoStatus,
}

/// Merge `incoming` todos into `existing`. When `merge` is true, entries are updated/added by id
/// (preserving order, appending new ids); otherwise the list is replaced wholesale.
pub fn merge_todos(existing: &[TodoItem], incoming: Vec<TodoItem>, merge: bool) -> Vec<TodoItem> {
    if !merge {
        return incoming;
    }
    let mut result = existing.to_vec();
    for item in incoming {
        if let Some(slot) = result.iter_mut().find(|e| e.id == item.id) {
            slot.content = item.content;
            slot.status = item.status;
        } else {
            result.push(item);
        }
    }
    result
}
