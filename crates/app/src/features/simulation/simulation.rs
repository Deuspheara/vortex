//! Simulated agent run — thinking, tools, streaming reply for demo / send.

use std::time::Duration;

use gpui::{AsyncApp, Entity, Timer, WeakEntity};

use crate::features::chat::thread_view::ThreadView;
use crate::features::shell::state::{
    AgentStatus, ApprovalRisk, ConversationId, DeltaBuffer, DiffFileSummary, ThreadItem,
};
use crate::ui::agent_window::AgentWindow;

#[allow(dead_code)]
const ASSISTANT_REPLY: &str = "I've refactored the LoginForm component to use the new `useAuth` hook.\n\n## Changes summary\n\n| File | Status |\n|------|--------|\n| login.tsx | Updated |\n| useAuth.ts | New |\n| types.ts | Cleaned up |\n\n```typescript\nexport function useAuth() {\n  const [loading, setLoading] = useState(false);\n  const [error, setError] = useState<string | null>(null);\n  return { login, loading, error };\n}\n```\n\n### Architecture\n\n- LoginForm calls `useAuth` for sign-in state\n- Session store handles token refresh\n- Auth API backed by Postgres with Redis cache\n";

/// Start a scripted agent turn for the given conversation.
#[allow(dead_code)]
pub fn start_agent_simulation(
    agent: Entity<AgentWindow>,
    thread: Entity<ThreadView>,
    conversation_id: ConversationId,
    cx: &mut gpui::App,
) {
    let agent_weak = agent.downgrade();
    let thread_weak = thread.downgrade();

    cx.spawn(async move |cx| {
        run_simulation(agent_weak, thread_weak, conversation_id, cx).await;
    })
    .detach();
}

#[allow(dead_code)]
async fn run_simulation(
    agent: WeakEntity<AgentWindow>,
    thread: WeakEntity<ThreadView>,
    conversation_id: ConversationId,
    cx: &mut AsyncApp,
) {
    let sleep = |ms: u64| async move {
        Timer::after(Duration::from_millis(ms)).await;
    };

    if !mark_simulation_started(&agent, &conversation_id, cx) {
        return;
    }

    let turn = turn_number(&agent, &conversation_id, cx);

    set_agent_status(&agent, AgentStatus::Thinking, cx);
    push_item(
        &agent,
        &thread,
        &conversation_id,
        ThreadItem::ReasoningStep {
            id: sim_id(&conversation_id, turn, "reason"),
            title: "Thinking".into(),
            summary: "inspecting auth module".into(),
            expanded: false,
            status: AgentStatus::Thinking,
            depth: 0,
            parent_call_id: None,
        },
        cx,
    );
    sleep(1400).await;

    complete_reasoning(&agent, &thread, &conversation_id, turn, cx);
    sleep(300).await;

    push_item(
        &agent,
        &thread,
        &conversation_id,
        ThreadItem::ToolCall {
            id: sim_id(&conversation_id, turn, "tool-read"),
            tool_name: "ReadFile".into(),
            command: Some("src/auth/login.tsx".into()),
            output: None,
            expanded: false,
            status: AgentStatus::RunningTool,
            depth: 0,
            parent_call_id: None,
        },
        cx,
    );
    set_agent_status(&agent, AgentStatus::RunningTool, cx);
    sleep(900).await;

    finish_tool(
        &agent,
        &thread,
        &conversation_id,
        turn,
        "tool-read",
        "export function LoginForm() { /* … */ }",
        cx,
    );
    sleep(400).await;

    push_item(
        &agent,
        &thread,
        &conversation_id,
        ThreadItem::ToolCall {
            id: sim_id(&conversation_id, turn, "tool-edit"),
            tool_name: "EditFile".into(),
            command: Some("src/auth/login.tsx".into()),
            output: None,
            expanded: false,
            status: AgentStatus::RunningTool,
            depth: 0,
            parent_call_id: None,
        },
        cx,
    );
    sleep(800).await;

    finish_tool(
        &agent,
        &thread,
        &conversation_id,
        turn,
        "tool-edit",
        "Applied 3 hunks",
        cx,
    );
    sleep(350).await;

    push_item(
        &agent,
        &thread,
        &conversation_id,
        ThreadItem::ToolCall {
            id: sim_id(&conversation_id, turn, "tool-cmd"),
            tool_name: "RunCommand".into(),
            command: Some("cargo check".into()),
            output: None,
            expanded: false,
            status: AgentStatus::RunningTool,
            depth: 0,
            parent_call_id: None,
        },
        cx,
    );
    sleep(1100).await;

    finish_tool(
        &agent,
        &thread,
        &conversation_id,
        turn,
        "tool-cmd",
        "Finished `cargo check` (0 warnings)",
        cx,
    );
    sleep(300).await;

    push_item(
        &agent,
        &thread,
        &conversation_id,
        ThreadItem::DiffSummary {
            id: sim_id(&conversation_id, turn, "diff"),
            files_changed: 3,
            additions: 10,
            deletions: 1,
            files: vec![
                DiffFileSummary {
                    path: "crates/app/src/main.rs".into(),
                    added: 7,
                    removed: 1,
                },
                DiffFileSummary {
                    path: "crates/app/src/ui/agent_window.rs".into(),
                    added: 2,
                    removed: 0,
                },
                DiffFileSummary {
                    path: "crates/app/Cargo.toml".into(),
                    added: 1,
                    removed: 0,
                },
            ],
            expanded: false,
            depth: 0,
            parent_call_id: None,
        },
        cx,
    );
    sleep(500).await;

    let assistant_id = sim_id(&conversation_id, turn, "assistant");
    push_item(
        &agent,
        &thread,
        &conversation_id,
        ThreadItem::AssistantMessage {
            id: assistant_id.clone(),
            markdown: String::new(),
            streaming: true,
            depth: 0,
            parent_call_id: None,
        },
        cx,
    );
    set_agent_status(&agent, AgentStatus::Thinking, cx);

    let mut buffer = DeltaBuffer::default();
    let mut pos = 0;
    let chars: Vec<char> = ASSISTANT_REPLY.chars().collect();

    while pos < chars.len() {
        let chunk_end = (pos + 4).min(chars.len());
        let delta: String = chars[pos..chunk_end].iter().collect();
        pos = chunk_end;
        buffer.push(&delta);

        if buffer.should_flush() || pos >= chars.len() {
            let delta = buffer.take();
            append_assistant_delta(&agent, &thread, &conversation_id, &assistant_id, &delta, cx)
                .await;
        }

        Timer::after(Duration::from_millis(24)).await;
    }

    finish_assistant(&agent, &thread, &conversation_id, &assistant_id, cx);
    sleep(400).await;

    push_item(
        &agent,
        &thread,
        &conversation_id,
        ThreadItem::ApprovalRequest {
            id: sim_id(&conversation_id, turn, "approval"),
            title: "Apply changes to 3 files?".into(),
            risk: ApprovalRisk::Medium,
            resolved: false,
        },
        cx,
    );

    set_agent_status(&agent, AgentStatus::WaitingApproval, cx);
    clear_simulation_flag(&agent, &conversation_id, cx);
}

#[allow(dead_code)]
fn turn_number(
    agent: &WeakEntity<AgentWindow>,
    conversation_id: &ConversationId,
    cx: &mut AsyncApp,
) -> usize {
    agent
        .read_with(cx, |view, _| {
            view.thread_items_for(conversation_id)
                .iter()
                .filter(|item| matches!(item, ThreadItem::UserMessage { .. }))
                .count()
        })
        .ok()
        .unwrap_or(1)
}

#[allow(dead_code)]
fn sim_id(conversation_id: &ConversationId, turn: usize, suffix: &str) -> String {
    format!("sim-{}-t{turn}-{suffix}", conversation_id.0)
}

#[allow(dead_code)]
fn mark_simulation_started(
    agent: &WeakEntity<AgentWindow>,
    conversation_id: &ConversationId,
    cx: &mut AsyncApp,
) -> bool {
    agent
        .update(cx, |view, cx| {
            view.try_start_simulation(conversation_id.clone(), cx)
        })
        .ok()
        .unwrap_or(false)
}

#[allow(dead_code)]
fn clear_simulation_flag(
    agent: &WeakEntity<AgentWindow>,
    conversation_id: &ConversationId,
    cx: &mut AsyncApp,
) {
    let _ = agent.update(cx, |view, cx| {
        view.finish_simulation(conversation_id.clone(), cx)
    });
}

#[allow(dead_code)]
fn set_agent_status(agent: &WeakEntity<AgentWindow>, status: AgentStatus, cx: &mut AsyncApp) {
    let _ = agent.update(cx, |view, cx| view.set_agent_status(status, cx));
}

#[allow(dead_code)]
fn push_item(
    agent: &WeakEntity<AgentWindow>,
    thread: &WeakEntity<ThreadView>,
    conversation_id: &ConversationId,
    item: ThreadItem,
    cx: &mut AsyncApp,
) {
    let cid = conversation_id.clone();
    let _ = agent.update(cx, |view, cx| view.push_thread_item(cid, item, cx));
    sync_thread(agent, thread, conversation_id, cx);
}

#[allow(dead_code)]
fn sync_thread(
    agent: &WeakEntity<AgentWindow>,
    thread: &WeakEntity<ThreadView>,
    conversation_id: &ConversationId,
    cx: &mut AsyncApp,
) {
    let Some(items) = agent
        .read_with(cx, |view, _| view.thread_items_for(conversation_id))
        .ok()
    else {
        return;
    };
    let cid = conversation_id.clone();
    let _ = thread.update(cx, |view, cx| view.sync(cid, items, false, cx));
}

#[allow(dead_code)]
fn complete_reasoning(
    agent: &WeakEntity<AgentWindow>,
    thread: &WeakEntity<ThreadView>,
    conversation_id: &ConversationId,
    turn: usize,
    cx: &mut AsyncApp,
) {
    let id = sim_id(conversation_id, turn, "reason");
    let cid = conversation_id.clone();
    let _ = agent.update(cx, |view, cx| {
        view.update_thread_item(
            cid,
            &id,
            |item| {
                if let ThreadItem::ReasoningStep {
                    status, summary, ..
                } = item
                {
                    *status = AgentStatus::Completed;
                    *summary = "inspected 4 files · planned patch".into();
                }
            },
            cx,
        );
    });
    sync_thread(agent, thread, conversation_id, cx);
}

#[allow(dead_code)]
fn finish_tool(
    agent: &WeakEntity<AgentWindow>,
    thread: &WeakEntity<ThreadView>,
    conversation_id: &ConversationId,
    turn: usize,
    tool_suffix: &str,
    output: &str,
    cx: &mut AsyncApp,
) {
    let id = sim_id(conversation_id, turn, tool_suffix);
    let output = output.to_string();
    let cid = conversation_id.clone();
    let _ = agent.update(cx, |view, cx| {
        view.update_thread_item(
            cid,
            &id,
            |item| {
                if let ThreadItem::ToolCall {
                    status,
                    output: out,
                    ..
                } = item
                {
                    *status = AgentStatus::Completed;
                    *out = Some(output);
                }
            },
            cx,
        );
    });
    sync_thread(agent, thread, conversation_id, cx);
}

#[allow(dead_code)]
async fn append_assistant_delta(
    agent: &WeakEntity<AgentWindow>,
    thread: &WeakEntity<ThreadView>,
    conversation_id: &ConversationId,
    assistant_id: &str,
    delta: &str,
    cx: &mut AsyncApp,
) {
    let delta = delta.to_string();
    let assistant_id = assistant_id.to_string();
    let cid = conversation_id.clone();
    let _ = agent.update(cx, |view, cx| {
        view.update_thread_item(
            cid,
            &assistant_id,
            |item| {
                if let ThreadItem::AssistantMessage { markdown, .. } = item {
                    markdown.push_str(&delta);
                }
            },
            cx,
        );
    });
    sync_thread(agent, thread, conversation_id, cx);
}

#[allow(dead_code)]
fn finish_assistant(
    agent: &WeakEntity<AgentWindow>,
    thread: &WeakEntity<ThreadView>,
    conversation_id: &ConversationId,
    assistant_id: &str,
    cx: &mut AsyncApp,
) {
    let assistant_id = assistant_id.to_string();
    let cid = conversation_id.clone();
    let _ = agent.update(cx, |view, cx| {
        view.update_thread_item(
            cid,
            &assistant_id,
            |item| {
                if let ThreadItem::AssistantMessage { streaming, .. } = item {
                    *streaming = false;
                }
            },
            cx,
        );
        view.set_agent_status(AgentStatus::Completed, cx);
    });
    sync_thread(agent, thread, conversation_id, cx);
}
