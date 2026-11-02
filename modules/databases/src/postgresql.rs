use crate::adapter::*;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

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
            if stderr.contains("already exists") {
                return Ok(());
            }
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
        let escaped_pwd = plan.password_ref.0.replace("'", "''");
        let sql = format!(
            "CREATE USER \"{}\" WITH PASSWORD '{}';",
            plan.username.0, escaped_pwd
        );

        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", "postgres"]);
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
                    "ALTER USER \"{}\" WITH PASSWORD '{}';",
                    plan.username.0, escaped_pwd
                );
                let mut alter_cmd = self.build_cmd("psql");
                alter_cmd.args(["-d", "postgres"]);
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
                    return Err(DatabaseError::OperationFailed(
                        String::from_utf8_lossy(&alter_output.stderr).to_string(),
                    ));
                }
                return Ok(());
            }
            return Err(DatabaseError::OperationFailed(stderr));
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
        let privs = if plan.privileges.is_empty() {
            "ALL PRIVILEGES".to_string()
        } else {
            plan.privileges.join(", ")
        };
        let sql = format!(
            "GRANT {} ON DATABASE \"{}\" TO \"{}\";",
            privs, plan.db_name.0, plan.username.0
        );

        let mut cmd = self.build_cmd("psql");
        cmd.args(["-d", "postgres", "-c", &sql]);

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
        let mut cmd = self.build_cmd("pg_dump");
        cmd.arg("-d").arg(&plan.db_name.0);

        let file = std::fs::File::create(&plan.output_path).map_err(|e| {
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
        let mut cmd = self.build_cmd("psql");
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
}
