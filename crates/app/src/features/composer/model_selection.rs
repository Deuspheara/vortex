//! Split the advertised Codex choices into model and reasoning controls.
//! Selection values remain the exact catalog values used by command routing.
#[derive(Debug)]
pub struct CodexModelSelection {
    pub model: String,
    pub effort: Option<String>,
    pub models: Vec<(String, String)>,
    pub efforts: Vec<(String, String)>,
}

fn parts(value: &str) -> (&str, Option<&str>) {
    value
        .rsplit_once(" · ")
        .map_or((value, None), |(model, effort)| (model, Some(effort)))
}

impl CodexModelSelection {
    pub fn new(selected: &str, catalog: &[String]) -> Self {
        let (model, effort) = parts(selected);
        let mut choices = catalog.to_vec();
        if !selected.is_empty() && !choices.iter().any(|choice| choice == selected) {
            choices.insert(0, selected.to_string());
        }
        let mut models: Vec<(String, String)> = Vec::new();
        let mut efforts = Vec::new();
        for value in &choices {
            let (name, reasoning) = parts(value);
            if let Some(existing) = models.iter_mut().find(|(label, _)| label == name) {
                if reasoning == effort {
                    existing.1 = value.clone();
                }
            } else {
                models.push((name.to_string(), value.clone()));
            }
            if name == model {
                if let Some(reasoning) = reasoning {
                    efforts.push((reasoning.to_string(), value.clone()));
                }
            }
        }
        Self {
            model: model.to_string(),
            effort: effort.map(str::to_string),
            models,
            efforts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_models_keeps_only_advertised_efforts() {
        let catalog =
            ["a · low", "a · high", "b · low", "b · high", "c · medium"].map(str::to_string);
        let selection = CodexModelSelection::new("a · high", &catalog);
        assert_eq!(
            selection.models,
            [
                ("a".into(), "a · high".into()),
                ("b".into(), "b · high".into()),
                ("c".into(), "c · medium".into())
            ]
        );
        assert_eq!(
            selection.efforts,
            [
                ("low".into(), "a · low".into()),
                ("high".into(), "a · high".into())
            ]
        );
    }
}
