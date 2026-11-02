use serde::{Deserialize, Serialize};

use super::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef(String);

impl ImageRef {
    pub fn new(image: String) -> Result<Self, DomainError> {
        if image.is_empty() {
            return Err(DomainError::InvalidImageRef);
        }
        // Minimal validation, format like 'repo/image:tag'
        if image.contains(' ') || image.len() > 255 {
            return Err(DomainError::InvalidImageRef);
        }
        Ok(Self(image))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
