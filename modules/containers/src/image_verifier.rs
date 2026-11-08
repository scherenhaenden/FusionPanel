#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    MalformedReference,
    IllegalCharacters,
    FloatingTagNotAllowed,
    RegistryNotAllowed(String),
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::MalformedReference => write!(f, "Malformed image reference"),
            VerifyError::IllegalCharacters => write!(f, "Image reference contains illegal characters"),
            VerifyError::FloatingTagNotAllowed => write!(f, "Image reference must use an immutable digest pin"),
            VerifyError::RegistryNotAllowed(r) => write!(f, "Registry '{}' is not in the allowed list", r),
        }
    }
}

impl std::error::Error for VerifyError {}

#[derive(Debug, Clone)]
pub struct ImagePolicyVerifier {
    require_digest_pinning: bool,
    allowed_registries: Vec<String>,
}

impl ImagePolicyVerifier {
    pub fn new(require_digest_pinning: bool, allowed_registries: Vec<String>) -> Self {
        Self {
            require_digest_pinning,
            allowed_registries,
        }
    }

    pub fn verify(&self, image: &str) -> Result<(), VerifyError> {
        // 1. Check for illegal characters
        let illegal_chars = ['\0', '\n', '\r', '\t', '&', '|', ';', '$', '`', '<', '>', '\\', ' ', '"', '\''];
        if image.chars().any(|c| illegal_chars.contains(&c)) {
            return Err(VerifyError::IllegalCharacters);
        }

        // 2. Parse the image reference
        // Format: [registry/][namespace/]repository[:tag][@digest]
        
        // Find @digest
        let (repo_tag, digest) = if let Some(idx) = image.rfind('@') {
            (&image[..idx], Some(&image[idx + 1..]))
        } else {
            (image, None)
        };

        // From repo_tag, find tag. Tag is after the LAST colon, but only if that colon is AFTER the last slash.
        let (repo, _tag) = if let Some(colon_idx) = repo_tag.rfind(':') {
            if let Some(slash_idx) = repo_tag.rfind('/') {
                if colon_idx > slash_idx {
                    (&repo_tag[..colon_idx], Some(&repo_tag[colon_idx + 1..]))
                } else {
                    (repo_tag, None)
                }
            } else {
                (&repo_tag[..colon_idx], Some(&repo_tag[colon_idx + 1..]))
            }
        } else {
            (repo_tag, None)
        };

        if repo.is_empty() {
            return Err(VerifyError::MalformedReference);
        }

        // Verify digest pinning
        if self.require_digest_pinning && digest.is_none() {
            return Err(VerifyError::FloatingTagNotAllowed);
        }

        if let Some(d) = digest {
            if !d.starts_with("sha256:") {
                return Err(VerifyError::MalformedReference);
            }
        }

        // Determine registry
        let registry = if let Some(slash_idx) = repo.find('/') {
            let first_part = &repo[..slash_idx];
            // Does first_part contain '.' or ':' or is 'localhost'?
            if first_part.contains('.') || first_part.contains(':') || first_part == "localhost" {
                first_part.to_string()
            } else {
                "docker.io".to_string()
            }
        } else {
            "docker.io".to_string()
        };

        // Check registry against allowlist
        if !self.allowed_registries.is_empty() && !self.allowed_registries.contains(&registry) {
            return Err(VerifyError::RegistryNotAllowed(registry));
        }

        Ok(())
    }
}
