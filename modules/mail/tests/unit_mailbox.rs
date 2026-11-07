#![forbid(unsafe_code)]

use mail::domain::{
    AccountState, EmailAddress, EmailAddressError, MailDomain, MailDomainError, MailQuota,
    MailQuotaError, MailboxAccount, PasswordHash, PasswordHashError,
};
use uuid::Uuid;

#[test]
fn test_mail_domain_valid() {
    let domain = MailDomain::new("example.com").unwrap();
    assert_eq!(domain.as_str(), "example.com");

    let domain = MailDomain::new("sub.example.co.uk").unwrap();
    assert_eq!(domain.as_str(), "sub.example.co.uk");
}

#[test]
fn test_mail_domain_invalid() {
    // Unicode/IDNA
    assert_eq!(
        MailDomain::new("exämple.com").unwrap_err(),
        MailDomainError::InvalidCharacters
    );

    // Empty
    assert_eq!(MailDomain::new("").unwrap_err(), MailDomainError::Empty);

    // Traversal
    assert_eq!(
        MailDomain::new("example.com/..").unwrap_err(),
        MailDomainError::InvalidCharacters
    );
    assert_eq!(
        MailDomain::new("../example.com").unwrap_err(),
        MailDomainError::InvalidCharacters
    );

    // Invalid DNS
    assert_eq!(
        MailDomain::new("-example.com").unwrap_err(),
        MailDomainError::InvalidCharacters
    );
}

#[test]
fn test_email_address_valid() {
    let email = EmailAddress::new("test.user+tag@example.com").unwrap();
    assert_eq!(email.as_string(), "test.user+tag@example.com");
}

#[test]
fn test_email_address_invalid() {
    assert_eq!(
        EmailAddress::new("invalid_email").unwrap_err(),
        EmailAddressError::InvalidFormat
    );
    assert_eq!(
        EmailAddress::new("@example.com").unwrap_err(),
        EmailAddressError::InvalidFormat
    );
    assert_eq!(
        EmailAddress::new("test@").unwrap_err(),
        EmailAddressError::DomainError(MailDomainError::Empty)
    );
    assert_eq!(
        EmailAddress::new("test@exämple.com").unwrap_err(),
        EmailAddressError::DomainError(MailDomainError::InvalidCharacters)
    );
}

#[test]
fn test_mail_quota() {
    let quota = MailQuota::new(1024).unwrap();
    assert_eq!(quota.bytes(), 1024);

    assert_eq!(
        MailQuota::new(0).unwrap_err(),
        MailQuotaError::ZeroOrNegative
    );
}

#[test]
fn test_password_hash() {
    let valid_hash = "$argon2id$v=19$m=65536,t=3,p=4$somehash";
    let hash = PasswordHash::new(valid_hash).unwrap();
    assert_eq!(hash.as_str(), valid_hash);

    assert_eq!(
        PasswordHash::new("invalid_hash").unwrap_err(),
        PasswordHashError::InvalidFormat
    );
    assert_eq!(
        PasswordHash::new("$bcrypt$v=19...").unwrap_err(),
        PasswordHashError::InvalidFormat
    );
}

#[test]
fn test_mailbox_account_creation() {
    let id = Uuid::new_v4();
    let email = EmailAddress::new("user@example.com").unwrap();
    let hash = PasswordHash::new("$argon2id$somehash").unwrap();
    let quota = MailQuota::new(1024 * 1024).unwrap(); // 1MB

    let account = MailboxAccount::new(id, email.clone(), hash.clone(), quota.clone());

    assert_eq!(account.id, id);
    assert_eq!(account.email.as_string(), "user@example.com");
    assert_eq!(account.password_hash.as_str(), "$argon2id$somehash");
    assert_eq!(account.quota.bytes(), 1024 * 1024);
    assert_eq!(account.state, AccountState::Active);
}

#[path = "../src/mailbox.rs"]
pub mod mailbox;

use mailbox::{MailboxPasswordHash, VirtualMailboxAccount, VirtualMailboxManager};

#[test]
fn test_extended_password_hash() {
    let valid_argon = "$argon2id$v=19$m=65536,t=3,p=4$somehash";
    let hash = MailboxPasswordHash::new(valid_argon).unwrap();
    assert_eq!(hash.as_str(), valid_argon);

    let valid_sha512 = "$6$rounds=5000$usesomesillystri$D4IrlXatmP7rx3P3InaxBeoomnAihFOm10TE8IYm0OQQc3eG0L4fW59Hk9Zq9e/tH0yZpT31.C6R2r2C.oU9P.";
    let hash2 = MailboxPasswordHash::new(valid_sha512).unwrap();
    assert_eq!(hash2.as_str(), valid_sha512);

    assert!(MailboxPasswordHash::new("invalid_hash").is_err());
    assert!(MailboxPasswordHash::new("$bcrypt$v=19...").is_err());
}

#[test]
fn test_virtual_mailbox_manager() {
    let mut manager = VirtualMailboxManager::new();

    let domain1 = MailDomain::new("example.com").unwrap();
    manager.add_domain(domain1);

    let email1 = EmailAddress::new("user@example.com").unwrap();
    let hash1 = MailboxPasswordHash::new("$argon2id$somehash").unwrap();
    let quota1 = MailQuota::new(1024).unwrap();

    let account1 = VirtualMailboxAccount::new(email1, hash1, quota1);

    // Should succeed because example.com is in manager
    assert!(manager.add_mailbox(account1).is_ok());

    let email2 = EmailAddress::new("user@otherdomain.com").unwrap();
    let hash2 = MailboxPasswordHash::new("$6$somehash").unwrap();
    let quota2 = MailQuota::new(1024).unwrap();

    let account2 = VirtualMailboxAccount::new(email2, hash2, quota2);

    // Should fail because otherdomain.com is NOT in manager
    assert!(manager.add_mailbox(account2).is_err());

    assert_eq!(manager.mailboxes().len(), 1);
}
