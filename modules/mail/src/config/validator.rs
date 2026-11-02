pub fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }

    // Check for safe characters, rejecting anything outside a-zA-Z0-9.-
    // Explicitly reject %, shell metacharacters, and unicode.
    for c in domain.chars() {
        if !c.is_ascii_alphanumeric() && c != '-' && c != '.' {
            return false;
        }
    }

    if domain.starts_with('-')
        || domain.ends_with('-')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return false;
    }

    true
}

pub fn is_valid_username(username: &str) -> bool {
    if username.is_empty() || username.len() > 64 {
        return false;
    }

    // Check for safe characters: a-zA-Z0-9.-_
    // Reject %, shell metacharacters, unicode.
    for c in username.chars() {
        if !c.is_ascii_alphanumeric() && c != '-' && c != '.' && c != '_' {
            return false;
        }
    }

    if username.starts_with('-')
        || username.ends_with('-')
        || username.starts_with('.')
        || username.ends_with('.')
    {
        return false;
    }

    true
}
