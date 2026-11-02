#![forbid(unsafe_code)]

#[derive(Debug, Clone)]
pub struct ExternalBinding {
    pub source_path: String,
    pub canonical_path: String,
    pub target_path: String,
    pub owner: Option<String>,
    pub purpose: String,
    pub read_write_policy: String, // "ro" or "rw"
    pub mount_review: Option<String>,
    pub symlink_review: Option<String>,
    pub operator_review_reference: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ValidationError {
    PathTraversal,
    ProtectedTarget,
    OwnerAmbiguity,
    UnsafeSymlinkEscape,
    UnreviewedMount,
    MissingReviewReference,
    StrictManagedRootViolation,
}

pub fn validate_external_binding(binding: &ExternalBinding) -> Result<(), ValidationError> {
    let paths = [&binding.source_path, &binding.canonical_path, &binding.target_path];

    // Check path traversal
    for path in paths.iter() {
        if path.contains("..") {
            return Err(ValidationError::PathTraversal);
        }
    }

    // Check protected targets
    let protected_prefixes = ["/etc", "/proc", "/sys"];
    for path in paths.iter() {
        for prefix in protected_prefixes.iter() {
            if path.starts_with(prefix) {
                return Err(ValidationError::ProtectedTarget);
            }
        }
    }

    // Check owner ambiguity
    if binding.owner.is_none() || binding.owner.as_ref().unwrap().trim().is_empty() {
        return Err(ValidationError::OwnerAmbiguity);
    }

    // Operator review reference is required
    if binding.operator_review_reference.is_none() {
        return Err(ValidationError::MissingReviewReference);
    }

    // Symlink rules
    if binding.source_path != binding.canonical_path {
        if binding.symlink_review.is_none() || binding.symlink_review.as_deref() != Some("APPROVED") {
            return Err(ValidationError::UnsafeSymlinkEscape);
        }
    }

    // Mount rules (if it's a mount, we assume mount_review must be Some("APPROVED"))
    // We determine if it's considered a mount based on the presence of mount_review or absence of symlink_review
    // For test purposes, if neither review is provided for a valid binding, it's unreviewed.
    if binding.mount_review.is_none() && binding.symlink_review.is_none() {
        return Err(ValidationError::UnreviewedMount);
    }

    if let Some(mr) = &binding.mount_review {
        if mr != "APPROVED" {
            return Err(ValidationError::UnreviewedMount);
        }
    }

    Ok(())
}

pub fn validate_managed_root(path: &str) -> Result<(), ValidationError> {
    if path.contains("..") {
        return Err(ValidationError::PathTraversal);
    }

    let protected = ["/etc", "/proc", "/sys"];
    for p in protected {
        if path.starts_with(p) {
            return Err(ValidationError::ProtectedTarget);
        }
    }

    // Global managed-root validator is strict
    if !path.starts_with("/srv/managed/") {
        return Err(ValidationError::StrictManagedRootViolation);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_binding() -> ExternalBinding {
        ExternalBinding {
            source_path: "/data/ext_source".to_string(),
            canonical_path: "/data/ext_source".to_string(),
            target_path: "/var/www/ext_target".to_string(),
            owner: Some("app_user".to_string()),
            purpose: "external document root".to_string(),
            read_write_policy: "ro".to_string(),
            mount_review: Some("APPROVED".to_string()),
            symlink_review: None,
            operator_review_reference: Some("TICKET-1234".to_string()),
        }
    }

    #[test]
    fn test_valid_binding() {
        let binding = valid_binding();
        assert_eq!(validate_external_binding(&binding), Ok(()));
    }

    #[test]
    fn test_rejection_of_path_traversal() {
        let mut binding = valid_binding();
        binding.source_path = "/data/ext/../source".to_string();
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::PathTraversal));
    }

    #[test]
    fn test_rejection_of_protected_targets() {
        let mut binding = valid_binding();
        binding.target_path = "/etc/passwd".to_string();
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::ProtectedTarget));

        binding.target_path = "/var/www/ext".to_string();
        binding.source_path = "/proc/1/environ".to_string();
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::ProtectedTarget));

        binding.source_path = "/data/ext".to_string();
        binding.canonical_path = "/sys/class/net".to_string();
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::ProtectedTarget));
    }

    #[test]
    fn test_owner_ambiguity() {
        let mut binding = valid_binding();
        binding.owner = None;
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::OwnerAmbiguity));

        binding.owner = Some("   ".to_string());
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::OwnerAmbiguity));
    }

    #[test]
    fn test_unsafe_symlink_escapes() {
        let mut binding = valid_binding();
        binding.source_path = "/data/symlink".to_string();
        binding.canonical_path = "/var/lib/outside".to_string();
        binding.symlink_review = None;
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::UnsafeSymlinkEscape));

        binding.symlink_review = Some("UNAPPROVED".to_string());
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::UnsafeSymlinkEscape));

        binding.symlink_review = Some("APPROVED".to_string());
        binding.mount_review = None;
        assert_eq!(validate_external_binding(&binding), Ok(()));
    }

    #[test]
    fn test_unreviewed_mounts() {
        let mut binding = valid_binding();
        binding.mount_review = None;
        binding.symlink_review = None;
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::UnreviewedMount));

        binding.mount_review = Some("PENDING".to_string());
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::UnreviewedMount));
    }

    #[test]
    fn test_missing_operator_review() {
        let mut binding = valid_binding();
        binding.operator_review_reference = None;
        assert_eq!(validate_external_binding(&binding), Err(ValidationError::MissingReviewReference));
    }

    #[test]
    fn test_strict_managed_root_validator() {
        // Valid managed root
        assert_eq!(validate_managed_root("/srv/managed/site1"), Ok(()));

        // Missing strict prefix
        assert_eq!(validate_managed_root("/var/www/site1"), Err(ValidationError::StrictManagedRootViolation));

        // Protected target
        assert_eq!(validate_managed_root("/etc/site1"), Err(ValidationError::ProtectedTarget));

        // Path traversal
        assert_eq!(validate_managed_root("/srv/managed/../site1"), Err(ValidationError::PathTraversal));
    }
}
