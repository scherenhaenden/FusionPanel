#![forbid(unsafe_code)]

/// HTTP is the default worker. The old WebSocket flow remains available only
/// when the operator selects `legacy-ws` explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Http,
    LegacyWebSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeError {
    UnknownMode,
    LegacyModeMustBeExplicit,
}

pub fn select_mode(
    configured_mode: Option<&str>,
    legacy_environment_present: bool,
) -> Result<AgentMode, ModeError> {
    match configured_mode {
        Some("http") => Ok(AgentMode::Http),
        Some("legacy-ws") => Ok(AgentMode::LegacyWebSocket),
        Some(_) => Err(ModeError::UnknownMode),
        None if legacy_environment_present => Err(ModeError::LegacyModeMustBeExplicit),
        None => Ok(AgentMode::Http),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_is_default_and_legacy_requires_explicit_selection() {
        assert_eq!(select_mode(None, false), Ok(AgentMode::Http));
        assert_eq!(
            select_mode(Some("legacy-ws"), true),
            Ok(AgentMode::LegacyWebSocket)
        );
        assert_eq!(
            select_mode(None, true),
            Err(ModeError::LegacyModeMustBeExplicit)
        );
        assert_eq!(
            select_mode(Some("other"), false),
            Err(ModeError::UnknownMode)
        );
    }
}
