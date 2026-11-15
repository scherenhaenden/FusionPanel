#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use thiserror::Error;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = DependencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(DependencyError::InvalidVersion(s.to_string()));
        }
        let major = parts.get(0).unwrap_or(&"0").parse().map_err(|_| DependencyError::InvalidVersion(s.to_string()))?;
        let minor = parts.get(1).unwrap_or(&"0").parse().map_err(|_| DependencyError::InvalidVersion(s.to_string()))?;
        let patch = parts.get(2).unwrap_or(&"0").parse().map_err(|_| DependencyError::InvalidVersion(s.to_string()))?;
        Ok(Version { major, minor, patch })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionConstraint {
    Exact(Version),
    GreaterEq(Version),
    Greater(Version),
    LessEq(Version),
    Less(Version),
    Any,
}

impl FromStr for VersionConstraint {
    type Err = DependencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s == "*" || s.is_empty() {
            return Ok(VersionConstraint::Any);
        } else if let Some(v) = s.strip_prefix(">=") {
            return Ok(VersionConstraint::GreaterEq(v.trim().parse()?));
        } else if let Some(v) = s.strip_prefix("<=") {
            return Ok(VersionConstraint::LessEq(v.trim().parse()?));
        } else if let Some(v) = s.strip_prefix(">") {
            return Ok(VersionConstraint::Greater(v.trim().parse()?));
        } else if let Some(v) = s.strip_prefix("<") {
            return Ok(VersionConstraint::Less(v.trim().parse()?));
        } else if let Some(v) = s.strip_prefix("=") {
            return Ok(VersionConstraint::Exact(v.trim().parse()?));
        } else {
            return Ok(VersionConstraint::Exact(s.parse()?));
        }
    }
}

impl VersionConstraint {
    pub fn matches(&self, version: &Version) -> bool {
        match self {
            VersionConstraint::Exact(v) => version == v,
            VersionConstraint::GreaterEq(v) => version >= v,
            VersionConstraint::Greater(v) => version > v,
            VersionConstraint::LessEq(v) => version <= v,
            VersionConstraint::Less(v) => version < v,
            VersionConstraint::Any => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub constraint: VersionConstraint,
}

impl Dependency {
    pub fn new(name: &str, constraint: &str) -> Result<Self, DependencyError> {
        Ok(Self {
            name: name.to_string(),
            constraint: constraint.parse()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub version: Version,
    pub dependencies: Vec<Dependency>,
}

impl Package {
    pub fn new(name: &str, version: &str) -> Result<Self, DependencyError> {
        Ok(Self {
            name: name.to_string(),
            version: version.parse()?,
            dependencies: Vec::new(),
        })
    }

    pub fn add_dependency(&mut self, dep: Dependency) {
        self.dependencies.push(dep);
    }

    pub fn with_dependency(mut self, dep: Dependency) -> Self {
        self.dependencies.push(dep);
        self
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum DependencyError {
    #[error("Invalid version format: {0}")]
    InvalidVersion(String),
    #[error("Package not found: {0}")]
    PackageNotFound(String),
    #[error("Version incompatibility for package: {0}")]
    VersionIncompatibility(String),
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),
}

#[derive(Default)]
pub struct DependencySolver {
    pub packages: BTreeMap<String, Vec<Package>>,
}

impl DependencySolver {
    pub fn new() -> Self {
        Self {
            packages: BTreeMap::new(),
        }
    }

    pub fn add_package(&mut self, package: Package) {
        self.packages.entry(package.name.clone()).or_default().push(package);
    }

    pub fn resolve(&self, target_name: &str, constraint: &VersionConstraint) -> Result<Vec<Package>, DependencyError> {
        let mut resolved = BTreeMap::new();
        let mut path = Vec::new();

        self.resolve_recursive(target_name, constraint, &mut resolved, &mut path)?;

        Ok(resolved.into_values().collect())
    }

    fn resolve_recursive(
        &self,
        name: &str,
        constraint: &VersionConstraint,
        resolved: &mut BTreeMap<String, Package>,
        path: &mut Vec<String>,
    ) -> Result<(), DependencyError> {
        if path.iter().any(|p| p == name) {
            return Err(DependencyError::CircularDependency(name.to_string()));
        }

        if let Some(existing) = resolved.get(name) {
            if !constraint.matches(&existing.version) {
                return Err(DependencyError::VersionIncompatibility(name.to_string()));
            }
            return Ok(());
        }

        let candidates = self.packages.get(name).ok_or_else(|| DependencyError::PackageNotFound(name.to_string()))?;

        path.push(name.to_string());

        let mut matching_candidates: Vec<&Package> = candidates.iter()
            .filter(|p| constraint.matches(&p.version))
            .collect();

        matching_candidates.sort_by(|a, b| b.version.cmp(&a.version));

        let mut last_error = DependencyError::PackageNotFound(name.to_string());

        for candidate in matching_candidates {
            resolved.insert(name.to_string(), (*candidate).clone());
            let mut success = true;

            let current_resolved = resolved.clone();

            for dep in &candidate.dependencies {
                if let Err(e) = self.resolve_recursive(&dep.name, &dep.constraint, resolved, path) {
                    success = false;
                    last_error = e;
                    break;
                }
            }

            if success {
                path.pop();
                return Ok(());
            } else {
                *resolved = current_resolved;
            }
        }

        path.pop();

        if !candidates.iter().any(|p| constraint.matches(&p.version)) {
            Err(DependencyError::VersionIncompatibility(name.to_string()))
        } else {
            Err(last_error)
        }
    }
}
