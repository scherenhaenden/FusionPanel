use crate::adapter::*;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

pub struct MariaDbAdapter {
    config: DatabaseConfig,
}

impl MariaDbAdapter {
    pub fn new(config: DatabaseConfig) -> Self {
        Self { config }
    }

    fn build_cmd(&self, program: &str) -> Command {
        let mut cmd;

        match &self.config.strategy {
            ConnectionStrategy::Host { socket_path } => {
                cmd = Command::new(program);
                if let Some(sock) = socket_path {
                    cmd.arg(format!("--socket={}", sock));
                } else {
                    cmd.arg(format!("--host={}", self.config.host));
                    cmd.arg(format!("--port={}", self.config.port));
                }
                cmd.env("MYSQL_PWD", &self.config.admin_password_ref.0);
            }
            ConnectionStrategy::Container { container_name, .. } => {
                let mut container_cmd = Command::new("docker");
                // Do not pass secret in argv. Let it pass through env. docker exec passes env vars if specified via -e without value
                container_cmd.args(["exec", "-i", "-e", "MYSQL_PWD", container_name, program]);
                container_cmd.env("MYSQL_PWD", &self.config.admin_password_ref.0);
                cmd = container_cmd;
            }
        }

        cmd.arg(format!("--user={}", self.config.admin_user));

        cmd
    }
}

#[async_trait]
impl DatabaseAdapter for MariaDbAdapter {
    fn engine(&self) -> DatabaseEngine {
        DatabaseEngine::MariaDB
    }

    fn config(&self) -> &DatabaseConfig {
        &self.config
    }

    async fn check_connection(&self) -> Result<()> {
        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-e").arg("SELECT 1;");

        let output = cmd.output().await
            .map_err(|e| DatabaseError::ConnectionFailed(format!("Failed to execute mysql: {}", e)))?;

        if !output.status.success() {
            return Err(DatabaseError::ConnectionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        Ok(())
    }

    async fn plan_create_database(&self, name: DbName, owner: DbUser) -> Result<CreateDbPlan> {
        validate_identifier(&name.0)?;
        validate_identifier(&owner.0)?;
        Ok(CreateDbPlan { db_name: name, owner })
    }

    async fn apply_create_database(&self, plan: &CreateDbPlan) -> Result<()> {
        let sql = format!("CREATE DATABASE IF NOT EXISTS `{}`;", plan.db_name.0);

        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-e").arg(&sql);

        let output = cmd.output().await
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to execute mysql: {}", e)))?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        Ok(())
    }

    async fn plan_create_user(&self, username: DbUser, password_ref: SecretString) -> Result<CreateUserPlan> {
        validate_identifier(&username.0)?;
        Ok(CreateUserPlan { username, password_ref })
    }

    async fn apply_create_user(&self, plan: &CreateUserPlan) -> Result<()> {
        let escaped_pwd = plan.password_ref.0.replace("\\", "\\\\").replace("'", "''");
        let sql = format!("CREATE USER IF NOT EXISTS '{}'@'%' IDENTIFIED BY '{}';", plan.username.0, escaped_pwd);

        let mut cmd = self.build_cmd("mysql");
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to spawn mysql: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(sql.as_bytes()).await
                .map_err(|e| DatabaseError::OperationFailed(format!("Failed to write to stdin: {}", e)))?;
        }

        let output = child.wait_with_output().await
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to wait for mysql: {}", e)))?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        Ok(())
    }

    async fn plan_grant(&self, db_name: DbName, username: DbUser, privileges: Vec<String>) -> Result<GrantPlan> {
        validate_identifier(&db_name.0)?;
        validate_identifier(&username.0)?;
        for priv_str in &privileges {
            validate_privilege(priv_str)?;
        }
        Ok(GrantPlan { db_name, username, privileges })
    }

    async fn apply_grant(&self, plan: &GrantPlan) -> Result<()> {
        let privs = if plan.privileges.is_empty() { "ALL PRIVILEGES".to_string() } else { plan.privileges.join(", ") };
        let sql = format!("GRANT {} ON `{}`.* TO '{}'@'%';", privs, plan.db_name.0, plan.username.0);

        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-e").arg(&sql);

        let output = cmd.output().await
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to execute mysql: {}", e)))?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        Ok(())
    }

    async fn plan_dump(&self, db_name: DbName, output_path: String) -> Result<DumpPlan> {
        validate_identifier(&db_name.0)?;
        Ok(DumpPlan { db_name, output_path })
    }

    async fn apply_dump(&self, plan: &DumpPlan) -> Result<()> {
        let mut cmd = self.build_cmd("mysqldump");
        cmd.arg(&plan.db_name.0);

        let file = std::fs::File::create(&plan.output_path)
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to create dump file: {}", e)))?;

        cmd.stdout(Stdio::from(file));

        let output = cmd.output().await
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to execute mysqldump: {}", e)))?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        Ok(())
    }

    async fn plan_restore(&self, db_name: DbName, input_path: String) -> Result<RestorePlan> {
        validate_identifier(&db_name.0)?;
        Ok(RestorePlan { db_name, input_path })
    }

    async fn apply_restore(&self, plan: &RestorePlan) -> Result<()> {
        let mut cmd = self.build_cmd("mysql");
        cmd.arg(&plan.db_name.0);

        let file = std::fs::File::open(&plan.input_path)
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to open restore file: {}", e)))?;

        cmd.stdin(Stdio::from(file));

        let output = cmd.output().await
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to execute mysql restore: {}", e)))?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        Ok(())
    }
}
