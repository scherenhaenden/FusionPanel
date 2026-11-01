use rusqlite::{Connection, OpenFlags};
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};
use thiserror::Error;

use crate::inventory::{InventoryItem, InventoryItemType, MigrationInventory};

const PROFILE_CANDIDATE_FIXTURE: &str = "unqualified-candidate-site-db-fixture";
// No CloudPanel source release is qualified yet. The candidate table layout
// below is exercised only as a synthetic SQLite fixture until a sanitized,
// version-matched export verifies the schema. Do not add a version here based
// on a guessed table fingerprint alone.
const QUALIFIED_SOURCE_VERSIONS: &[&str] = &[];
const REQUIRED_TABLES: &[(&str, &[&str])] = &[
    (
        "site",
        &[
            "id",
            "domain_name",
            "user",
            "type",
            "root_directory",
            "reverse_proxy_url",
        ],
    ),
    ("php_settings", &["id", "site_id", "php_version"]),
    ("database", &["id", "name", "site_id", "database_server_id"]),
    ("database_server", &["id", "engine", "version"]),
    ("database_user", &["id", "database_id", "user_name"]),
];

#[derive(Error, Debug, PartialEq, Eq)]
pub enum DiscoveryError {
    #[error("database error: {0}")]
    Database(String),
    #[error("unsupported or unknown CloudPanel version `{0}`; pass a verified version supported by an explicit schema profile")]
    UnsupportedVersion(String),
    #[error("required schema table or column is missing: {0}")]
    SchemaMissing(String),
    #[error("schema contains ambiguous associations: {0}")]
    AmbiguousAssociation(String),
    #[error("invalid source version `{0}`")]
    InvalidVersion(String),
    #[error("source database contains a dangling association: {0}")]
    DanglingAssociation(String),
}

pub struct CloudPanelDiscovery;

impl CloudPanelDiscovery {
    /// Deliberately refuses to infer a product version from guessed settings
    /// tables. Call `discover_with_version` with a version obtained from the
    /// source package/CLI.
    pub fn discover<P: AsRef<Path>>(_db_path: P) -> Result<MigrationInventory, DiscoveryError> {
        Err(DiscoveryError::UnsupportedVersion("not supplied".into()))
    }

    pub fn discover_with_version<P: AsRef<Path>>(
        db_path: P,
        source_version: &str,
    ) -> Result<MigrationInventory, DiscoveryError> {
        parse_version(source_version)?;
        if !QUALIFIED_SOURCE_VERSIONS.contains(&source_version) {
            return Err(DiscoveryError::UnsupportedVersion(source_version.into()));
        }

        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| DiscoveryError::Database(e.to_string()))?;
        Self::validate_schema(&conn)?;
        let mut inventory = MigrationInventory::new(
            "CloudPanel".into(),
            source_version.into(),
            PROFILE_CANDIDATE_FIXTURE.into(),
        );
        Self::discover_sites(&conn, &mut inventory)?;
        Self::discover_site_users(&mut inventory);
        Self::discover_databases(&conn, &mut inventory)?;
        Self::discover_database_users(&conn, &mut inventory)?;
        Ok(inventory)
    }

    /// Exercise the candidate table layout against real SQLite in tests. This
    /// is deliberately not a production compatibility claim.
    #[cfg(test)]
    fn discover_fixture_layout<P: AsRef<Path>>(
        db_path: P,
        source_version: &str,
    ) -> Result<MigrationInventory, DiscoveryError> {
        parse_version(source_version)?;
        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| DiscoveryError::Database(e.to_string()))?;
        Self::validate_schema(&conn)?;
        let mut inventory = MigrationInventory::new(
            "CloudPanel fixture".into(),
            source_version.into(),
            PROFILE_CANDIDATE_FIXTURE.into(),
        );
        Self::discover_sites(&conn, &mut inventory)?;
        Self::discover_site_users(&mut inventory);
        Self::discover_databases(&conn, &mut inventory)?;
        Self::discover_database_users(&conn, &mut inventory)?;
        Ok(inventory)
    }

    fn validate_schema(conn: &Connection) -> Result<(), DiscoveryError> {
        for (table, required) in REQUIRED_TABLES {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info(\"{}\")", table))
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let columns = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|e| DiscoveryError::Database(e.to_string()))?
                .collect::<Result<BTreeSet<_>, _>>()
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            if columns.is_empty() {
                return Err(DiscoveryError::SchemaMissing(format!("table `{table}`")));
            }
            for column in *required {
                if !columns.contains(*column) {
                    return Err(DiscoveryError::SchemaMissing(format!(
                        "column `{table}.{column}`"
                    )));
                }
            }
        }
        Ok(())
    }

    fn discover_sites(
        conn: &Connection,
        inventory: &mut MigrationInventory,
    ) -> Result<(), DiscoveryError> {
        let mut stmt = conn.prepare(
            "SELECT s.id, s.domain_name, s.user, s.type, s.root_directory, s.reverse_proxy_url, p.php_version \
             FROM site AS s LEFT JOIN php_settings AS p ON p.site_id = s.id ORDER BY s.id, p.id"
        ).map_err(|e| DiscoveryError::Database(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| DiscoveryError::Database(e.to_string()))?;
        let mut seen = BTreeSet::new();
        while let Some(row) = rows
            .next()
            .map_err(|e| DiscoveryError::Database(e.to_string()))?
        {
            let id: i64 = row
                .get(0)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            if !seen.insert(id) {
                return Err(DiscoveryError::AmbiguousAssociation(format!(
                    "site {id} has multiple PHP settings rows"
                )));
            }
            let domain: String = row
                .get(1)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let owner: String = row
                .get(2)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let kind: String = row
                .get(3)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let root: String = row
                .get(4)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let upstream: Option<String> = row
                .get(5)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let php: Option<String> = row
                .get(6)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let mut metadata = HashMap::from([
                ("site_kind".into(), kind),
                ("source_owner".into(), owner.clone()),
                ("source_document_root".into(), root),
            ]);
            if let Some(value) = upstream {
                metadata.insert("source_upstream_url".into(), value);
            }
            if let Some(value) = php.clone() {
                metadata.insert("php_version".into(), value);
            }
            inventory.items.push(InventoryItem {
                id: format!("site_{id}"),
                item_type: InventoryItemType::Site,
                name: domain,
                version: php,
                associations: vec![format!("user_{owner}")],
                metadata,
            });
        }
        Ok(())
    }

    fn discover_site_users(inventory: &mut MigrationInventory) {
        let mut owners = BTreeSet::new();
        for site in inventory
            .items
            .iter()
            .filter(|item| item.item_type == InventoryItemType::Site)
        {
            if let Some(owner) = site.metadata.get("source_owner") {
                owners.insert(owner.clone());
            }
        }
        for owner in owners {
            inventory.items.push(InventoryItem {
                id: format!("user_{owner}"),
                item_type: InventoryItemType::User,
                name: owner.clone(),
                version: None,
                associations: inventory
                    .items
                    .iter()
                    .filter(|item| {
                        item.item_type == InventoryItemType::Site
                            && item.metadata.get("source_owner") == Some(&owner)
                    })
                    .map(|item| item.id.clone())
                    .collect(),
                metadata: HashMap::from([("ownership".into(), "external_or_unknown".into())]),
            });
        }
    }

    fn discover_databases(
        conn: &Connection,
        inventory: &mut MigrationInventory,
    ) -> Result<(), DiscoveryError> {
        let mut stmt = conn.prepare(
            "SELECT d.id, d.name, d.site_id, d.database_server_id, s.engine, s.version \
             FROM database AS d LEFT JOIN database_server AS s ON s.id = d.database_server_id ORDER BY d.id"
        ).map_err(|e| DiscoveryError::Database(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| DiscoveryError::Database(e.to_string()))?;
        while let Some(row) = rows
            .next()
            .map_err(|e| DiscoveryError::Database(e.to_string()))?
        {
            let id: i64 = row
                .get(0)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let name: String = row
                .get(1)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let site_id: i64 = row
                .get(2)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let server_id: i64 = row
                .get(3)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            if !inventory.items.iter().any(|item| {
                item.id == format!("site_{site_id}") && item.item_type == InventoryItemType::Site
            }) {
                return Err(DiscoveryError::DanglingAssociation(format!(
                    "database {id} references missing site {site_id}"
                )));
            }
            let engine: Option<String> = row
                .get(4)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let server_version: Option<String> = row
                .get(5)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let (Some(engine), Some(server_version)) = (engine, server_version) else {
                return Err(DiscoveryError::DanglingAssociation(format!(
                    "database {id} references missing database server {server_id}"
                )));
            };
            inventory.items.push(InventoryItem {
                id: format!("db_{id}"),
                item_type: InventoryItemType::Database,
                name,
                version: Some(server_version.clone()),
                associations: vec![format!("site_{site_id}")],
                metadata: HashMap::from([
                    ("engine".into(), engine),
                    ("database_server_id".into(), server_id.to_string()),
                    ("ownership".into(), "external_or_unknown".into()),
                ]),
            });
        }
        Ok(())
    }

    fn discover_database_users(
        conn: &Connection,
        inventory: &mut MigrationInventory,
    ) -> Result<(), DiscoveryError> {
        let database_ids: BTreeSet<String> = inventory
            .items
            .iter()
            .filter(|item| item.item_type == InventoryItemType::Database)
            .map(|item| item.id.clone())
            .collect();
        let mut stmt = conn
            .prepare("SELECT id, database_id, user_name FROM database_user ORDER BY id")
            .map_err(|e| DiscoveryError::Database(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| DiscoveryError::Database(e.to_string()))?;
        while let Some(row) = rows
            .next()
            .map_err(|e| DiscoveryError::Database(e.to_string()))?
        {
            let id: i64 = row
                .get(0)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let db_id: i64 = row
                .get(1)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            let name: String = row
                .get(2)
                .map_err(|e| DiscoveryError::Database(e.to_string()))?;
            if !database_ids.contains(&format!("db_{db_id}")) {
                return Err(DiscoveryError::DanglingAssociation(format!(
                    "database user {id} references missing database {db_id}"
                )));
            }
            inventory.items.push(InventoryItem {
                id: format!("dbuser_{id}"),
                item_type: InventoryItemType::DatabaseUser,
                name,
                version: None,
                associations: vec![format!("db_{db_id}")],
                metadata: HashMap::from([("ownership".into(), "external_or_unknown".into())]),
            });
        }
        Ok(())
    }
}

fn parse_version(version: &str) -> Result<(u64, u64, u64), DiscoveryError> {
    if version.is_empty() || !version.is_ascii() || version.contains(' ') {
        return Err(DiscoveryError::InvalidVersion(version.into()));
    }
    let mut build_split = version.split('+');
    let without_build = build_split.next().unwrap_or_default();
    let build = build_split.next();
    if build_split.next().is_some() || build.is_some_and(|v| !valid_identifiers(v, false)) {
        return Err(DiscoveryError::InvalidVersion(version.into()));
    }
    let (core, prerelease) = match without_build.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (without_build, None),
    };
    if prerelease.is_some_and(|v| !valid_identifiers(v, true)) {
        return Err(DiscoveryError::InvalidVersion(version.into()));
    }
    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|b| b.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err(DiscoveryError::InvalidVersion(version.into()));
    }
    let parsed = parts
        .iter()
        .map(|part| {
            part.parse::<u64>()
                .map_err(|_| DiscoveryError::InvalidVersion(version.into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((parsed[0], parsed[1], parsed[2]))
}

fn valid_identifiers(value: &str, prerelease: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && (!prerelease
                    || !identifier.bytes().all(|b| b.is_ascii_digit())
                    || identifier == "0"
                    || !identifier.starts_with('0'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    struct TempDatabase(NamedTempFile);
    impl TempDatabase {
        fn create() -> Self {
            Self(NamedTempFile::new().unwrap())
        }
        fn path(&self) -> &Path {
            self.0.path()
        }
        fn setup(&self) -> Connection {
            let conn = Connection::open(self.path()).unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON;
                CREATE TABLE site(id INTEGER PRIMARY KEY,domain_name TEXT NOT NULL,user TEXT NOT NULL,type TEXT NOT NULL,root_directory TEXT NOT NULL,reverse_proxy_url TEXT);
                CREATE TABLE php_settings(id INTEGER PRIMARY KEY,site_id INTEGER NOT NULL,php_version TEXT);
                CREATE TABLE database_server(id INTEGER PRIMARY KEY,engine TEXT NOT NULL,version TEXT NOT NULL);
                CREATE TABLE database(id INTEGER PRIMARY KEY,name TEXT NOT NULL,site_id INTEGER NOT NULL,database_server_id INTEGER NOT NULL);
                CREATE TABLE database_user(id INTEGER PRIMARY KEY,database_id INTEGER NOT NULL,user_name TEXT NOT NULL);
                INSERT INTO site VALUES(1,'app.example.test','app-user','reverse-proxy','/srv/app','https://127.0.0.1:8443');
                INSERT INTO site VALUES(2,'www.example.test','app-user','static','/home/app/site','');
                INSERT INTO php_settings VALUES(1,2,'8.3');
                INSERT INTO database_server VALUES(1,'mariadb','10.11');
                INSERT INTO database VALUES(11,'primary_db',1,1);
                INSERT INTO database VALUES(12,'analytics_db',1,1);
                INSERT INTO database VALUES(13,'secondary_db',2,1);
                INSERT INTO database_user VALUES(21,11,'app-readwrite');
                INSERT INTO database_user VALUES(22,11,'app-readonly');").unwrap();
            conn
        }
    }
    #[test]
    fn refuses_to_guess_version() {
        assert!(matches!(
            CloudPanelDiscovery::discover("unused"),
            Err(DiscoveryError::UnsupportedVersion(_))
        ));
        assert!(matches!(
            CloudPanelDiscovery::discover_with_version("unused", "1.9.0"),
            Err(DiscoveryError::UnsupportedVersion(_))
        ));
        assert!(matches!(
            CloudPanelDiscovery::discover_with_version("unused", "2.5.4-alpha-beta.2+build.5"),
            Err(DiscoveryError::UnsupportedVersion(_))
        ));
        for version in [
            "2.x",
            "2.05.4",
            "2.5.4-01",
            "2.5.4+bad..meta",
            "2.5.4 extra",
        ] {
            assert!(
                matches!(
                    CloudPanelDiscovery::discover_with_version("unused", version),
                    Err(DiscoveryError::InvalidVersion(_))
                ),
                "{version}"
            );
        }
    }

    #[test]
    fn discovers_sites_every_database_and_user_association_read_only() {
        let db = TempDatabase::create();
        drop(db.setup());
        let before = fs::read(db.path()).unwrap();
        let inventory =
            CloudPanelDiscovery::discover_fixture_layout(db.path(), "2.5.4-alpha-beta.2+build.5")
                .unwrap();
        let after = fs::read(db.path()).unwrap();
        assert_eq!(before, after);
        assert_eq!(inventory.schema_profile, PROFILE_CANDIDATE_FIXTURE);
        assert_eq!(
            inventory
                .items
                .iter()
                .filter(|i| i.item_type == InventoryItemType::Database)
                .count(),
            3
        );
        let related = inventory
            .items
            .iter()
            .filter(|i| i.item_type == InventoryItemType::Database && i.associations == ["site_1"])
            .count();
        assert_eq!(related, 2);
        let users = inventory
            .items
            .iter()
            .filter(|i| i.item_type == InventoryItemType::DatabaseUser)
            .collect::<Vec<_>>();
        assert_eq!(users.len(), 2);
        assert!(users.iter().all(|i| i.associations == ["db_11"]));
        let site_users = inventory
            .items
            .iter()
            .filter(|i| i.item_type == InventoryItemType::User)
            .collect::<Vec<_>>();
        assert_eq!(site_users.len(), 1); // hyphenated owner retained, never normalized
        assert_eq!(site_users[0].name, "app-user");
        assert_eq!(site_users[0].metadata["ownership"], "external_or_unknown");
        let proxy = inventory.items.iter().find(|i| i.id == "site_1").unwrap();
        assert_eq!(proxy.metadata["site_kind"], "reverse-proxy");
        assert_eq!(
            proxy.metadata["source_upstream_url"],
            "https://127.0.0.1:8443"
        );
        assert_eq!(proxy.metadata["source_owner"], "app-user");
        assert_eq!(proxy.metadata["source_document_root"], "/srv/app");
    }

    #[test]
    fn missing_database_or_user_subsystem_is_not_silently_ignored() {
        for table in ["database", "database_server", "database_user"] {
            let db = TempDatabase::create();
            let conn = db.setup();
            conn.execute(&format!("DROP TABLE {table}"), []).unwrap();
            drop(conn);
            assert!(
                matches!(CloudPanelDiscovery::discover_fixture_layout(db.path(), "2.5.4"), Err(DiscoveryError::SchemaMissing(s)) if s.contains(table)),
                "missing {table} must not be silently skipped"
            );
        }
    }

    #[test]
    fn version_two_label_does_not_override_schema_fingerprint() {
        let db = TempDatabase::create();
        let conn = Connection::open(db.path()).unwrap();
        conn.execute_batch("CREATE TABLE site(id,domain_name,php_version);")
            .unwrap();
        drop(conn);
        assert!(matches!(
            CloudPanelDiscovery::discover_fixture_layout(db.path(), "2.5.4"),
            Err(DiscoveryError::SchemaMissing(_))
        ));
    }

    #[test]
    fn dangling_site_server_and_database_user_associations_fail_closed() {
        for (sql, expected) in [
            (
                "UPDATE database SET site_id=999 WHERE id=11",
                "missing site",
            ),
            (
                "UPDATE database SET database_server_id=999 WHERE id=11",
                "database server",
            ),
            (
                "UPDATE database_user SET database_id=999 WHERE id=21",
                "missing database",
            ),
        ] {
            let db = TempDatabase::create();
            let conn = db.setup();
            conn.execute(sql, []).unwrap();
            drop(conn);
            let error =
                CloudPanelDiscovery::discover_fixture_layout(db.path(), "2.5.4").unwrap_err();
            assert!(
                matches!(error, DiscoveryError::DanglingAssociation(_)),
                "{error:?}"
            );
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn temporary_sqlite_fixture_is_removed_when_its_owner_drops() {
        let path = {
            let db = TempDatabase::create();
            let path = db.path().to_path_buf();
            drop(db.setup());
            assert!(path.exists());
            drop(db);
            path
        };
        assert!(!path.exists());
    }

    #[test]
    fn duplicate_php_settings_rows_are_reported_as_ambiguous() {
        let db = TempDatabase::create();
        let conn = db.setup();
        conn.execute("INSERT INTO php_settings VALUES(2,2,'8.4')", [])
            .unwrap();
        drop(conn);
        assert!(matches!(
            CloudPanelDiscovery::discover_fixture_layout(db.path(), "2.5.4"),
            Err(DiscoveryError::AmbiguousAssociation(_))
        ));
    }
}
