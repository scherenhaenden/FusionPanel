use migration::discovery::{CloudPanelDiscovery, DiscoveryError};
use rusqlite::Connection;
use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        for attempt in 0..100u32 {
            let path = std::env::temp_dir().join(format!(
                "fusionpanel-migration-e2e-{}-{nonce}-{attempt}.sqlite",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    drop(file);
                    return Self(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create isolated SQLite fixture: {error}"),
            }
        }
        panic!("could not allocate a unique SQLite fixture path")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Negative integration case: a candidate-looking SQLite layout must not be
/// advertised as a supported CloudPanel source version until a sanitized,
/// version-matched source export qualifies it.
#[test]
fn unqualified_sqlite_schema_is_rejected_without_source_mutation() {
    let db = TempDb::new();
    let conn = Connection::open(&db.0).unwrap();
    conn.execute_batch(
        "CREATE TABLE site(id INTEGER PRIMARY KEY,domain_name TEXT,user TEXT,type TEXT,root_directory TEXT,reverse_proxy_url TEXT);
         CREATE TABLE php_settings(id INTEGER PRIMARY KEY,site_id INTEGER,php_version TEXT);
         CREATE TABLE database_server(id INTEGER PRIMARY KEY,engine TEXT,version TEXT);
         CREATE TABLE database(id INTEGER PRIMARY KEY,name TEXT,site_id INTEGER,database_server_id INTEGER);
         CREATE TABLE database_user(id INTEGER PRIMARY KEY,database_id INTEGER,user_name TEXT);
         INSERT INTO site VALUES(1,'app.example.test','site-owner','reverse-proxy','/external/site','https://127.0.0.1:8443');
         INSERT INTO database_server VALUES(1,'mariadb','10.11');
         INSERT INTO database VALUES(1,'one',1,1);
         INSERT INTO database VALUES(2,'two',1,1);
         INSERT INTO database_user VALUES(1,1,'db-user-a');
         INSERT INTO database_user VALUES(2,1,'db-user-b');",
    )
    .unwrap();
    drop(conn);

    let before = fs::read(&db.0).unwrap();
    let result = CloudPanelDiscovery::discover_with_version(&db.0, "2.5.4");
    assert!(
        matches!(result, Err(DiscoveryError::UnsupportedVersion(version)) if version == "2.5.4")
    );
    assert_eq!(
        before,
        fs::read(&db.0).unwrap(),
        "discovery must leave the source export untouched"
    );
}

/// True source-server E2E qualification. It remains ignored in ordinary test
/// runs, and its explicit qualification command must fail if the source export
/// is absent or its exact version has no registered, verified schema adapter.
///
/// Required qualification command:
/// `cargo test --manifest-path modules/migration/Cargo.toml --test e2e -- --ignored`
#[test]
#[ignore = "BLOCKED: requires sanitized version-matched CloudPanel source export and a qualified schema adapter"]
fn sanitized_cloudpanel_export_qualification() {
    let path = std::env::var_os("FUSIONPANEL_CLOUDPANEL_DB")
        .expect("RESULT=BLOCKED; missing FUSIONPANEL_CLOUDPANEL_DB");
    let version = std::env::var("FUSIONPANEL_CLOUDPANEL_VERSION")
        .expect("RESULT=BLOCKED; missing FUSIONPANEL_CLOUDPANEL_VERSION");
    let inventory = CloudPanelDiscovery::discover_with_version(path, &version)
        .expect("source export must match an explicitly qualified versioned schema profile");
    assert!(
        !inventory.items.is_empty(),
        "source inventory cannot be silently empty"
    );
}
