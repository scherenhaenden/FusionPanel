#![forbid(unsafe_code)]

use mail::domain::{EmailAddress, MailQuota, MailboxAccount, PasswordHash};
use network_services::mail_worker::{MailWorker, MailWorkerError};
use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn test_init_db() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("mail.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .connect(&db_url)
        .await
        .expect("Failed to connect");

    let worker = MailWorker::new(
        pool.clone(),
        dir.path().join("domains"),
        dir.path().join("mailboxes"),
        dir.path().join("aliases"),
    );

    worker.init_db().await.expect("Failed to init DB");

    // Verify tables exist
    let row: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('users', 'aliases')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, 2);
}

#[tokio::test]
async fn test_sync_accounts() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("mail.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .connect(&db_url)
        .await
        .expect("Failed to connect");

    let worker = MailWorker::new(
        pool.clone(),
        dir.path().join("domains"),
        dir.path().join("mailboxes"),
        dir.path().join("aliases"),
    );

    worker.init_db().await.unwrap();

    let acc1 = MailboxAccount::new(
        Uuid::new_v4(),
        EmailAddress::new("alice@example.com").unwrap(),
        PasswordHash::new("$argon2id$v=19$m=4096,t=3,p=1$abc$def").unwrap(),
        MailQuota::new(1024).unwrap(),
    );

    let res = worker.sync_accounts(&[acc1]).await;
    match res {
        Err(MailWorkerError::CommandFailed(msg)) if msg.contains("BLOCKED") => {
            // expected in test without postmap
        }
        other => panic!("Expected BLOCKED error, got {:?}", other),
    }

    let domains_content = fs::read_to_string(dir.path().join("domains")).unwrap_or_default();
    assert_eq!(domains_content, "");

    let mailboxes_content = fs::read_to_string(dir.path().join("mailboxes")).unwrap_or_default();
    assert_eq!(mailboxes_content, "");

    // Verify DB
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn test_sync_aliases() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("mail.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .connect(&db_url)
        .await
        .expect("Failed to connect");

    let worker = MailWorker::new(
        pool.clone(),
        dir.path().join("domains"),
        dir.path().join("mailboxes"),
        dir.path().join("aliases"),
    );

    worker.init_db().await.unwrap();

    let aliases = vec![("postmaster@example.com", "alice@example.com")];
    let res = worker.sync_aliases(&aliases).await;

    match res {
        Err(MailWorkerError::CommandFailed(msg)) if msg.contains("BLOCKED") => {
            // expected
        }
        other => panic!("Expected BLOCKED error, got {:?}", other),
    }

    let aliases_content = fs::read_to_string(dir.path().join("aliases")).unwrap_or_default();
    assert_eq!(aliases_content, "");

    // Verify DB
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM aliases")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn test_reload_daemons_blocked() {
    let dir = tempdir().unwrap();
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    let worker = MailWorker::new(
        pool,
        dir.path().join("domains"),
        dir.path().join("mailboxes"),
        dir.path().join("aliases"),
    );

    let res = worker.reload_daemons().await;
    match res {
        Err(MailWorkerError::CommandFailed(msg)) => {
            assert!(msg.contains("BLOCKED"));
        }
        other => panic!("Expected BLOCKED error, got {:?}", other),
    }
}
