#![forbid(unsafe_code)]

use async_trait::async_trait;
use databases::adapter::{
    ConnectionStrategy, DatabaseAdapter, DatabaseConfig, DatabaseEngine, DbName, DbUser,
    SecretString,
};
use databases::mariadb::MariaDbAdapter;
use databases::postgres::PostgresAdapter;
use fusionpanel_agent_protocol::DatabaseEngineKind;

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn apply_database(&self, engine: DatabaseEngineKind, name: &str) -> Result<(), String>;
    async fn delete_database(&self, engine: DatabaseEngineKind, name: &str) -> Result<(), String>;
    async fn create_user(
        &self,
        engine: DatabaseEngineKind,
        database_name: &str,
        username: &str,
        password: &str,
    ) -> Result<(), String>;
}

pub struct RealDatabaseDriver;

impl RealDatabaseDriver {
    fn postgres_adapter() -> PostgresAdapter {
        let host = std::env::var("FUSIONPANEL_PG_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("FUSIONPANEL_PG_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(5432);
        let admin_user =
            std::env::var("FUSIONPANEL_PG_USER").unwrap_or_else(|_| "postgres".to_string());
        let admin_password = std::env::var("FUSIONPANEL_PG_PASSWORD").unwrap_or_default();
        let socket_path = std::env::var("FUSIONPANEL_PG_SOCKET").ok();
        PostgresAdapter::new(DatabaseConfig {
            engine: DatabaseEngine::PostgreSQL,
            strategy: ConnectionStrategy::Host { socket_path },
            host,
            port,
            admin_user,
            admin_password_ref: SecretString(admin_password),
        })
    }

    fn mariadb_adapter() -> MariaDbAdapter {
        let host =
            std::env::var("FUSIONPANEL_MARIADB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("FUSIONPANEL_MARIADB_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(3306);
        let admin_user =
            std::env::var("FUSIONPANEL_MARIADB_USER").unwrap_or_else(|_| "root".to_string());
        let admin_password = std::env::var("FUSIONPANEL_MARIADB_PASSWORD").unwrap_or_default();
        let socket_path = std::env::var("FUSIONPANEL_MARIADB_SOCKET").ok();
        MariaDbAdapter::new(DatabaseConfig {
            engine: DatabaseEngine::MariaDB,
            strategy: ConnectionStrategy::Host { socket_path },
            host,
            port,
            admin_user,
            admin_password_ref: SecretString(admin_password),
        })
    }

    fn adapter(engine: DatabaseEngineKind) -> Box<dyn DatabaseAdapter> {
        match engine {
            DatabaseEngineKind::Postgresql => Box::new(Self::postgres_adapter()),
            DatabaseEngineKind::Mariadb => Box::new(Self::mariadb_adapter()),
        }
    }
}

#[async_trait]
impl DatabaseDriver for RealDatabaseDriver {
    async fn apply_database(&self, engine: DatabaseEngineKind, name: &str) -> Result<(), String> {
        let adapter = Self::adapter(engine);
        let db_name = DbName(name.to_string());
        if adapter
            .database_exists(&db_name)
            .await
            .map_err(|e| e.to_string())?
        {
            return Ok(());
        }
        let owner = DbUser(adapter.config().admin_user.clone());
        let plan = adapter
            .plan_create_database(db_name, owner)
            .await
            .map_err(|e| e.to_string())?;
        adapter
            .apply_create_database(&plan)
            .await
            .map_err(|e| e.to_string())
    }

    async fn delete_database(&self, engine: DatabaseEngineKind, name: &str) -> Result<(), String> {
        let adapter = Self::adapter(engine);
        let db_name = DbName(name.to_string());
        if !adapter
            .database_exists(&db_name)
            .await
            .map_err(|e| e.to_string())?
        {
            return Ok(());
        }
        adapter
            .drop_database(&db_name)
            .await
            .map_err(|e| e.to_string())
    }

    async fn create_user(
        &self,
        engine: DatabaseEngineKind,
        database_name: &str,
        username: &str,
        password: &str,
    ) -> Result<(), String> {
        let adapter = Self::adapter(engine);
        let db_name = DbName(database_name.to_string());
        let db_user = DbUser(username.to_string());

        if !adapter
            .user_exists(&db_user)
            .await
            .map_err(|e| e.to_string())?
        {
            let user_plan = adapter
                .plan_create_user(db_user.clone(), SecretString(password.to_string()))
                .await
                .map_err(|e| e.to_string())?;
            adapter
                .apply_create_user(&user_plan)
                .await
                .map_err(|e| e.to_string())?;
        }

        let grant_plan = adapter
            .plan_grant(db_name, db_user, vec!["ALL".to_string()])
            .await
            .map_err(|e| e.to_string())?;
        adapter
            .apply_grant(&grant_plan)
            .await
            .map_err(|e| e.to_string())
    }
}
