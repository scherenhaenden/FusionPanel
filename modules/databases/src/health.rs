#![forbid(unsafe_code)]

use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use std::time::Instant;

// We use #[path] in our tests file or other modules to include this, so we don't modify lib.rs.
use crate::adapter::{DatabaseConfig, DatabaseEngine, ConnectionStrategy};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub latency_ms: Option<u128>,
    pub active_connections: Option<u32>,
    pub version: Option<String>,
    pub error: Option<String>,
}

pub struct DatabaseHealthChecker {
    config: DatabaseConfig,
    timeout: Duration,
}

impl DatabaseHealthChecker {
    pub fn new(config: DatabaseConfig) -> Self {
        Self {
            config,
            timeout: Duration::from_secs(2), // default 2s
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn check(&self) -> HealthStatus {
        let result = tokio::time::timeout(self.timeout, self.perform_check()).await;

        match result {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => HealthStatus {
                is_healthy: false,
                latency_ms: None,
                active_connections: None,
                version: None,
                error: Some(e),
            },
            Err(_) => HealthStatus {
                is_healthy: false,
                latency_ms: None,
                active_connections: None,
                version: None,
                error: Some("Connection timeout".to_string()),
            },
        }
    }

    async fn perform_check(&self) -> Result<HealthStatus, String> {
        let start = Instant::now();

        let (version, connections) = match self.config.engine {
            DatabaseEngine::PostgreSQL => self.check_postgres().await?,
            DatabaseEngine::MariaDB => self.check_mariadb().await?,
        };

        let latency = start.elapsed().as_millis();

        Ok(HealthStatus {
            is_healthy: true,
            latency_ms: Some(latency),
            active_connections: Some(connections),
            version: Some(version),
            error: None,
        })
    }

    fn build_cmd(&self, program: &str, is_postgres: bool) -> Command {
        let mut cmd = match &self.config.strategy {
            ConnectionStrategy::Host { .. } => {
                let mut c = Command::new(program);
                if is_postgres {
                    c.env("PGPASSWORD", &self.config.admin_password_ref.0);
                } else if !self.config.admin_password_ref.0.is_empty() {
                    c.env("MYSQL_PWD", &self.config.admin_password_ref.0);
                }
                c
            }
            ConnectionStrategy::Container { container_name, .. } => {
                let mut c = Command::new("docker");

                // When using docker exec, environment variables passed to `Command::new("docker")`
                // stay in the host context. We must explicitly forward them to the container
                // using `docker exec -e KEY=VALUE`.
                if is_postgres {
                    c.args(["exec", "-i", "-e", &format!("PGPASSWORD={}", self.config.admin_password_ref.0), container_name, program]);
                } else if !self.config.admin_password_ref.0.is_empty() {
                    c.args(["exec", "-i", "-e", &format!("MYSQL_PWD={}", self.config.admin_password_ref.0), container_name, program]);
                } else {
                    c.args(["exec", "-i", container_name, program]);
                }
                c
            }
        };
        // Important to kill the process if the future drops (e.g. timeout) to avoid process leaks
        cmd.kill_on_drop(true);
        cmd
    }

    async fn check_postgres(&self) -> Result<(String, u32), String> {
        let mut cmd = self.build_cmd("psql", true);
        cmd.args([
            "-h", &self.config.host,
            "-p", &self.config.port.to_string(),
            "-U", &self.config.admin_user,
            "-d", "postgres",
            "-t",
            "-c", "SELECT version(), (SELECT count(*) FROM pg_stat_activity);",
        ]);

        let output = cmd.output().await.map_err(|e| e.to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Command failed: {}", stderr));
        }

        let out = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = out.split('|').map(|s| s.trim()).collect();

        if parts.len() < 2 {
            return Err("Unexpected output format".to_string());
        }

        let version = parts[0].to_string();
        let connections = parts[1].parse::<u32>().map_err(|_| "Failed to parse connection count")?;

        Ok((version, connections))
    }

    async fn check_mariadb(&self) -> Result<(String, u32), String> {
        let mut cmd = self.build_cmd("mysql", false);
        cmd.args([
            "-h", &self.config.host,
            "-P", &self.config.port.to_string(),
            "-u", &self.config.admin_user,
            "-e", "SELECT VERSION(); SHOW STATUS WHERE `variable_name` = 'Threads_connected';",
            "--batch",
            "--skip-column-names",
        ]);

        let output = cmd.output().await.map_err(|e| e.to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Command failed: {}", stderr));
        }

        let out = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = out.lines().map(|s| s.trim()).collect();

        if lines.len() < 2 {
            return Err("Unexpected output format".to_string());
        }

        let version = lines[0].to_string();
        let conn_parts: Vec<&str> = lines[1].split_whitespace().collect();

        if conn_parts.len() < 2 {
            return Err("Unexpected connection count format".to_string());
        }

        let connections = conn_parts[1].parse::<u32>().map_err(|_| "Failed to parse connection count")?;

        Ok((version, connections))
    }
}
