use crate::inventory::{InventoryItem, InventoryItemType, MigrationInventory};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Unknown version: {0}")]
    VersionUnknown(String),
    #[error("Schema missing: {0}")]
    SchemaMissing(String),
    #[error("Unsupported schema: {0}")]
    UnsupportedSchema(String),
}

pub struct CloudPanelDiscovery;

impl CloudPanelDiscovery {
    pub fn discover<P: AsRef<Path>>(db_path: P) -> Result<MigrationInventory, DiscoveryError> {
        // Open read-only to guarantee no mutations during discovery
        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

        let version = Self::discover_version(&conn)?;
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), version.clone());

        // Validate supported versions. Explicitly unsupported for versions other than 2.4.x or 2.3.x, for example.
        if !version.starts_with("2.4") && !version.starts_with("2.3") {
             return Err(DiscoveryError::UnsupportedSchema(format!("Version {} is not explicitly supported.", version)));
        }

        Self::discover_sites(&conn, &mut inventory)?;
        Self::discover_databases(&conn, &mut inventory)?;
        Self::discover_users(&conn, &mut inventory)?;

        Ok(inventory)
    }

    fn discover_version(conn: &Connection) -> Result<String, DiscoveryError> {
        let mut stmt = conn.prepare("SELECT value FROM setting WHERE name = 'version'")
            .or_else(|_| conn.prepare("SELECT version FROM app_settings"))
            .map_err(|e| DiscoveryError::SchemaMissing(format!("No version table found: {}", e)))?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let version: String = row.get(0)?;
            Ok(version)
        } else {
            Err(DiscoveryError::VersionUnknown("Version row not found".to_string()))
        }
    }

    fn discover_sites(conn: &Connection, inventory: &mut MigrationInventory) -> Result<(), DiscoveryError> {
        let mut stmt = match conn.prepare("SELECT id, domain_name, php_version FROM site") {
            Ok(stmt) => stmt,
            Err(e) => {
                return Err(DiscoveryError::SchemaMissing(format!("site table error: {}", e)));
            }
        };

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let domain_name: String = row.get(1)?;
            let php_version: Option<String> = row.get(2).unwrap_or(None);

            let mut metadata = std::collections::HashMap::new();
            if let Some(php) = &php_version {
                metadata.insert("php_version".to_string(), php.clone());
            }

            // Find database associations for this site
            let associations = Self::discover_site_database_associations(conn, id)?;

            inventory.items.push(InventoryItem {
                id: format!("site_{}", id),
                item_type: InventoryItemType::Site,
                name: domain_name,
                version: php_version,
                metadata,
                associations,
                ownership: None,
                aliases: Vec::new(),
                schedules: Vec::new(),
                variants: Vec::new(),
                user_mappings: std::collections::HashMap::new(),
            });
        }
        Ok(())
    }

    fn discover_site_database_associations(conn: &Connection, site_id: i64) -> Result<Vec<String>, DiscoveryError> {
        // Assume CloudPanel schema maps site_id to database in a relation table or on the database itself
        // Example: SELECT id FROM database WHERE site_id = ?
        let mut stmt = match conn.prepare("SELECT id FROM database WHERE site_id = ?") {
            Ok(stmt) => stmt,
            Err(_) => {
                // If table is missing, return empty or error. Assuming schema demands it for strictness
                return Err(DiscoveryError::SchemaMissing("database table missing site_id relation".to_string()));
            }
        };

        let mut rows = stmt.query([site_id])?;
        let mut associations = Vec::new();
        while let Some(row) = rows.next()? {
            let db_id: i64 = row.get(0)?;
            associations.push(format!("db_{}", db_id));
        }
        Ok(associations)
    }

    fn discover_databases(conn: &Connection, inventory: &mut MigrationInventory) -> Result<(), DiscoveryError> {
        let mut stmt = match conn.prepare("SELECT id, database_name, site_id FROM database") {
            Ok(stmt) => stmt,
            Err(e) => {
                return Err(DiscoveryError::SchemaMissing(format!("database table error: {}", e)));
            }
        };

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let site_id: Option<i64> = row.get(2).unwrap_or(None);

            let mut associations = Vec::new();
            if let Some(s_id) = site_id {
                associations.push(format!("site_{}", s_id));
            }

            inventory.items.push(InventoryItem {
                id: format!("db_{}", id),
                item_type: InventoryItemType::Database,
                name,
                version: None,
                metadata: std::collections::HashMap::new(),
                associations,
                ownership: None,
                aliases: Vec::new(),
                schedules: Vec::new(),
                variants: Vec::new(),
                user_mappings: std::collections::HashMap::new(),
            });
        }
        Ok(())
    }

    fn discover_users(conn: &Connection, inventory: &mut MigrationInventory) -> Result<(), DiscoveryError> {
        let mut stmt = match conn.prepare("SELECT id, user_name, role FROM user") {
            Ok(stmt) => stmt,
            Err(e) => {
                return Err(DiscoveryError::SchemaMissing(format!("user table error: {}", e)));
            }
        };

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let role: String = row.get(2).unwrap_or_else(|_| "user".to_string());

            let mut metadata = std::collections::HashMap::new();
            metadata.insert("role".to_string(), role);

            inventory.items.push(InventoryItem {
                id: format!("user_{}", id),
                item_type: InventoryItemType::User,
                name,
                version: None,
                metadata,
                associations: Vec::new(),
                ownership: None,
                aliases: Vec::new(),
                schedules: Vec::new(),
                variants: Vec::new(),
                user_mappings: std::collections::HashMap::new(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    #[test]
    fn test_discovery_reads_from_sqlite() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        // Setup fixture
        {
            let conn = Connection::open(db_path).unwrap();
            conn.execute("CREATE TABLE setting (name TEXT PRIMARY KEY, value TEXT)", []).unwrap();
            conn.execute("INSERT INTO setting (name, value) VALUES ('version', '2.4.1')", []).unwrap();

            conn.execute("CREATE TABLE site (id INTEGER PRIMARY KEY, domain_name TEXT, php_version TEXT)", []).unwrap();
            conn.execute("INSERT INTO site (id, domain_name, php_version) VALUES (1, 'mysite.com', '8.2')", []).unwrap();

            conn.execute("CREATE TABLE database (id INTEGER PRIMARY KEY, database_name TEXT, site_id INTEGER)", []).unwrap();
            // Two databases attached to the same site
            conn.execute("INSERT INTO database (id, database_name, site_id) VALUES (10, 'my_db_1', 1)", []).unwrap();
            conn.execute("INSERT INTO database (id, database_name, site_id) VALUES (11, 'my_db_2', 1)", []).unwrap();

            conn.execute("CREATE TABLE user (id INTEGER PRIMARY KEY, user_name TEXT, role TEXT)", []).unwrap();
            conn.execute("INSERT INTO user (id, user_name, role) VALUES (100, 'admin_user', 'admin')", []).unwrap();
        }

        // Run discovery
        let inventory = CloudPanelDiscovery::discover(db_path).unwrap();

        assert_eq!(inventory.source_panel, "CloudPanel");
        assert_eq!(inventory.source_version, "2.4.1");

        let site = inventory.items.iter().find(|i| i.item_type == InventoryItemType::Site).unwrap();
        assert_eq!(site.name, "mysite.com");
        assert_eq!(site.version, Some("8.2".to_string()));
        assert_eq!(site.associations.len(), 2);
        assert!(site.associations.contains(&"db_10".to_string()));
        assert!(site.associations.contains(&"db_11".to_string()));

        let db1 = inventory.items.iter().find(|i| i.item_type == InventoryItemType::Database && i.id == "db_10").unwrap();
        assert_eq!(db1.name, "my_db_1");
        assert_eq!(db1.associations, vec!["site_1".to_string()]);

        let db2 = inventory.items.iter().find(|i| i.item_type == InventoryItemType::Database && i.id == "db_11").unwrap();
        assert_eq!(db2.name, "my_db_2");
        assert_eq!(db2.associations, vec!["site_1".to_string()]);

        let user = inventory.items.iter().find(|i| i.item_type == InventoryItemType::User).unwrap();
        assert_eq!(user.name, "admin_user");
        assert_eq!(user.metadata.get("role").unwrap(), "admin");
    }

    #[test]
    fn test_discovery_schema_missing_error() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        // Setup fixture
        {
            let conn = Connection::open(db_path).unwrap();
            conn.execute("CREATE TABLE dummy (id INTEGER PRIMARY KEY)", []).unwrap();
        }

        let result = CloudPanelDiscovery::discover(db_path);
        assert!(matches!(result, Err(DiscoveryError::SchemaMissing(_))));
    }

    #[test]
    fn test_discovery_unsupported_schema_version() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        // Setup fixture
        {
            let conn = Connection::open(db_path).unwrap();
            conn.execute("CREATE TABLE setting (name TEXT PRIMARY KEY, value TEXT)", []).unwrap();
            conn.execute("INSERT INTO setting (name, value) VALUES ('version', '1.0.0')", []).unwrap(); // Unsupported
        }

        let result = CloudPanelDiscovery::discover(db_path);
        assert!(matches!(result, Err(DiscoveryError::UnsupportedSchema(_))));
    }
}
