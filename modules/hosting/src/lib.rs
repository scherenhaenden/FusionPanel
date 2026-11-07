#![forbid(unsafe_code)]

pub mod domain;

// Adding a basic test suite for domain validations
#[cfg(test)]
mod tests {
    use crate::domain::{validate_document_root, DomainName, ResourceState};

    #[test]
    fn test_valid_domain() {
        assert!(DomainName::new("example.com").is_ok());
        assert!(DomainName::new("sub.example.co.uk").is_ok());
    }

    #[test]
    fn test_invalid_domains() {
        assert!(DomainName::new("").is_err());
        assert!(DomainName::new("-example.com").is_err());
        assert!(DomainName::new("example-.com").is_err());
        assert!(DomainName::new("192.168.1.1").is_err()); // IP literal
        assert!(DomainName::new("exa mple.com").is_err()); // spaces
        assert!(DomainName::new("münchen.de").is_err()); // IDNA explicitly rejected
    }

    #[test]
    fn test_document_root_validation() {
        assert!(validate_document_root("/srv/fusionpanel/sites/example.com").is_ok());
        assert!(validate_document_root("/var/www/html").is_err());
        assert!(validate_document_root("/srv/fusionpanel/sites/../secret").is_err());
        assert!(validate_document_root("/srv/fusionpanel/sites/foo\0bar").is_err());
        assert!(validate_document_root("srv/fusionpanel/sites/example").is_err());
        // Not absolute
    }

    #[test]
    fn test_resource_state_transitions() {
        let state = ResourceState::Pending;
        assert_eq!(
            state.transition_to(ResourceState::Active).unwrap(),
            ResourceState::Active
        );

        let state = ResourceState::Active;
        assert!(state.transition_to(ResourceState::Pending).is_err());
        assert_eq!(
            state.transition_to(ResourceState::Suspended).unwrap(),
            ResourceState::Suspended
        );
    }
}
pub mod config;
pub mod acme;
pub mod vhost;
pub mod upstream;
pub mod compression;
pub mod php_tuning;

#[cfg(test)]
mod user_tests {
    use crate::domain::validation::OsUsername;
    use std::convert::TryInto;

    #[test]
    fn test_valid_os_usernames() {
        let name: Result<OsUsername, _> = "valid-user".to_string().try_into();
        assert!(name.is_ok());
        let name: Result<OsUsername, _> = "user_name123".to_string().try_into();
        assert!(name.is_ok());
    }

    #[test]
    fn test_invalid_os_usernames() {
        let name: Result<OsUsername, _> = "".to_string().try_into();
        assert!(name.is_err());
        let name: Result<OsUsername, _> = "toolongusername12345678901234567890".to_string().try_into();
        assert!(name.is_err());
        let name: Result<OsUsername, _> = "-leadinghyphen".to_string().try_into();
        assert!(name.is_err());
        let name: Result<OsUsername, _> = "root".to_string().try_into();
        assert!(name.is_err());
        let name: Result<OsUsername, _> = "www-data".to_string().try_into();
        assert!(name.is_err());
        let name: Result<OsUsername, _> = "user name".to_string().try_into();
        assert!(name.is_err());
    }

    #[test]
    fn test_username_normalization() {
        let name: OsUsername = "tenant-123".to_string().try_into().unwrap();
        assert_eq!(name.normalized(), "tenant_123");
    }
}
