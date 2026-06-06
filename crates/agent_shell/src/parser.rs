use std::collections::HashMap;

use crate::{ShellError, ShellResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCommand {
    pub words: Vec<String>,
}

pub fn parse_script(script: &str, env: &[(String, String)]) -> ShellResult<Vec<ParsedCommand>> {
    let env: HashMap<&str, &str> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut commands = Vec::new();
    let mut words = Vec::<String>::new();
    let mut word = String::new();
    let mut chars = script.chars().peekable();
    let mut single = false;
    let mut double = false;
    let mut in_word = false;

    while let Some(ch) = chars.next() {
        if single {
            if ch == '\'' {
                single = false;
            } else {
                word.push(ch);
            }
            in_word = true;
            continue;
        }
        if double {
            match ch {
                '"' => double = false,
                '\\' => {
                    if let Some(next) = chars.next() {
                        word.push(next);
                    }
                }
                '$' => expand_var(&mut word, &mut chars, &env)?,
                '`' => {
                    return Err(ShellError::Unsupported(
                        "backtick command substitution is not supported".into(),
                    ));
                }
                _ => word.push(ch),
            }
            in_word = true;
            continue;
        }

        match ch {
            '\'' => {
                single = true;
                in_word = true;
            }
            '"' => {
                double = true;
                in_word = true;
            }
            '\\' => {
                if let Some(next) = chars.next() {
                    word.push(next);
                    in_word = true;
                }
            }
            '$' => {
                if chars.peek() == Some(&'(') {
                    return Err(ShellError::Unsupported(
                        "command substitution is not supported".into(),
                    ));
                }
                expand_var(&mut word, &mut chars, &env)?;
                in_word = true;
            }
            '`' => {
                return Err(ShellError::Unsupported(
                    "backtick command substitution is not supported".into(),
                ));
            }
            '>' | '<' => {
                return Err(ShellError::Unsupported(
                    "redirection is not supported by fake shell".into(),
                ));
            }
            '|' => {
                return Err(ShellError::Unsupported(
                    "pipes are not supported by fake shell".into(),
                ));
            }
            '#' if !in_word => {
                while let Some(next) = chars.next() {
                    if next == '\n' {
                        finish_command(&mut commands, &mut words, &mut word, &mut in_word);
                        break;
                    }
                }
            }
            ';' | '\n' => {
                finish_command(&mut commands, &mut words, &mut word, &mut in_word);
            }
            c if c.is_whitespace() => {
                finish_word(&mut words, &mut word, &mut in_word);
            }
            _ => {
                word.push(ch);
                in_word = true;
            }
        }
    }
    if single {
        return Err(ShellError::Parse("unterminated single quote".into()));
    }
    if double {
        return Err(ShellError::Parse("unterminated double quote".into()));
    }
    finish_command(&mut commands, &mut words, &mut word, &mut in_word);
    Ok(commands)
}

fn expand_var<I>(
    word: &mut String,
    chars: &mut std::iter::Peekable<I>,
    env: &HashMap<&str, &str>,
) -> ShellResult<()>
where
    I: Iterator<Item = char>,
{
    if chars.peek() == Some(&'{') {
        chars.next();
        let mut name = String::new();
        while let Some(ch) = chars.next() {
            if ch == '}' {
                word.push_str(env.get(name.as_str()).copied().unwrap_or(""));
                return Ok(());
            }
            name.push(ch);
        }
        return Err(ShellError::Parse("unterminated variable expansion".into()));
    }

    let mut name = String::new();
    while let Some(ch) = chars.peek().copied() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    if name.is_empty() {
        word.push('$');
    } else {
        word.push_str(env.get(name.as_str()).copied().unwrap_or(""));
    }
    Ok(())
}

fn finish_word(words: &mut Vec<String>, word: &mut String, in_word: &mut bool) {
    if *in_word {
        words.push(std::mem::take(word));
        *in_word = false;
    }
}

fn finish_command(
    commands: &mut Vec<ParsedCommand>,
    words: &mut Vec<String>,
    word: &mut String,
    in_word: &mut bool,
) {
    finish_word(words, word, in_word);
    if !words.is_empty() {
        commands.push(ParsedCommand {
            words: std::mem::take(words),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_command() {
        let out = parse_script("echo hello world", &[]).unwrap();
        assert_eq!(out[0].words, vec!["echo", "hello", "world"]);
    }

    #[test]
    fn parses_quoted_args() {
        let out = parse_script("echo 'a b' \"c d\"", &[]).unwrap();
        assert_eq!(out[0].words, vec!["echo", "a b", "c d"]);
    }

    #[test]
    fn expands_env_vars() {
        let out = parse_script("echo $NAME ${NAME}", &[("NAME".into(), "vortex".into())]).unwrap();
        assert_eq!(out[0].words, vec!["echo", "vortex", "vortex"]);
    }

    #[test]
    fn parses_comments_and_semicolon() {
        let out = parse_script("echo a # skip\npwd; echo b", &[]).unwrap();
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn rejects_substitution_and_redirects() {
        assert!(parse_script("echo $(pwd)", &[]).is_err());
        assert!(parse_script("echo `pwd`", &[]).is_err());
        assert!(parse_script("echo hi > out", &[]).is_err());
    }
}
