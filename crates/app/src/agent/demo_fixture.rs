//! Opt-in fixture replay through the real event store and reducer, never a model run.
use agent_protocol::*;
use agent_store::{
    EventStore, StoredEvent, StoredPatchProposal, StoredProject, StoredRun, StoredSession,
};
use chrono::{TimeZone, Utc};
use std::path::Path;

const BEFORE: &str = "export const notes = ['Launch checklist', 'Design review', 'Release notes'];\n\nexport function visibleNotes(notes) {\n  return notes;\n}\n";
const AFTER: &str = "export const notes = ['Launch checklist', 'Design review', 'Release notes'];\n\nexport function visibleNotes(notes, query = '') {\n  const search = query.trim().toLowerCase();\n  return notes.filter(note => note.toLowerCase().includes(search));\n}\n";
const DIFF: &str = "--- a/notes.mjs\n+++ b/notes.mjs\n@@ -1,5 +1,6 @@\n export const notes = ['Launch checklist', 'Design review', 'Release notes'];\n \n-export function visibleNotes(notes) {\n-  return notes;\n+export function visibleNotes(notes, query = '') {\n+  const search = query.trim().toLowerCase();\n+  return notes.filter(note => note.toLowerCase().includes(search));\n }\n--- a/index.html\n+++ b/index.html\n@@ -1,6 +1,9 @@\n <h1>Atlas Notes</h1>\n+<input id=\"search\" type=\"search\" aria-label=\"Search notes\" placeholder=\"Search notes\">\n <ul id=\"notes\"></ul>\n <script type=\"module\">\n import { notes, visibleNotes } from './notes.mjs';\n-document.querySelector('#notes').replaceChildren(...visibleNotes(notes).map(text => Object.assign(document.createElement('li'), { textContent: text })));\n+const render = () => document.querySelector('#notes').replaceChildren(...visibleNotes(notes, document.querySelector('#search').value).map(text => Object.assign(document.createElement('li'), { textContent: text })));\n+document.querySelector('#search').addEventListener('input', render);\n+render();\n </script>\n";
const HTML: &str = "<h1>Atlas Notes</h1>\n<ul id=\"notes\"></ul>\n<script type=\"module\">\nimport { notes, visibleNotes } from './notes.mjs';\ndocument.querySelector('#notes').replaceChildren(...visibleNotes(notes).map(text => Object.assign(document.createElement('li'), { textContent: text })));\n</script>\n";

pub fn seed(store: &dyn EventStore, data_dir: &Path) -> Result<(), String> {
    let root = data_dir.join("atlas-notes");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    // Do not overwrite a fixture that has been edited or applied by the user.
    for (name, content) in [("notes.mjs", BEFORE), ("index.html", HTML)] {
        let path = root.join(name);
        if !path.exists() {
            std::fs::write(path, content).map_err(|e| e.to_string())?;
        }
    }
    let now = Utc
        .with_ymd_and_hms(2026, 10, 15, 12, 0, 0)
        .single()
        .unwrap();
    let project = ProjectId::new("demo-atlas");
    let session = SessionId::new("demo-search");
    let run = RunId::new("demo-replay");
    store.upsert_project(&StoredProject {
        id: project.clone(),
        root_path: root.display().to_string(),
        name: "Atlas Notes · Demo".into(),
        trusted: false,
        created_at: now,
        updated_at: now,
    })?;
    if store.get_session(&session)?.is_some() {
        return Ok(());
    }
    store.create_session(&StoredSession {
        id: session.clone(),
        project_id: project,
        title: "Add search to the notes list · fixture".into(),
        created_at: now,
        updated_at: now,
    })?;
    store.create_run(&StoredRun {
        id: run.clone(),
        session_id: session.clone(),
        parent_run_id: None,
        depth: 0,
        model: ModelId::new("Offline fixture replay"),
        mode: AgentMode::ApplyWithApproval,
        status: RunStatus::Completed,
        started_at: now,
        finished_at: Some(now),
    })?;
    let call = ToolCallId::new("demo-inspect");
    let patch = PatchId::new("demo-search-patch");
    let events = vec![
        AgentEvent::RunStarted {run_id:run.clone(),session_id:session,model:ModelId::new("Offline fixture replay"),mode:AgentMode::ApplyWithApproval,depth:0,parent_run_id:None},
        AgentEvent::AssistantTextDelta {run_id:run.clone(),text:"## Atlas Notes — add search\n\nDemo data / fixture replay. Add a search field to the notes list, keep filtering case-insensitive, and preserve all notes when the query is empty. No model or agent tools were executed for this replay.\n".into()},
        AgentEvent::PlanUpdated {run_id:run.clone(),markdown:"1. Inspect the notes list and its renderer.\n2. Add a labelled search field and a pure filter function.\n3. Review the proposed patch before applying it.\n\nFixture scenario; test execution is not claimed.".into(),created_at:now.to_rfc3339()},
        AgentEvent::ToolCallStarted {run_id:run.clone(),call_id:call.clone(),name:"read_file".into(),args_preview:"notes.mjs · fixture inspection".into(),dedupe_key:None,risk:RiskLevel::Low},
        AgentEvent::ToolOutputDelta {run_id:Some(run.clone()),call_id:call.clone(),stream:OutputStreamKind::Stdout,chunk:format!("Fixture output — baseline contents of atlas-notes/notes.mjs\n{BEFORE}")},
        AgentEvent::ToolCallFinished {run_id:Some(run.clone()),call_id:call,status:ToolStatus::Completed,summary:"Fixture inspection: notes.mjs and index.html".into(),body:Some("The list currently returns every note. The proposed change adds a search input and filters the existing data.".into())},
        AgentEvent::PatchProposed {patch_id:patch.clone(),files:vec!["notes.mjs".into(),"index.html".into()],unified_diff:DIFF.into(),risk:RiskLevel::Low},
        AgentEvent::AssistantTextDelta {run_id:run.clone(),text:"\n### Ready for review\nThe proposed patch adds a labelled search field and preserves the original notes. Review the changes before applying. This is a replay fixture: no inference, tool timing, token cost or passing test result is claimed.\n".into()},
        AgentEvent::RunFinished {run_id:run.clone(),status:RunStatus::Completed},
    ];
    store.record_patch_proposal(&StoredPatchProposal {
        id: patch,
        run_id: run.clone(),
        base_git_sha: None,
        diff: DIFF.into(),
        status: "proposed".into(),
        created_at: now,
    })?;
    for (i, event) in events.into_iter().enumerate() {
        store.append_event(&StoredEvent {
            id: EventId::new(format!("demo-event-{i}")),
            run_id: run.clone(),
            sequence: i as i64,
            event,
            created_at: now,
        })?;
    }
    // Candidate file is separate from the baseline project; used only by focused tests.
    std::fs::write(data_dir.join("candidate-notes.mjs"), AFTER).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_is_isolated_idempotent_and_replayable() {
        let root =
            std::env::temp_dir().join(format!("vortex-fixture-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let store = agent_store::SqliteEventStore::open(root.join("vortex.db")).unwrap();
        seed(&store, &root).unwrap();
        seed(&store, &root).unwrap();
        let events = store
            .load_session_events(&SessionId::new("demo-search"))
            .unwrap();
        assert_eq!(events.len(), 9);
        assert!(
            events
                .iter()
                .any(|e| matches!(e.event, AgentEvent::PatchProposed { .. }))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e.event, AgentEvent::UsageUpdated { .. }))
        );
        assert_eq!(
            std::fs::read_to_string(root.join("atlas-notes/notes.mjs")).unwrap(),
            BEFORE
        );
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
