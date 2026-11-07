#![forbid(unsafe_code)]

use crate::adapter::*;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;
use uuid::Uuid;

fn quote_password_literal(password: &str) -> String {
    loop {
        let delimiter = format!("$fusionpanel_{}$", Uuid::new_v4().simple());
        if !password.contains(&delimiter) {
            return format!("{delimiter}{password}{delimiter}");
        }
    }
}

pub struct PostgresAdapter {
    config: DatabaseConfig,
}

impl PostgresAdapter {
    pub fn new(config: DatabaseConfig) -> Self {
        Self { config }
    }

    fn build_cmd(&self, program: &str) -> Command {
        let mut cmd;

        match &self.config.strategy {
            ConnectionStrategy::Host { socket_path: _ } => {
                cmd = Command::new(program);
                cmd.arg("-h").arg(&self.config.host);
                cmd.arg("-p").arg(self.config.port.to_string());
                cmd.env("PGPASSWORD", &self.config.admin_password_ref.0);
            }
            ConnectionStrategy::Container { container_name, .. } => {
                let mut container_cmd = Command::new("docker");
                container_cmd.args(["exec", "-i", "-e", "PGPASSWORD", container_name, program]);
                container_cmd.env("PGPASSWORD", &self.config.admin_password_ref.0);
                cmd = container_cmd;
            }
        }

        cmd.arg("-U").arg(&self.config.admin_user);

        cmd
    }
}

#[async_trait]
impl DatabaseAdapter for PostgresAdapter {
    fn engine(&self) -> DatabaseEngine {
        DatabaseEngine::PostgreSQL
    }

    fn config(&self) -> &DatabaseConfig {
        &self.config
    }

    async fn database_exists(&self, name: &DbName) -> Result<bool> {
        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", "postgres", "-t", "-A", "-c", &format!(
            "SELECT 1 FROM pg_database WHERE datname = '{}';",
            name.0
        )]);
        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql: {}", e))
        })?;
        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let out_str = String::from_utf8_lossy(&output.stdout);
        Ok(out_str.trim() == "1")
    }

    async fn list_databases(&self) -> Result<Vec<ObservedDatabase>> {
        let mut cmd = self.build_cmd("psql");
        // Filter out template1, template0, etc. Postgres typical system schemas don't appear in pg_database but system templates do.
        // We exclude databases starting with 'template' and 'postgres' itself.
        cmd.args(["-d", "postgres", "-t", "-A", "-c",
            "SELECT datname, pg_encoding_to_char(encoding), datcollate FROM pg_database \
             WHERE datistemplate = false AND datname != 'postgres';"
        ]);
        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql: {}", e))
        })?;
        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let out_str = String::from_utf8_lossy(&output.stdout);
        let mut dbs = Vec::new();
        for line in out_str.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 3 {
                dbs.push(ObservedDatabase {
                    name: DbName(parts[0].to_string()),
                    charset: parts[1].to_string(),
                    collation: parts[2].to_string(),
                });
            }
        }
        Ok(dbs)
    }

    async fn user_exists(&self, username: &DbUser) -> Result<bool> {
        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", "postgres", "-t", "-A", "-c", &format!(
            "SELECT 1 FROM pg_roles WHERE rolname = '{}';",
            username.0
        )]);
        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql: {}", e))
        })?;
        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let out_str = String::from_utf8_lossy(&output.stdout);
        Ok(out_str.trim() == "1")
    }

    async fn check_connection(&self) -> Result<()> {
        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", "postgres", "-c", "SELECT 1;"]);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::ConnectionFailed(format!("Failed to execute psql: {}", e))
        })?;

        if !output.status.success() {
            return Err(DatabaseError::ConnectionFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    async fn plan_create_database(&self, name: DbName, owner: DbUser) -> Result<CreateDbPlan> {
        validate_identifier(&name.0)?;
        validate_identifier(&owner.0)?;
        Ok(CreateDbPlan {
            db_name: name,
            owner,
        })
    }

    async fn apply_create_database(&self, plan: &CreateDbPlan) -> Result<()> {
        validate_identifier(&plan.db_name.0)?;
        validate_identifier(&plan.owner.0)?;

        if self.database_exists(&plan.db_name).await? {
            return Err(DatabaseError::Conflict(format!(
                "Database '{}' already exists",
                plan.db_name.0
            )));
        }

        let sql = format!(
            "CREATE DATABASE \"{}\" OWNER \"{}\" ENCODING 'UTF8';",
            plan.db_name.0, plan.owner.0
        );

        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", "postgres", "-c", &sql]);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(DatabaseError::OperationFailed(stderr));
        }
        Ok(())
    }

    async fn plan_create_user(
        &self,
        username: DbUser,
        password_ref: SecretString,
    ) -> Result<CreateUserPlan> {
        validate_identifier(&username.0)?;
        Ok(CreateUserPlan {
            username,
            password_ref,
        })
    }

    async fn apply_create_user(&self, plan: &CreateUserPlan) -> Result<()> {
        validate_identifier(&plan.username.0)?;
        let sql = format!(
            "CREATE USER \"{}\" WITH PASSWORD {};",
            plan.username.0,
            quote_password_literal(&plan.password_ref.0)
        );

        let mut cmd = self.build_cmd("psql");
        cmd.args(["-v", "ON_ERROR_STOP=1", "-d", "postgres"]);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to spawn psql: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(sql.as_bytes()).await.map_err(|e| {
                DatabaseError::OperationFailed(format!("Failed to write to stdin: {}", e))
            })?;
        }

        let output = child.wait_with_output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to wait for psql: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("already exists") {
                let alter_sql = format!(
                    "ALTER USER \"{}\" WITH PASSWORD {};",
                    plan.username.0,
                    quote_password_literal(&plan.password_ref.0)
                );
                let mut alter_cmd = self.build_cmd("psql");
                alter_cmd.args(["-v", "ON_ERROR_STOP=1", "-d", "postgres"]);
                alter_cmd
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let mut alter_child = alter_cmd.spawn().map_err(|e| {
                    DatabaseError::OperationFailed(format!("Failed to spawn psql alter: {}", e))
                })?;

                if let Some(mut stdin) = alter_child.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    stdin.write_all(alter_sql.as_bytes()).await.map_err(|e| {
                        DatabaseError::OperationFailed(format!(
                            "Failed to write to stdin alter: {}",
                            e
                        ))
                    })?;
                }
                let alter_output = alter_child.wait_with_output().await.map_err(|e| {
                    DatabaseError::OperationFailed(format!("Failed to wait for psql alter: {}", e))
                })?;
                if !alter_output.status.success() {
                    let alter_err = String::from_utf8_lossy(&alter_output.stderr).to_string();
                    let safe_err = alter_err.replace(&plan.password_ref.0, "[REDACTED]");
                    return Err(DatabaseError::OperationFailed(safe_err));
                }
                return Ok(());
            }
            let safe_err = stderr.replace(&plan.password_ref.0, "[REDACTED]");
            return Err(DatabaseError::OperationFailed(safe_err));
        }
        Ok(())
    }

    async fn plan_grant(
        &self,
        db_name: DbName,
        username: DbUser,
        privileges: Vec<String>,
    ) -> Result<GrantPlan> {
        validate_identifier(&db_name.0)?;
        validate_identifier(&username.0)?;
        for priv_str in &privileges {
            validate_privilege(priv_str)?;
        }
        Ok(GrantPlan {
            db_name,
            username,
            privileges,
        })
    }

    async fn apply_grant(&self, plan: &GrantPlan) -> Result<()> {
        validate_identifier(&plan.db_name.0)?;
        validate_identifier(&plan.username.0)?;
        for priv_str in &plan.privileges {
            validate_privilege(priv_str)?;
        }

        let privs = if plan.privileges.is_empty() {
            "ALL".to_string() // In postgres, ALL PRIVILEGES is also valid but ALL is standard
        } else {
            plan.privileges.join(", ")
        };

        // Connect to the specific database to grant table privileges on schema public
        // First grant CONNECT to the database
        let connect_sql = format!(
            "GRANT CONNECT ON DATABASE \"{}\" TO \"{}\";",
            plan.db_name.0, plan.username.0
        );
        let mut cmd_connect = self.build_cmd("psql");
        cmd_connect.args(["-d", "postgres", "-c", &connect_sql, "-v", "ON_ERROR_STOP=1"]);

        let output_connect = cmd_connect.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql for grant connect: {}", e))
        })?;

        if !output_connect.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output_connect.stderr).to_string(),
            ));
        }

        // Then grant privileges on all tables in public schema
        let sql = format!(
            "GRANT {} ON ALL TABLES IN SCHEMA public TO \"{}\";",
            privs, plan.username.0
        );

        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", &plan.db_name.0, "-c", &sql, "-v", "ON_ERROR_STOP=1"]);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql: {}", e))
        })?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    async fn plan_dump(&self, db_name: DbName, output_path: String) -> Result<DumpPlan> {
        validate_identifier(&db_name.0)?;
        Ok(DumpPlan {
            db_name,
            output_path,
        })
    }

    async fn apply_dump(&self, plan: &DumpPlan) -> Result<()> {
        validate_identifier(&plan.db_name.0)?;
        let mut cmd = self.build_cmd("pg_dump");
        cmd.arg("-d").arg(&plan.db_name.0);

        // Ensure dump is protected using standard library mechanisms
        let mut file_options = std::fs::OpenOptions::new();
        file_options.write(true).create(true).truncate(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            file_options.mode(0o600); // Only owner can read/write
        }

        let file = file_options.open(&plan.output_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to create dump file: {}", e))
        })?;

        cmd.stdout(Stdio::from(file));

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute pg_dump: {}", e))
        })?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    async fn plan_restore(&self, db_name: DbName, input_path: String) -> Result<RestorePlan> {
        validate_identifier(&db_name.0)?;
        Ok(RestorePlan {
            db_name,
            input_path,
        })
    }

    async fn apply_restore(&self, plan: &RestorePlan) -> Result<()> {
        validate_identifier(&plan.db_name.0)?;
        let mut cmd = self.build_cmd("psql");
        cmd.args(["-v", "ON_ERROR_STOP=1"]);
        cmd.arg("-d").arg(&plan.db_name.0);

        let file = std::fs::File::open(&plan.input_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to open restore file: {}", e))
        })?;

        cmd.stdin(Stdio::from(file));

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql restore: {}", e))
        })?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    async fn drop_database(&self, name: &DbName) -> Result<()> {
        validate_identifier(&name.0)?;

        if !self.database_exists(name).await? {
            return Ok(());
        }

        let sql = format!("DROP DATABASE \"{}\";", name.0);

        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", "postgres", "-c", &sql]);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql for drop db: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(DatabaseError::OperationFailed(stderr));
        }
        Ok(())
    }

    async fn drop_user(&self, username: &DbUser) -> Result<()> {
        validate_identifier(&username.0)?;

        if !self.user_exists(username).await? {
            return Ok(());
        }

        let sql = format!("DROP USER \"{}\";", username.0);

        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", "postgres", "-c", &sql]);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute psql for drop user: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(DatabaseError::OperationFailed(stderr));
        }
        Ok(())
    }
}
