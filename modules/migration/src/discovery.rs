use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use thiserror::Error;

use crate::inventory::{InventoryItem, InventoryItemType, MigrationInventory};

#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Missing expected table or column: {0}")]
    SchemaMissing(String),
    #[error("Failed to determine version: {0}")]
    VersionUnknown(String),
}

/// Discovers CloudPanel version and inventory by reading its SQLite database (read-only).
pub struct CloudPanelDiscovery;

impl CloudPanelDiscovery {
    pub fn discover<P: AsRef<Path>>(db_path: P) -> Result<MigrationInventory, DiscoveryError> {
        // Open read-only to guarantee no mutations during discovery
        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

        let version = Self::discover_version(&conn)?;
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), version);

        Self::discover_sites(&conn, &mut inventory)?;
        Self::discover_databases(&conn, &mut inventory)?;
        Self::discover_users(&conn, &mut inventory)?;

        Ok(inventory)
    }

    fn discover_version(conn: &Connection) -> Result<String, DiscoveryError> {
        // Typically, version might be in a settings or config table.
        // We'll attempt a common pattern or fallback to "unknown".
        // Example: SELECT value FROM setting WHERE name = 'version' OR SELECT version FROM system_info
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
                // Return gracefully or error if this is a strict requirement
                // For this example, we'll error out if `site` table doesn't exist.
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

            inventory.items.push(InventoryItem {
                id: format!("site_{}", id),
                item_type: InventoryItemType::Site,
                name: domain_name,
                version: php_version,
                metadata,
            });
        }
        Ok(())
    }

    fn discover_databases(conn: &Connection, inventory: &mut MigrationInventory) -> Result<(), DiscoveryError> {
        // Attempt to query databases
        let mut stmt = match conn.prepare("SELECT id, database_name FROM database") {
            Ok(stmt) => stmt,
            Err(_) => {
                // Database table might not exist or might be named differently, this is okay for a basic check, we'll skip.
                return Ok(());
            }
        };

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;

            inventory.items.push(InventoryItem {
                id: format!("db_{}", id),
                item_type: InventoryItemType::Database,
                name,
                version: None,
                metadata: std::collections::HashMap::new(),
            });
        }
        Ok(())
    }

    fn discover_users(conn: &Connection, inventory: &mut MigrationInventory) -> Result<(), DiscoveryError> {
        let mut stmt = match conn.prepare("SELECT id, user_name, role FROM user") {
            Ok(stmt) => stmt,
            Err(_) => return Ok(()),
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
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;

    #[test]
    fn test_discovery_reads_from_sqlite() {
        let db_path = "test_discovery.sqlite";
        let _ = fs::remove_file(db_path);

        // Setup fixture
        {
            let conn = Connection::open(db_path).unwrap();
            conn.execute(
                "CREATE TABLE setting (name TEXT PRIMARY KEY, value TEXT)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO setting (name, value) VALUES ('version', '2.4.1')",
                [],
            ).unwrap();

            conn.execute(
                "CREATE TABLE site (id INTEGER PRIMARY KEY, domain_name TEXT, php_version TEXT)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO site (id, domain_name, php_version) VALUES (1, 'mysite.com', '8.2')",
                [],
            ).unwrap();

            conn.execute(
                "CREATE TABLE database (id INTEGER PRIMARY KEY, database_name TEXT)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO database (id, database_name) VALUES (10, 'my_db')",
                [],
            ).unwrap();

            conn.execute(
                "CREATE TABLE user (id INTEGER PRIMARY KEY, user_name TEXT, role TEXT)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO user (id, user_name, role) VALUES (100, 'admin_user', 'admin')",
                [],
            ).unwrap();
        }

        // Run discovery
        let inventory = CloudPanelDiscovery::discover(db_path).unwrap();

        assert_eq!(inventory.source_panel, "CloudPanel");
        assert_eq!(inventory.source_version, "2.4.1");

        let site = inventory.items.iter().find(|i| i.item_type == InventoryItemType::Site).unwrap();
        assert_eq!(site.name, "mysite.com");
        assert_eq!(site.version, Some("8.2".to_string()));

        let db = inventory.items.iter().find(|i| i.item_type == InventoryItemType::Database).unwrap();
        assert_eq!(db.name, "my_db");

        let user = inventory.items.iter().find(|i| i.item_type == InventoryItemType::User).unwrap();
        assert_eq!(user.name, "admin_user");
        assert_eq!(user.metadata.get("role").unwrap(), "admin");

        // Cleanup
        fs::remove_file(db_path).unwrap();
    }

    #[test]
    fn test_discovery_schema_missing_error() {
        let db_path = "test_missing_schema.sqlite";
        let _ = fs::remove_file(db_path);

        // Setup fixture
        {
            let conn = Connection::open(db_path).unwrap();
            // Create a table, but not the expected setting table for version
            conn.execute("CREATE TABLE dummy (id INTEGER PRIMARY KEY)", []).unwrap();
        }

        let result = CloudPanelDiscovery::discover(db_path);
        assert!(matches!(result, Err(DiscoveryError::SchemaMissing(_))));

        // Cleanup
        fs::remove_file(db_path).unwrap();
    }
}
