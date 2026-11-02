use crate::config::validator::is_valid_domain;

#[derive(Debug, PartialEq)]
pub enum PostfixError {
    InvalidDomain(String),
    InvalidUser(String),
}

pub fn render_virtual_mailbox_domains(domains: &[&str]) -> Result<String, PostfixError> {
    let mut config = String::new();
    for domain in domains {
        if !is_valid_domain(domain) {
            return Err(PostfixError::InvalidDomain(domain.to_string()));
        }
        config.push_str(&format!("{} OK\n", domain));
    }
    Ok(config)
}

pub fn render_virtual_mailbox_maps(mailboxes: &[(&str, &str)]) -> Result<String, PostfixError> {
    // mailboxes is slice of (username, domain)
    use crate::config::validator::is_valid_username;
    let mut config = String::new();

    for (user, domain) in mailboxes {
        if !is_valid_username(user) {
            return Err(PostfixError::InvalidUser(user.to_string()));
        }
        if !is_valid_domain(domain) {
            return Err(PostfixError::InvalidDomain(domain.to_string()));
        }
        // Assuming mailbox format is user@domain domain/user/
        config.push_str(&format!("{}@{} {}/{}/\n", user, domain, domain, user));
    }
    Ok(config)
}
