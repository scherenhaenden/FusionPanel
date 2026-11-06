use mail::domain::{EmailAddress, MailQuota, MailboxAccount, PasswordHash};
use network_services::mail_worker::{MailWorker, MailWorkerError};
use sqlx::sqlite::SqlitePoolOptions;
use std::process::Stdio;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use uuid::Uuid;

#[tokio::test]
async fn test_real_disposable_mail_integration() {
    let dir = std::sync::Arc::new(tempdir().unwrap());

    let has_docker = Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_docker {
        panic!("BLOCKED: missing runtime dependency for real disposable MTA integration test (needs docker)");
    }

    let config_dir = dir.path().join("config");
    let postfix_dir = config_dir.join("postfix");
    let dovecot_dir = config_dir.join("dovecot");
    std::fs::create_dir_all(&postfix_dir).unwrap();
    std::fs::create_dir_all(&dovecot_dir).unwrap();

    // Create actual empty files so postmap won't complain if run blindly
    std::fs::write(postfix_dir.join("domains"), "").unwrap();
    std::fs::write(postfix_dir.join("mailboxes"), "").unwrap();
    std::fs::write(postfix_dir.join("aliases"), "").unwrap();
    std::fs::write(postfix_dir.join("domains.db"), "").unwrap();
    std::fs::write(postfix_dir.join("mailboxes.db"), "").unwrap();
    std::fs::write(postfix_dir.join("aliases.db"), "").unwrap();

    let db_path = dir.path().join("mail.db");

    std::fs::write(
        dovecot_dir.join("10-auth.conf"),
        "auth_mechanisms = plain login\n!include auth-sql.conf.ext",
    )
    .unwrap();
    let dovecot_sql_conf = "driver = sqlite\nconnect = /var/mail/mail.db\ndefault_pass_scheme = ARGON2ID\npassword_query = SELECT password FROM users WHERE username = '%n' AND domain = '%d'\nuser_query = SELECT maildir as home, 5000 as uid, 5000 as gid FROM users WHERE username = '%n' AND domain = '%d'";
    std::fs::write(dovecot_dir.join("dovecot-sql.conf.ext"), dovecot_sql_conf).unwrap();
    std::fs::write(dovecot_dir.join("auth-sql.conf.ext"), "passdb {\n  driver = sql\n  args = /etc/dovecot/dovecot-sql.conf.ext\n}\nuserdb {\n  driver = sql\n  args = /etc/dovecot/dovecot-sql.conf.ext\n}").unwrap();

    let postfix_mount = format!("{}:/etc/postfix/fusionpanel", postfix_dir.display());

    // Single file mounts are fine for Dovecot as we don't rename them at runtime
    let auth_conf_mount = format!(
        "{}:/etc/dovecot/conf.d/10-auth.conf",
        dovecot_dir.join("10-auth.conf").display()
    );
    let sql_conf_ext_mount = format!(
        "{}:/etc/dovecot/auth-sql.conf.ext",
        dovecot_dir.join("auth-sql.conf.ext").display()
    );
    let sql_conf_mount = format!(
        "{}:/etc/dovecot/dovecot-sql.conf.ext",
        dovecot_dir.join("dovecot-sql.conf.ext").display()
    );
    let db_mount = format!("{}:/var/mail/mail.db", db_path.display());

    let docker_run = Command::new("docker")
        .arg("run")
        .arg("-d")
        .arg("--rm")
        .arg("-p")
        .arg("2525:25")
        .arg("-p")
        .arg("1143:143")
        .arg("-v")
        .arg(&postfix_mount)
        .arg("-v")
        .arg(&auth_conf_mount)
        .arg("-v")
        .arg(&sql_conf_ext_mount)
        .arg("-v")
        .arg(&sql_conf_mount)
        .arg("-v")
        .arg(&db_mount)
        .arg("--name")
        .arg("fusionpanel-test-mail-stack")
        .arg("mailserver/docker-mailserver:latest")
        .output()
        .await;

    let container_started = docker_run.is_ok() && docker_run.unwrap().status.success();
    if !container_started {
        panic!("BLOCKED: Docker daemon overlayfs mount restricted / invalid argument in sandbox");
    }

    async fn run_test(
        dir: std::sync::Arc<tempfile::TempDir>,
        postfix_dir: std::path::PathBuf,
        db_path: std::path::PathBuf,
    ) {
        tokio::time::sleep(Duration::from_secs(10)).await;

        // Configure postfix inside the container
        let _ = Command::new("docker")
            .args([
                "exec",
                "fusionpanel-test-mail-stack",
                "postconf",
                "-e",
                "virtual_mailbox_domains=hash:/etc/postfix/fusionpanel/domains",
            ])
            .output()
            .await;
        let _ = Command::new("docker")
            .args([
                "exec",
                "fusionpanel-test-mail-stack",
                "postconf",
                "-e",
                "virtual_mailbox_maps=hash:/etc/postfix/fusionpanel/mailboxes",
            ])
            .output()
            .await;
        let _ = Command::new("docker")
            .args([
                "exec",
                "fusionpanel-test-mail-stack",
                "postconf",
                "-e",
                "virtual_alias_maps=hash:/etc/postfix/fusionpanel/aliases",
            ])
            .output()
            .await;

        let bin_dir = dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        let postmap_path = bin_dir.join("postmap");
        // Convert host path to container path for postmap
        let host_postfix_dir_str = postfix_dir.display().to_string();
        std::fs::write(&postmap_path, format!("#!/bin/sh\nARG=$(echo \"$1\" | sed \"s|{}|/etc/postfix/fusionpanel|g\")\ndocker exec fusionpanel-test-mail-stack postmap \"$ARG\"", host_postfix_dir_str)).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&postmap_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let doveadm_path = bin_dir.join("doveadm");
        std::fs::write(
            &doveadm_path,
            "#!/bin/sh\ndocker exec fusionpanel-test-mail-stack doveadm \"$@\"",
        )
        .unwrap();
        std::fs::set_permissions(&doveadm_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let postfix_path = bin_dir.join("postfix");
        std::fs::write(
            &postfix_path,
            "#!/bin/sh\ndocker exec fusionpanel-test-mail-stack postfix \"$@\"",
        )
        .unwrap();
        std::fs::set_permissions(&postfix_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let old_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bin_dir.display(), old_path);
        std::env::set_var("PATH", &new_path);

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        let pool = SqlitePoolOptions::new()
            .connect(&db_url)
            .await
            .expect("Failed to connect");

        let worker = MailWorker::new(
            pool.clone(),
            postfix_dir.join("domains"),
            postfix_dir.join("mailboxes"),
            postfix_dir.join("aliases"),
        );

        worker.init_db().await.unwrap();

        let acc1 = MailboxAccount::new(
            Uuid::new_v4(),
            EmailAddress::new("admin@test.local").unwrap(),
            PasswordHash::new("$argon2id$v=19$m=4096,t=3,p=1$c29tZXNhbHQ$P/eF6lVf0i7U9yE8B3A=")
                .unwrap(),
            MailQuota::new(1024).unwrap(),
        );

        let res = worker.sync_accounts(&[acc1]).await;

        let aliases = vec![("catchall@test.local", "admin@test.local")];
        let alias_res = worker.sync_aliases(&aliases).await;

        std::env::set_var("PATH", old_path);

        let bad_worker = MailWorker::new(
            pool.clone(),
            postfix_dir.join("domains"),
            postfix_dir.join("mailboxes"),
            postfix_dir.join("nonexistent_alias_path"),
        );
        let recovery_res = bad_worker.sync_aliases(&aliases).await;
        assert!(
            recovery_res.is_err(),
            "Recovery scenario failed to error out"
        );

        if res.is_ok() && alias_res.is_ok() {
            let smtp_res = TcpStream::connect("127.0.0.1:2525").await;
            if let Ok(mut stream) = smtp_res {
                let mut buf = [0; 1024];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                assert!(String::from_utf8_lossy(&buf[..n]).contains("220"));

                stream.write_all(b"EHLO test.local\r\n").await.unwrap();
                let n = stream.read(&mut buf).await.unwrap_or(0);
                assert!(String::from_utf8_lossy(&buf[..n]).contains("250"));

                stream
                    .write_all(b"MAIL FROM:<sender@example.com>\r\n")
                    .await
                    .unwrap();
                let n = stream.read(&mut buf).await.unwrap_or(0);
                assert!(String::from_utf8_lossy(&buf[..n]).contains("250"));

                stream
                    .write_all(b"RCPT TO:<catchall@test.local>\r\n")
                    .await
                    .unwrap();
                let n = stream.read(&mut buf).await.unwrap_or(0);
                assert!(String::from_utf8_lossy(&buf[..n]).contains("250"));

                stream.write_all(b"DATA\r\n").await.unwrap();
                let n = stream.read(&mut buf).await.unwrap_or(0);
                assert!(String::from_utf8_lossy(&buf[..n]).contains("354"));

                let large_body = "x".repeat(2048);
                stream
                    .write_all(format!("Subject: Test\r\n\r\n{}\r\n.\r\n", large_body).as_bytes())
                    .await
                    .unwrap();
                let n = stream.read(&mut buf).await.unwrap_or(0);

                let response = String::from_utf8_lossy(&buf[..n]);
                assert!(
                    response.contains("552") || response.contains("quota"),
                    "SMTP Data response: {}",
                    response
                );
            } else {
                panic!("Failed to connect to SMTP");
            }

            let imap_res = TcpStream::connect("127.0.0.1:1143").await;
            if let Ok(mut stream) = imap_res {
                let mut buf = [0; 1024];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                assert!(String::from_utf8_lossy(&buf[..n]).contains("* OK"));

                stream
                    .write_all(b"a1 LOGIN admin@test.local password\r\n")
                    .await
                    .unwrap();
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let response = String::from_utf8_lossy(&buf[..n]);

                assert!(
                    response.contains("a1 OK"),
                    "IMAP Login response: {}",
                    response
                );

                stream.write_all(b"a2 SELECT INBOX\r\n").await.unwrap();
                let n = stream.read(&mut buf).await.unwrap_or(0);
                assert!(String::from_utf8_lossy(&buf[..n]).contains("OK"));
            } else {
                panic!("Failed to connect to IMAP");
            }
        }

        match res {
            Ok(_) => {}
            Err(e) => {
                if let MailWorkerError::CommandFailed(msg) = &e {
                    if msg.contains("BLOCKED") {
                        panic!("BLOCKED: {}", msg);
                    }
                }
                panic!("Expected sync to succeed but it failed: {:?}", e);
            }
        }
    }

    // Execute test code and guarantee docker container cleanup even on panic
    let result = tokio::spawn(run_test(dir.clone(), postfix_dir, db_path)).await;

    let _ = Command::new("docker")
        .args(["stop", "fusionpanel-test-mail-stack"])
        .output()
        .await;

    if let Err(err) = result {
        if err.is_panic() {
            std::panic::resume_unwind(err.into_panic());
        }
    }
}
