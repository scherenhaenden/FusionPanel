#![forbid(unsafe_code)]

use crate::error::MailError;
use std::collections::HashSet;

pub struct SieveValidator {
    user_email: String,
}

impl SieveValidator {
    pub fn new(user_email: impl Into<String>) -> Self {
        Self {
            user_email: user_email.into(),
        }
    }

    pub fn validate(&self, script: &str) -> Result<(), MailError> {
        let mut chars = script.chars().peekable();
        let mut brace_depth = 0;
        let mut redirect_count = 0;
        let mut redirect_targets = HashSet::new();
        let mut last_command = String::new();

        while let Some(&c) = chars.peek() {
            match c {
                '{' => {
                    brace_depth += 1;
                    last_command.clear();
                    chars.next();
                }
                '}' => {
                    if brace_depth == 0 {
                        return Err(MailError::InvalidInput(
                            "Unbalanced braces: extra '}'".to_string(),
                        ));
                    }
                    brace_depth -= 1;
                    last_command.clear();
                    chars.next();
                }
                ';' => {
                    last_command.clear();
                    chars.next();
                }
                '#' => {
                    // hash comment
                    chars.next();
                    while let Some(&ch) = chars.peek() {
                        if ch == '\n' || ch == '\r' {
                            break;
                        }
                        chars.next();
                    }
                }
                '/' => {
                    chars.next();
                    if let Some(&'*') = chars.peek() {
                        chars.next(); // consume '*'
                        let mut closed = false;
                        while let Some(ch) = chars.next() {
                            if ch == '*' {
                                if let Some(&'/') = chars.peek() {
                                    chars.next();
                                    closed = true;
                                    break;
                                }
                            }
                        }
                        if !closed {
                            return Err(MailError::InvalidInput(
                                "Unterminated bracket comment".to_string(),
                            ));
                        }
                    }
                }
                '"' => {
                    chars.next(); // consume '"'
                    let mut s = String::new();
                    let mut closed = false;
                    while let Some(ch) = chars.next() {
                        if ch == '\\' {
                            if let Some(esc) = chars.next() {
                                s.push(esc);
                            } else {
                                break;
                            }
                        } else if ch == '"' {
                            closed = true;
                            break;
                        } else {
                            s.push(ch);
                        }
                    }
                    if !closed {
                        return Err(MailError::InvalidInput(
                            "Unterminated quoted string".to_string(),
                        ));
                    }

                    if last_command == "redirect" {
                        if s.eq_ignore_ascii_case(&self.user_email) {
                            return Err(MailError::InvalidInput(format!(
                                "Self-redirect detected: {}",
                                s
                            )));
                        }
                        if !redirect_targets.contains(&s) {
                            redirect_targets.insert(s.clone());
                            redirect_count += 1;
                            if redirect_count > 5 {
                                return Err(MailError::InvalidInput(
                                    "Too many redirect targets".to_string(),
                                ));
                            }
                        }
                    }
                }
                c if c.is_alphabetic() => {
                    let mut word = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '_' {
                            word.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    let lower = word.to_lowercase();

                    if lower == "text" {
                        if let Some(&':') = chars.peek() {
                            chars.next(); // consume ':'
                            let mut newline_seen = true;

                            while let Some(ch) = chars.next() {
                                if newline_seen && ch == '.' {
                                    let mut end = false;
                                    if let Some(&next_ch) = chars.peek() {
                                        if next_ch == '\n' || next_ch == '\r' {
                                            end = true;
                                        }
                                    } else {
                                        end = true;
                                    }
                                    if end {
                                        break;
                                    }
                                }
                                newline_seen = ch == '\n' || ch == '\r';
                            }
                            continue;
                        }
                    }

                    match lower.as_str() {
                        "require" | "if" | "elsif" | "else" | "keep" | "discard" | "redirect"
                        | "fileinto" | "stop" => {
                            last_command = lower;
                        }
                        _ => {}
                    }
                }
                _ => {
                    chars.next();
                }
            }
        }

        if brace_depth > 0 {
            return Err(MailError::InvalidInput(
                "Unbalanced braces: missing '}'".to_string(),
            ));
        }

        Ok(())
    }
}
