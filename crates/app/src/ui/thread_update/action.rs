use crate::features::shell::state::{ConversationId, ThreadItem};

#[derive(Clone, Debug)]
pub enum ThreadAction {
    SetConversation {
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
    },
    Sync {
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
        immediate: bool,
        /// Agent run still in flight — coalesce thread sync like streaming.
        run_active: bool,
    },
    PushItem(ThreadItem),
    UpdateItem(ThreadItem),
    RefreshItem(ThreadItem),
    SetApprovalActive(bool),
}
