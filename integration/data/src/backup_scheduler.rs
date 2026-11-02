#![forbid(unsafe_code)]

use chrono::Utc;
use cron::Schedule;
use sha2::{Digest, Sha256};

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Zstd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseType {
    Mysql,
    Postgres,
}

pub struct DumpScheduler {
    schedule: Schedule,
    db_type: DatabaseType,
    output_dir: PathBuf,
    compression: Compression,
}

impl DumpScheduler {
    pub fn new(
        cron_expr: &str,
        db_type: DatabaseType,
        output_dir: PathBuf,
        compression: Compression,
    ) -> Result<Self, cron::error::Error> {
        let schedule = Schedule::from_str(cron_expr)?;
        Ok(Self {
            schedule,
            db_type,
            output_dir,
            compression,
        })
    }

    pub fn next_run(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.schedule.upcoming(Utc).next()
    }

    pub fn build_dump_command(&self, password_env_value: &str) -> Command {
        let (cmd_str, env_key) = match self.db_type {
            DatabaseType::Mysql => ("mysqldump", "MYSQL_PWD"),
            DatabaseType::Postgres => ("pg_dump", "PGPASSWORD"),
        };

        let mut cmd = Command::new(cmd_str);

        // Securely pass the password via environment variables
        cmd.env(env_key, password_env_value);

        match self.db_type {
            DatabaseType::Mysql => {
                cmd.args(["--opt", "--all-databases"]);
            }
            DatabaseType::Postgres => {
                cmd.args(["-Fc", "-f", "-"]);
            }
        }

        cmd.stdout(Stdio::piped());
        cmd
    }

    pub fn execute_dump(
        &self,
        filename: &str,
        password_env_value: &str,
    ) -> io::Result<(PathBuf, String)> {
        let output_path = self.output_dir.join(filename);
        let file = File::create(&output_path)?;

        let mut cmd = self.build_dump_command(password_env_value);
        let mut child = cmd.spawn()?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Could not capture stdout"))?;

        let mut hasher = Sha256::new();

        match self.compression {
            Compression::Gzip => {
                let mut encoder =
                    flate2::write::GzEncoder::new(file, flate2::Compression::default());
                let mut buffer = [0u8; 8192];
                loop {
                    let n = std::io::Read::read(&mut stdout, &mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                    encoder.write_all(&buffer[..n])?;
                }
                encoder.finish()?;
            }
            Compression::Zstd => {
                let mut encoder = zstd::stream::Encoder::new(file, 0)?;
                let mut buffer = [0u8; 8192];
                loop {
                    let n = std::io::Read::read(&mut stdout, &mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                    encoder.write_all(&buffer[..n])?;
                }
                encoder.finish()?;
            }
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::other(format!("Dump command failed: {}", status)));
        }

        let hash_result = format!("{:x}", hasher.finalize());
        Ok((output_path, hash_result))
    }
}
#[allow(dead_code)]
fn _dummy() {
    let _ = DatabaseType::Mysql;
    let _ = DatabaseType::Postgres;
    let _ = Compression::Gzip;
    let _ = Compression::Zstd;
}
