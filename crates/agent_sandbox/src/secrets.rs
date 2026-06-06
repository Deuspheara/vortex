use regex::Regex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretAction {
    Safe,
    Suspicious,
    Critical,
}

pub struct SecretScanner {
    patterns: Vec<(Regex, SecretAction)>,
}

impl Default for SecretScanner {
    fn default() -> Self {
        let patterns = vec![
            (r"OPENAI_API_KEY\s*=", SecretAction::Critical),
            (r"ANTHROPIC_API_KEY\s*=", SecretAction::Critical),
            (r"GITHUB_TOKEN\s*=", SecretAction::Critical),
            (r"sk-[A-Za-z0-9]{20,}", SecretAction::Critical),
            (
                r"-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----",
                SecretAction::Critical,
            ),
            (r"AKIA[0-9A-Z]{16}", SecretAction::Suspicious),
        ]
        .into_iter()
        .map(|(pat, action)| (Regex::new(pat).unwrap(), action))
        .collect();
        Self { patterns }
    }
}

impl SecretScanner {
    pub fn scan(&self, text: &str) -> SecretAction {
        let mut worst = SecretAction::Safe;
        for (pattern, action) in &self.patterns {
            if pattern.is_match(text) {
                worst = match (worst, action) {
                    (SecretAction::Critical, _) | (_, SecretAction::Critical) => {
                        SecretAction::Critical
                    }
                    (SecretAction::Suspicious, _) | (_, SecretAction::Suspicious) => {
                        SecretAction::Suspicious
                    }
                    _ => SecretAction::Safe,
                };
            }
        }
        worst
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (pattern, _) in &self.patterns {
            out = pattern.replace_all(&out, "[REDACTED]").into_owned();
        }
        out
    }
}
