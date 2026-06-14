#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    Start,
    Models,
    Safety,
    Appearance,
    Transcript,
    Project,
    About,
}

impl SettingsSection {
    pub const ALL: [Self; 7] = [
        Self::Start,
        Self::Models,
        Self::Safety,
        Self::Appearance,
        Self::Transcript,
        Self::Project,
        Self::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Models => "Models",
            Self::Safety => "Safety",
            Self::Appearance => "Appearance",
            Self::Transcript => "Transcript",
            Self::Project => "Project",
            Self::About => "About",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Start => "Project readiness and setup checks.",
            Self::Models => "Provider and model defaults for new agent runs.",
            Self::Safety => "Approval behavior and command guardrails.",
            Self::Appearance => "Theme, contrast, and light or dark mode.",
            Self::Transcript => "How much detail appears in the thread.",
            Self::Project => "Workspace folder and local project actions.",
            Self::About => "App and community theme details.",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SettingsSectionSelection {
    selected: SettingsSection,
}

impl SettingsSectionSelection {
    pub fn selected(self) -> SettingsSection {
        self.selected
    }

    pub fn select(&mut self, section: SettingsSection) {
        self.selected = section;
    }
}

#[cfg(test)]
mod tests {
    use super::{SettingsSection, SettingsSectionSelection};

    #[test]
    fn settings_section_defaults_to_start() {
        assert_eq!(SettingsSection::default(), SettingsSection::Start);
    }

    #[test]
    fn settings_section_selection_updates_selected_section() {
        let mut selection = SettingsSectionSelection::default();
        selection.select(SettingsSection::Models);

        assert_eq!(selection.selected(), SettingsSection::Models);
    }
}
