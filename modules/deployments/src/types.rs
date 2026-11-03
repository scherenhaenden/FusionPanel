use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeploymentId(pub String);

impl DeploymentId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn parse(s: &str) -> Result<Self, crate::error::DeploymentError> {
        uuid::Uuid::parse_str(s)
            .map(|u| Self(u.to_string()))
            .map_err(|e| {
                crate::error::DeploymentError::InvalidInput(format!("invalid deployment id: {e}"))
            })
    }
}

impl Default for DeploymentId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReleaseId(String);

impl ReleaseId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn parse(s: &str) -> Result<Self, crate::error::DeploymentError> {
        if s.is_empty() {
            return Err(crate::error::DeploymentError::InvalidInput("invalid release id: empty".to_string()));
        }
        for c in s.chars() {
            if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
                return Err(crate::error::DeploymentError::InvalidInput(format!("invalid release id: invalid character '{}'", c)));
            }
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl serde::Serialize for ReleaseId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for ReleaseId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ReleaseId::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl Default for ReleaseId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SiteId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Production,
    Staging,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStrategy {
    ReleaseDirectorySwitch,
    BlueGreen,
    ContainerRelease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentState {
    Queued,
    PendingApproval,
    Building,
    Deploying,
    Active,
    RolledBack,
    Failed,
    Cancelled,
}

impl DeploymentState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Active | Self::RolledBack | Self::Failed | Self::Cancelled
        )
    }

    pub fn can_transition_to(&self, next: DeploymentState) -> bool {
        match (self, next) {
            (DeploymentState::Queued, DeploymentState::PendingApproval) => true,
            (DeploymentState::Queued, DeploymentState::Building) => true,
            (DeploymentState::Queued, DeploymentState::Cancelled) => true,

            (DeploymentState::PendingApproval, DeploymentState::Building) => true,
            (DeploymentState::PendingApproval, DeploymentState::Cancelled) => true,
            (DeploymentState::PendingApproval, DeploymentState::Failed) => true,

            (DeploymentState::Building, DeploymentState::Deploying) => true,
            (DeploymentState::Building, DeploymentState::Failed) => true,
            (DeploymentState::Building, DeploymentState::Cancelled) => true,

            (DeploymentState::Deploying, DeploymentState::Active) => true,
            (DeploymentState::Deploying, DeploymentState::Failed) => true,
            (DeploymentState::Deploying, DeploymentState::Cancelled) => true,

            (DeploymentState::Active, DeploymentState::RolledBack) => true,

            // No transitions out of terminal states
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentProgressEvent {
    pub timestamp: DateTime<Utc>,
    pub stage: String,
    pub message: String,
    pub level: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSource {
    pub repo_url: String,
    pub commit_sha: String,
    pub branch: Option<String>,
    pub ref_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildSpec {
    pub project_type: ProjectType,
    pub command: Option<String>,
    pub artifact_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    Static,
    Rust,
    Node,
    Python,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentPlan {
    pub plan_id: String,
    pub site_id: SiteId,
    pub environment: Environment,
    pub strategy: DeploymentStrategy,
    pub git_source: GitSource,
    pub build_spec: BuildSpec,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedResult {
    pub deployment_id: DeploymentId,
    pub release_id: Option<ReleaseId>,
    pub previous_release_id: Option<ReleaseId>,
    pub state: DeploymentState,
    pub active_path: Option<String>,
    pub slot: Option<BlueGreenSlot>,
    pub applied_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlueGreenSlot {
    Blue,
    Green,
}

impl BlueGreenSlot {
    pub fn opposite(&self) -> Self {
        match self {
            Self::Blue => Self::Green,
            Self::Green => Self::Blue,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedState {
    pub deployment_id: DeploymentId,
    pub release_id: Option<ReleaseId>,
    pub state: DeploymentState,
    pub environment: Environment,
    pub current_slot: Option<BlueGreenSlot>,
    pub health_status: HealthStatus,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}
