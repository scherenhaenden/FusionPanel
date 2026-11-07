use mail::domain::MailboxAccount;
use sqlx::SqlitePool;
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum MailWorkerError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Postfix render error: {0}")]
    PostfixRender(String),
    #[error("Command execution failed: {0}")]
    CommandFailed(String),
}

pub struct MailWorker {
    db: SqlitePool,
    postfix_virtual_domains_path: PathBuf,
    postfix_virtual_mailboxes_path: PathBuf,
    postfix_virtual_aliases_path: PathBuf,
}

impl MailWorker {
    pub fn new(
        db: SqlitePool,
        postfix_virtual_domains_path: PathBuf,
        postfix_virtual_mailboxes_path: PathBuf,
        postfix_virtual_aliases_path: PathBuf,
    ) -> Self {
        Self {
            db,
            postfix_virtual_domains_path,
            postfix_virtual_mailboxes_path,
            postfix_virtual_aliases_path,
        }
    }

    pub async fn init_db(&self) -> Result<(), MailWorkerError> {
        let mut tx = self.db.begin().await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (
                username TEXT NOT NULL,
                domain TEXT NOT NULL,
                password TEXT NOT NULL,
                maildir TEXT NOT NULL,
                quota INTEGER NOT NULL,
                UNIQUE(username, domain)
            )",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS aliases (
                source TEXT NOT NULL,
                destination TEXT NOT NULL,
                UNIQUE(source, destination)
            )",
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn sync_accounts(&self, accounts: &[MailboxAccount]) -> Result<(), MailWorkerError> {
        let mut tx = self.db.begin().await?;

        let mut domains: Vec<String> = accounts
            .iter()
            .map(|a| a.email.as_string().split('@').nth(1).unwrap().to_string())
            .collect();
        domains.sort();
        domains.dedup();
        let domains_str: Vec<&str> = domains.iter().map(|d| d.as_str()).collect();

        let domains_config = mail::config::postfix::render_virtual_mailbox_domains(&domains_str)
            .map_err(|e| MailWorkerError::PostfixRender(format!("{:?}", e)))?;

        let mut mailboxes: Vec<(&str, &str)> = Vec::new();
        let mut all_usernames = Vec::new();
        let mut all_domains = Vec::new();
        for acc in accounts {
            let email = acc.email.as_string();
            let parts: Vec<&str> = email.split('@').collect();
            all_usernames.push(parts[0].to_string());
            all_domains.push(parts[1].to_string());
        }

        for i in 0..accounts.len() {
            mailboxes.push((&all_usernames[i], &all_domains[i]));
        }

        let maps_config = mail::config::postfix::render_virtual_mailbox_maps(&mailboxes)
            .map_err(|e| MailWorkerError::PostfixRender(format!("{:?}", e)))?;

        let temp_domains_path = self.postfix_virtual_domains_path.with_extension("tmp");
        let temp_mailboxes_path = self.postfix_virtual_mailboxes_path.with_extension("tmp");

        fs::write(&temp_domains_path, domains_config).await?;
        fs::write(&temp_mailboxes_path, maps_config).await?;

        if let Err(e) = self.run_postmap(&temp_domains_path).await {
            let _ = fs::remove_file(&temp_domains_path).await;
            let _ = fs::remove_file(&temp_mailboxes_path).await;
            let _ = tx.rollback().await;
            return Err(e);
        }
        if let Err(e) = self.run_postmap(&temp_mailboxes_path).await {
            let _ = fs::remove_file(&temp_domains_path).await;
            let temp_domains_db = PathBuf::from(format!("{}.db", temp_domains_path.display()));
            let _ = fs::remove_file(&temp_domains_db).await;
            let _ = fs::remove_file(&temp_mailboxes_path).await;
            let _ = tx.rollback().await;
            return Err(e);
        }

        sqlx::query("DELETE FROM users").execute(&mut *tx).await?;

        for acc in accounts {
            let email = acc.email.as_string();
            let parts: Vec<&str> = email.split('@').collect();
            let username = parts[0];
            let domain = parts[1];
            let maildir = format!("{}/{}/", domain, username);

            sqlx::query(
                "INSERT INTO users (username, domain, password, maildir, quota) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(username)
            .bind(domain)
            .bind(acc.password_hash.as_str())
            .bind(maildir)
            .bind(acc.quota.bytes() as i64)
            .execute(&mut *tx)
            .await?;
        }

        let temp_domains_db = PathBuf::from(format!("{}.db", temp_domains_path.display()));
        let temp_mailboxes_db = PathBuf::from(format!("{}.db", temp_mailboxes_path.display()));
        let domains_db = PathBuf::from(format!(
            "{}.db",
            self.postfix_virtual_domains_path.display()
        ));
        let mailboxes_db = PathBuf::from(format!(
            "{}.db",
            self.postfix_virtual_mailboxes_path.display()
        ));

        let bak_domains = PathBuf::from(format!(
            "{}.bak",
            self.postfix_virtual_domains_path.display()
        ));
        let bak_domains_db = PathBuf::from(format!(
            "{}.bak.db",
            self.postfix_virtual_domains_path.display()
        ));
        let bak_mailboxes = PathBuf::from(format!(
            "{}.bak",
            self.postfix_virtual_mailboxes_path.display()
        ));
        let bak_mailboxes_db = PathBuf::from(format!(
            "{}.bak.db",
            self.postfix_virtual_mailboxes_path.display()
        ));

        let domains_existed = self.postfix_virtual_domains_path.exists();
        let domains_db_existed = domains_db.exists();
        let mailboxes_existed = self.postfix_virtual_mailboxes_path.exists();
        let mailboxes_db_existed = mailboxes_db.exists();

        if domains_existed {
            let _ = fs::copy(&self.postfix_virtual_domains_path, &bak_domains).await;
        }
        if domains_db_existed {
            let _ = fs::copy(&domains_db, &bak_domains_db).await;
        }
        if mailboxes_existed {
            let _ = fs::copy(&self.postfix_virtual_mailboxes_path, &bak_mailboxes).await;
        }
        if mailboxes_db_existed {
            let _ = fs::copy(&mailboxes_db, &bak_mailboxes_db).await;
        }

        let r1 = fs::rename(&temp_domains_path, &self.postfix_virtual_domains_path).await;
        let r2 = fs::rename(&temp_domains_db, &domains_db).await;
        let r3 = fs::rename(&temp_mailboxes_path, &self.postfix_virtual_mailboxes_path).await;
        let r4 = fs::rename(&temp_mailboxes_db, &mailboxes_db).await;

        if r1.is_err() || r2.is_err() || r3.is_err() || r4.is_err() {
            if domains_existed {
                let _ = fs::rename(&bak_domains, &self.postfix_virtual_domains_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_domains_path).await;
            }
            if domains_db_existed {
                let _ = fs::rename(&bak_domains_db, &domains_db).await;
            } else {
                let _ = fs::remove_file(&domains_db).await;
            }
            if mailboxes_existed {
                let _ = fs::rename(&bak_mailboxes, &self.postfix_virtual_mailboxes_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_mailboxes_path).await;
            }
            if mailboxes_db_existed {
                let _ = fs::rename(&bak_mailboxes_db, &mailboxes_db).await;
            } else {
                let _ = fs::remove_file(&mailboxes_db).await;
            }
            let _ = tx.rollback().await;
            return Err(MailWorkerError::CommandFailed(
                "Failed to rename configuration files into place".to_string(),
            ));
        }

        if let Err(e) = self.reload_daemons().await {
            if domains_existed {
                let _ = fs::rename(&bak_domains, &self.postfix_virtual_domains_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_domains_path).await;
            }
            if domains_db_existed {
                let _ = fs::rename(&bak_domains_db, &domains_db).await;
            } else {
                let _ = fs::remove_file(&domains_db).await;
            }
            if mailboxes_existed {
                let _ = fs::rename(&bak_mailboxes, &self.postfix_virtual_mailboxes_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_mailboxes_path).await;
            }
            if mailboxes_db_existed {
                let _ = fs::rename(&bak_mailboxes_db, &mailboxes_db).await;
            } else {
                let _ = fs::remove_file(&mailboxes_db).await;
            }

            let _ = tx.rollback().await;
            return Err(e);
        }

        if let Err(e) = tx.commit().await {
            if domains_existed {
                let _ = fs::rename(&bak_domains, &self.postfix_virtual_domains_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_domains_path).await;
            }
            if domains_db_existed {
                let _ = fs::rename(&bak_domains_db, &domains_db).await;
            } else {
                let _ = fs::remove_file(&domains_db).await;
            }
            if mailboxes_existed {
                let _ = fs::rename(&bak_mailboxes, &self.postfix_virtual_mailboxes_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_mailboxes_path).await;
            }
            if mailboxes_db_existed {
                let _ = fs::rename(&bak_mailboxes_db, &mailboxes_db).await;
            } else {
                let _ = fs::remove_file(&mailboxes_db).await;
            }
            let _ = self.reload_daemons().await;
            return Err(e.into());
        }

        let _ = fs::remove_file(&bak_domains).await;
        let _ = fs::remove_file(&bak_domains_db).await;
        let _ = fs::remove_file(&bak_mailboxes).await;
        let _ = fs::remove_file(&bak_mailboxes_db).await;

        Ok(())
    }

    pub async fn sync_aliases(&self, aliases: &[(&str, &str)]) -> Result<(), MailWorkerError> {
        let mut tx = self.db.begin().await?;

        let mut config = String::new();
        for (source, dest) in aliases {
            config.push_str(&format!("{} {}\n", source, dest));
        }

        let temp_aliases_path = self.postfix_virtual_aliases_path.with_extension("tmp");
        fs::write(&temp_aliases_path, config).await?;

        if let Err(e) = self.run_postmap(&temp_aliases_path).await {
            let _ = fs::remove_file(&temp_aliases_path).await;
            let _ = tx.rollback().await;
            return Err(e);
        }

        sqlx::query("DELETE FROM aliases").execute(&mut *tx).await?;

        for (source, dest) in aliases {
            sqlx::query("INSERT INTO aliases (source, destination) VALUES (?, ?)")
                .bind(*source)
                .bind(*dest)
                .execute(&mut *tx)
                .await?;
        }

        let temp_aliases_db = PathBuf::from(format!("{}.db", temp_aliases_path.display()));
        let aliases_db = PathBuf::from(format!(
            "{}.db",
            self.postfix_virtual_aliases_path.display()
        ));

        let bak_aliases = PathBuf::from(format!(
            "{}.bak",
            self.postfix_virtual_aliases_path.display()
        ));
        let bak_aliases_db = PathBuf::from(format!(
            "{}.bak.db",
            self.postfix_virtual_aliases_path.display()
        ));

        let aliases_existed = self.postfix_virtual_aliases_path.exists();
        let aliases_db_existed = aliases_db.exists();

        if aliases_existed {
            let _ = fs::copy(&self.postfix_virtual_aliases_path, &bak_aliases).await;
        }
        if aliases_db_existed {
            let _ = fs::copy(&aliases_db, &bak_aliases_db).await;
        }

        let r1 = fs::rename(&temp_aliases_path, &self.postfix_virtual_aliases_path).await;
        let r2 = fs::rename(&temp_aliases_db, &aliases_db).await;

        if r1.is_err() || r2.is_err() {
            if aliases_existed {
                let _ = fs::rename(&bak_aliases, &self.postfix_virtual_aliases_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_aliases_path).await;
            }
            if aliases_db_existed {
                let _ = fs::rename(&bak_aliases_db, &aliases_db).await;
            } else {
                let _ = fs::remove_file(&aliases_db).await;
            }
            let _ = tx.rollback().await;
            return Err(MailWorkerError::CommandFailed(
                "Failed to rename configuration files into place".to_string(),
            ));
        }

        if let Err(e) = self.reload_daemons().await {
            if aliases_existed {
                let _ = fs::rename(&bak_aliases, &self.postfix_virtual_aliases_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_aliases_path).await;
            }
            if aliases_db_existed {
                let _ = fs::rename(&bak_aliases_db, &aliases_db).await;
            } else {
                let _ = fs::remove_file(&aliases_db).await;
            }

            let _ = tx.rollback().await;
            return Err(e);
        }

        if let Err(e) = tx.commit().await {
            if aliases_existed {
                let _ = fs::rename(&bak_aliases, &self.postfix_virtual_aliases_path).await;
            } else {
                let _ = fs::remove_file(&self.postfix_virtual_aliases_path).await;
            }
            if aliases_db_existed {
                let _ = fs::rename(&bak_aliases_db, &aliases_db).await;
            } else {
                let _ = fs::remove_file(&aliases_db).await;
            }
            let _ = self.reload_daemons().await;
            return Err(e.into());
        }

        let _ = fs::remove_file(&bak_aliases).await;
        let _ = fs::remove_file(&bak_aliases_db).await;

        Ok(())
    }

    async fn run_postmap(&self, path: &std::path::Path) -> Result<(), MailWorkerError> {
        let mut cmd = Command::new("postmap");
        cmd.arg(path);
        cmd.kill_on_drop(true);

        let output = cmd.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                MailWorkerError::CommandFailed(format!("BLOCKED: postmap not found: {}", e))
            } else {
                MailWorkerError::Io(e)
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MailWorkerError::CommandFailed(format!(
                "postmap failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    pub async fn reload_daemons(&self) -> Result<(), MailWorkerError> {
        let mut dovecot = Command::new("doveadm");
        dovecot.arg("reload");
        dovecot.kill_on_drop(true);

        let output = dovecot.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                MailWorkerError::CommandFailed(format!("BLOCKED: doveadm not found: {}", e))
            } else {
                MailWorkerError::Io(e)
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MailWorkerError::CommandFailed(format!(
                "doveadm reload failed: {}",
                stderr
            )));
        }

        let mut postfix = Command::new("postfix");
        postfix.arg("reload");
        postfix.kill_on_drop(true);

        let output = postfix.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                MailWorkerError::CommandFailed(format!("BLOCKED: postfix not found: {}", e))
            } else {
                MailWorkerError::Io(e)
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MailWorkerError::CommandFailed(format!(
                "postfix reload failed: {}",
                stderr
            )));
        }

        Ok(())
    }
}
