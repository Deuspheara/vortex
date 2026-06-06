use std::path::{Path, PathBuf};

use agent_protocol::{AndroidActionTrace, AndroidJourney, AndroidObservation};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedRunPaths {
    pub root: PathBuf,
    pub screenshots: PathBuf,
    pub ui_trees: PathBuf,
    pub actions: PathBuf,
    pub journey: PathBuf,
    pub logcat: PathBuf,
}

#[derive(Clone, Debug)]
pub struct JourneyRecorder {
    paths: RecordedRunPaths,
}

impl JourneyRecorder {
    pub fn new(project_root: impl AsRef<Path>, run_id: &str) -> Result<Self, String> {
        let root = project_root
            .as_ref()
            .join(".android-agent")
            .join("runs")
            .join(run_id);
        let paths = RecordedRunPaths {
            screenshots: root.join("screenshots"),
            ui_trees: root.join("ui_trees"),
            actions: root.join("actions.jsonl"),
            journey: root.join("journey.json"),
            logcat: root.join("logcat.txt"),
            root,
        };
        std::fs::create_dir_all(&paths.screenshots).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&paths.ui_trees).map_err(|e| e.to_string())?;
        Ok(Self { paths })
    }

    pub fn paths(&self) -> &RecordedRunPaths {
        &self.paths
    }

    pub fn write_journey(&self, journey: &AndroidJourney) -> Result<(), String> {
        let json = serde_json::to_string_pretty(journey).map_err(|e| e.to_string())?;
        std::fs::write(&self.paths.journey, json).map_err(|e| e.to_string())
    }

    pub fn append_action(&self, action: &AndroidActionTrace) -> Result<(), String> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.paths.actions)
            .map_err(|e| e.to_string())?;
        let line = serde_json::to_string(action).map_err(|e| e.to_string())?;
        writeln!(file, "{line}").map_err(|e| e.to_string())
    }

    pub fn write_observation_refs(&self, observation: &AndroidObservation) -> Result<(), String> {
        let manifest = self
            .paths
            .root
            .join(format!("{}.json", observation.observation_id));
        let json = serde_json::to_string_pretty(observation).map_err(|e| e.to_string())?;
        std::fs::write(manifest, json).map_err(|e| e.to_string())
    }

    pub fn write_logcat(&self, output: &str) -> Result<(), String> {
        std::fs::write(&self.paths.logcat, output).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::{AndroidJourneyStatus, StepResult};

    #[test]
    fn writes_journey_file() {
        let root =
            std::env::temp_dir().join(format!("android-recorder-test-{}", std::process::id()));
        let recorder = JourneyRecorder::new(&root, "run_1").expect("recorder");
        let journey = AndroidJourney {
            id: "j1".into(),
            title: "Smoke".into(),
            goal: "Reach home".into(),
            steps: Vec::new(),
            status: AndroidJourneyStatus::Planned,
        };
        recorder.write_journey(&journey).expect("write");
        assert!(recorder.paths().journey.exists());

        let action = AndroidActionTrace {
            action_id: "a1".into(),
            action: "tap".into(),
            target: Some("Continue".into()),
            reason: None,
            confidence: None,
            before_observation: None,
            after_observation: None,
            settle: None,
            status: "completed".into(),
            duration_ms: Some(1),
        };
        recorder.append_action(&action).expect("action");
        assert!(recorder.paths().actions.exists());

        let _ = std::fs::remove_dir_all(root);
        let _ = StepResult {
            status: AndroidJourneyStatus::Planned,
            summary: None,
        };
    }
}
