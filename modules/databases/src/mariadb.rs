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

    async fn database_exists(&self, name: &DbName) -> Result<bool> {
        validate_identifier(&name.0)?;
        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-N").arg("-B").arg("-e").arg(format!(
            "SELECT 1 FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = '{}';",
            name.0
        ));
        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute mysql: {}", e))
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
        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-N").arg("-B").arg("-e").arg(
            "SELECT SCHEMA_NAME, DEFAULT_CHARACTER_SET_NAME, DEFAULT_COLLATION_NAME FROM information_schema.SCHEMATA \
             WHERE SCHEMA_NAME NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys');"
        );
        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute mysql: {}", e))
        })?;
        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let out_str = String::from_utf8_lossy(&output.stdout);
        let mut dbs = Vec::new();
        for line in out_str.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
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
        validate_identifier(&username.0)?;
        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-N").arg("-B").arg("-e").arg(format!(
            "SELECT 1 FROM mysql.user WHERE User = '{}';",
            username.0
        ));
        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute mysql: {}", e))
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
        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-e").arg("SELECT 1;");

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::ConnectionFailed(format!("Failed to execute mysql: {}", e))
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
            "CREATE DATABASE `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;",
            plan.db_name.0
        );

        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-e").arg(&sql);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute mysql: {}", e))
        })?;

        if !output.status.success() {
            return Err(DatabaseError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
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
        let escaped_pwd = plan.password_ref.0.replace("\\", "\\\\").replace("'", "''");
        let sql = format!(
            "CREATE USER IF NOT EXISTS '{}'@'%' IDENTIFIED BY '{}';",
            plan.username.0, escaped_pwd
        );

        let mut cmd = self.build_cmd("mysql");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to spawn mysql: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(sql.as_bytes()).await.map_err(|e| {
                DatabaseError::OperationFailed(format!("Failed to write to stdin: {}", e))
            })?;
        }

        let output = child.wait_with_output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to wait for mysql: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let safe_err = stderr
                .replace(&plan.password_ref.0, "[REDACTED]")
                .replace(&escaped_pwd, "[REDACTED]");
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
            "ALL PRIVILEGES".to_string()
        } else {
            plan.privileges.join(", ")
        };
        let sql = format!(
            "GRANT {} ON `{}`.* TO '{}'@'%';",
            privs, plan.db_name.0, plan.username.0
        );

        let mut cmd = self.build_cmd("mysql");
        cmd.arg("-e").arg(&sql);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute mysql: {}", e))
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
        let mut cmd = self.build_cmd("mysqldump");
        cmd.arg(&plan.db_name.0);

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
            DatabaseError::OperationFailed(format!("Failed to execute mysqldump: {}", e))
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
        let mut cmd = self.build_cmd("mysql");
        cmd.arg(&plan.db_name.0);

        let file = std::fs::File::open(&plan.input_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to open restore file: {}", e))
        })?;

        cmd.stdin(Stdio::from(file));

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute mysql restore: {}", e))
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

        let sql = format!("DROP DATABASE `{}`;", name.0);

        let mut cmd = self.build_cmd("mysql");
        cmd.args(["-e", &sql]);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute mysql for drop db: {}", e))
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

        // Drop both localhost and wildcard since we created them that way
        let sql = format!(
            "DROP USER IF EXISTS '{}'@'localhost', '{}'@'%';",
            username.0, username.0
        );

        let mut cmd = self.build_cmd("mysql");
        cmd.args(["-e", &sql]);

        let output = cmd.output().await.map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to execute mysql for drop user: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(DatabaseError::OperationFailed(stderr));
        }
        Ok(())
    }
}
