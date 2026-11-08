#![forbid(unsafe_code)]

#[derive(Debug, PartialEq, Eq)]
pub enum DictCommand {
    Lookup { path: String, username: String },
    Set { path: String, username: String, value: String },
    Inc { path: String, username: String, diff: i64 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum DictResponse {
    Ok(Option<String>),
    Fail,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DictProtocolError {
    InvalidFormat,
    UnknownCommand,
    InvalidDiff,
    InjectionAttempt,
}

impl std::fmt::Display for DictProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DictProtocolError::InvalidFormat => write!(f, "Invalid command format"),
            DictProtocolError::UnknownCommand => write!(f, "Unknown command"),
            DictProtocolError::InvalidDiff => write!(f, "Invalid diff value for Inc command"),
            DictProtocolError::InjectionAttempt => write!(f, "Injection attempt detected"),
        }
    }
}

impl std::error::Error for DictProtocolError {}

fn check_injection(s: &str) -> Result<(), DictProtocolError> {
    for c in s.chars() {
        if c == '\n' || c == '\r' || c == '\0' || c == '\t' {
            return Err(DictProtocolError::InjectionAttempt);
        }
    }
    Ok(())
}

pub struct DovecotQuotaDictProtocol;

impl DovecotQuotaDictProtocol {
    pub fn parse(line: &str) -> Result<DictCommand, DictProtocolError> {
        if !line.ends_with('\n') {
            return Err(DictProtocolError::InvalidFormat);
        }

        let line = &line[..line.len() - 1];

        if line.is_empty() {
            return Err(DictProtocolError::InvalidFormat);
        }

        let cmd_char = line.chars().next().unwrap();
        let payload = &line[cmd_char.len_utf8()..];

        // Ensure no hidden newlines, carriage returns, or null bytes in the entire payload
        for c in payload.chars() {
            if c == '\n' || c == '\r' || c == '\0' {
                return Err(DictProtocolError::InjectionAttempt);
            }
        }

        let parts: Vec<&str> = payload.split('\t').collect();

        match cmd_char {
            'L' => {
                if parts.len() != 2 {
                    return Err(DictProtocolError::InjectionAttempt);
                }
                let path = parts[0];
                let username = parts[1];
                check_injection(path)?;
                check_injection(username)?;
                Ok(DictCommand::Lookup {
                    path: path.to_string(),
                    username: username.to_string(),
                })
            }
            'S' => {
                if parts.len() != 3 {
                    return Err(DictProtocolError::InjectionAttempt);
                }
                let path = parts[0];
                let username = parts[1];
                let value = parts[2];
                check_injection(path)?;
                check_injection(username)?;
                check_injection(value)?;
                Ok(DictCommand::Set {
                    path: path.to_string(),
                    username: username.to_string(),
                    value: value.to_string(),
                })
            }
            'I' => {
                if parts.len() != 3 {
                    return Err(DictProtocolError::InjectionAttempt);
                }
                let path = parts[0];
                let username = parts[1];
                let diff_str = parts[2];
                check_injection(path)?;
                check_injection(username)?;
                check_injection(diff_str)?;

                let diff = diff_str.parse::<i64>().map_err(|_| DictProtocolError::InvalidDiff)?;

                Ok(DictCommand::Inc {
                    path: path.to_string(),
                    username: username.to_string(),
                    diff,
                })
            }
            _ => Err(DictProtocolError::UnknownCommand),
        }
    }

    pub fn format(response: &DictResponse) -> String {
        match response {
            DictResponse::Ok(Some(value)) => format!("O{}\n", value),
            DictResponse::Ok(None) => "N\n".to_string(),
            DictResponse::Fail => "F\n".to_string(),
        }
    }
}
