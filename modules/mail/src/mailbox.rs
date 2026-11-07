#![forbid(unsafe_code)]

use mail::domain::{EmailAddress, MailDomain, MailQuota};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailboxPasswordHashError {
    InvalidFormat,
}

impl std::fmt::Display for MailboxPasswordHashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid password hash format")
    }
}

impl std::error::Error for MailboxPasswordHashError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailboxPasswordHash(String);

impl MailboxPasswordHash {
    pub fn new(hash: &str) -> Result<Self, MailboxPasswordHashError> {
        if hash.starts_with("$argon2id$") || hash.starts_with("$6$") {
            Ok(Self(hash.to_string()))
        } else {
            Err(MailboxPasswordHashError::InvalidFormat)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualMailboxAccount {
    pub email: EmailAddress,
    pub password_hash: MailboxPasswordHash,
    pub quota: MailQuota,
}

impl VirtualMailboxAccount {
    pub fn new(email: EmailAddress, password_hash: MailboxPasswordHash, quota: MailQuota) -> Self {
        Self {
            email,
            password_hash,
            quota,
        }
    }
}

pub struct VirtualMailboxManager {
    domains: Vec<MailDomain>,
    mailboxes: Vec<VirtualMailboxAccount>,
}

impl VirtualMailboxManager {
    pub fn new() -> Self {
        Self {
            domains: Vec::new(),
            mailboxes: Vec::new(),
        }
    }

    pub fn add_domain(&mut self, domain: MailDomain) {
        if !self.domains.iter().any(|d| d.as_str() == domain.as_str()) {
            self.domains.push(domain);
        }
    }

    pub fn add_mailbox(&mut self, account: VirtualMailboxAccount) -> Result<(), &'static str> {
        let full_addr = account.email.as_string();
        let (_, domain_part) = full_addr.split_once('@').unwrap();

        if !self.domains.iter().any(|d| d.as_str() == domain_part) {
            return Err("Domain does not exist");
        }

        self.mailboxes.push(account);
        Ok(())
    }

    pub fn mailboxes(&self) -> &[VirtualMailboxAccount] {
        &self.mailboxes
    }

    pub fn domains(&self) -> &[MailDomain] {
        &self.domains
    }
}
