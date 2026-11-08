use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseManifest {
    pub version: String,
    pub components: Vec<Component>,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Component {
    pub name: String,
    pub version: String,
    pub hash: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("Missing or empty version")]
    EmptyVersion,
    #[error("No components provided")]
    EmptyComponents,
    #[error("Component '{0}' has empty fields")]
    InvalidComponent(String),
}

impl ReleaseManifest {
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.version.is_empty() {
            return Err(ManifestError::EmptyVersion);
        }
        if self.components.is_empty() {
            return Err(ManifestError::EmptyComponents);
        }
        for comp in &self.components {
            if comp.name.is_empty() || comp.version.is_empty() || comp.hash.is_empty() {
                return Err(ManifestError::InvalidComponent(comp.name.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_manifest() {
        let manifest = ReleaseManifest {
            version: "1.0.0".to_string(),
            components: vec![Component {
                name: "fusionpanel-api".to_string(),
                version: "1.0.0".to_string(),
                hash: "abcdef".to_string(),
            }],
            timestamp: "2026-11-10T00:00:00Z".to_string(),
        };
        assert_eq!(manifest.validate(), Ok(()));
    }

    #[test]
    fn test_empty_version() {
        let manifest = ReleaseManifest {
            version: "".to_string(),
            components: vec![Component {
                name: "fusionpanel-api".to_string(),
                version: "1.0.0".to_string(),
                hash: "abcdef".to_string(),
            }],
            timestamp: "2026-11-10T00:00:00Z".to_string(),
        };
        assert_eq!(manifest.validate(), Err(ManifestError::EmptyVersion));
    }

    #[test]
    fn test_empty_components() {
        let manifest = ReleaseManifest {
            version: "1.0.0".to_string(),
            components: vec![],
            timestamp: "2026-11-10T00:00:00Z".to_string(),
        };
        assert_eq!(manifest.validate(), Err(ManifestError::EmptyComponents));
    }

    #[test]
    fn test_invalid_component() {
        let manifest = ReleaseManifest {
            version: "1.0.0".to_string(),
            components: vec![Component {
                name: "fusionpanel-api".to_string(),
                version: "".to_string(), // Invalid
                hash: "abcdef".to_string(),
            }],
            timestamp: "2026-11-10T00:00:00Z".to_string(),
        };
        assert_eq!(
            manifest.validate(),
            Err(ManifestError::InvalidComponent(
                "fusionpanel-api".to_string()
            ))
        );
    }

    #[test]
    fn test_serialization() {
        let manifest = ReleaseManifest {
            version: "1.0.0".to_string(),
            components: vec![Component {
                name: "fusionpanel-api".to_string(),
                version: "1.0.0".to_string(),
                hash: "abcdef".to_string(),
            }],
            timestamp: "2026-11-10T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: ReleaseManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, parsed);
    }
}
