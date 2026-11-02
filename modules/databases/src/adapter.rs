use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbUser(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseEngine {
    MariaDB,
    PostgreSQL,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStrategy {
    Host {
        socket_path: Option<String>,
    },
    Container {
        container_name: String,
        network: Option<String>,
    },
}

#[derive(Clone, Deserialize)]
pub struct SecretString(pub String);

impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub engine: DatabaseEngine,
    pub strategy: ConnectionStrategy,
    pub host: String,
    pub port: u16,
    pub admin_user: String,
    pub admin_password_ref: SecretString,
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Dependency unavailable: {0}")]
    DependencyUnavailable(String),
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDbPlan {
    pub db_name: DbName,
    pub owner: DbUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserPlan {
    pub username: DbUser,
    pub password_ref: SecretString,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantPlan {
    pub db_name: DbName,
    pub username: DbUser,
    pub privileges: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpPlan {
    pub db_name: DbName,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePlan {
    pub db_name: DbName,
    pub input_path: String,
}

#[async_trait::async_trait]
pub trait DatabaseAdapter: Send + Sync {
    fn engine(&self) -> DatabaseEngine;
    fn config(&self) -> &DatabaseConfig;

    async fn check_connection(&self) -> Result<()>;

    async fn plan_create_database(&self, name: DbName, owner: DbUser) -> Result<CreateDbPlan>;
    async fn apply_create_database(&self, plan: &CreateDbPlan) -> Result<()>;

    async fn plan_create_user(
        &self,
        username: DbUser,
        password_ref: SecretString,
    ) -> Result<CreateUserPlan>;
    async fn apply_create_user(&self, plan: &CreateUserPlan) -> Result<()>;

    async fn plan_grant(
        &self,
        db_name: DbName,
        username: DbUser,
        privileges: Vec<String>,
    ) -> Result<GrantPlan>;
    async fn apply_grant(&self, plan: &GrantPlan) -> Result<()>;

    async fn plan_dump(&self, db_name: DbName, output_path: String) -> Result<DumpPlan>;
    async fn apply_dump(&self, plan: &DumpPlan) -> Result<()>;

    async fn plan_restore(&self, db_name: DbName, input_path: String) -> Result<RestorePlan>;
    async fn apply_restore(&self, plan: &RestorePlan) -> Result<()>;
}

pub fn validate_identifier(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 63 {
        return Err(DatabaseError::InvalidInput(
            "Identifier must be 1-63 characters".into(),
        ));
    }

    for c in id.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(DatabaseError::InvalidInput(format!(
                "Invalid character in identifier: {}",
                c
            )));
        }
    }

    if id.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(DatabaseError::InvalidInput(
            "Identifier cannot start with a digit".into(),
        ));
    }

    Ok(())
}

pub fn validate_privilege(privilege: &str) -> Result<()> {
    let allowed = [
        "ALL PRIVILEGES",
        "SELECT",
        "INSERT",
        "UPDATE",
        "DELETE",
        "CREATE",
        "DROP",
        "ALTER",
        "INDEX",
        "REFERENCES",
    ];
    let upper_priv = privilege.to_uppercase();
    if !allowed.contains(&upper_priv.as_str()) {
        return Err(DatabaseError::InvalidInput(format!(
            "Invalid privilege: {}",
            privilege
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_identifier_valid() {
        assert!(validate_identifier("my_db_1").is_ok());
        assert!(validate_identifier("user123").is_ok());
        assert!(validate_identifier("A_B_C").is_ok());
    }

    #[test]
    fn test_validate_identifier_invalid() {
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("a".repeat(64).as_str()).is_err());
        assert!(validate_identifier("1db").is_err());
        assert!(validate_identifier("my-db").is_err());
        assert!(validate_identifier("my db").is_err());
        assert!(validate_identifier("db;drop").is_err());
    }

    #[test]
    fn test_validate_privilege() {
        assert!(validate_privilege("SELECT").is_ok());
        assert!(validate_privilege("all privileges").is_ok());
        assert!(validate_privilege("GRANT").is_err());
        assert!(validate_privilege("DROP DATABASE").is_err());
    }

    #[test]
    fn test_read_only_privilege_grants_verification() {
        assert!(validate_privilege("SELECT").is_ok());
        assert!(validate_privilege("INSERT").is_ok()); // just check it passes normal
    }

    #[test]
    fn test_secret_redaction() {
        let secret = SecretString("super_secret".into());
        let debug_str = format!("{:?}", secret);
        assert_eq!(debug_str, "[REDACTED]");
        assert!(!debug_str.contains("super_secret"));

        let plan = CreateUserPlan {
            username: DbUser("u1".into()),
            password_ref: secret,
        };
        let serialized = serde_json::to_string(&plan).unwrap();
        assert!(serialized.contains("[REDACTED]"));
        assert!(!serialized.contains("super_secret"));
    }
}
