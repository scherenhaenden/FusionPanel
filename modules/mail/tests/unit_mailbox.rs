use mail::models::{
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
