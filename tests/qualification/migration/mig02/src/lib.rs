// Copyright 2024 FusionPanel
// License: AGPL-3.0-only
#![forbid(unsafe_code)]

use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Unsupported version or schema fingerprint mismatch: {0}")]
    UnsupportedVersion(String),
    #[error("Dangling foreign keys detected")]
    DanglingForeignKeys,
    #[error("Multiple databases per site detected")]
    MultipleDatabasesPerSite,
    #[error("Ambiguous settings detected")]
    AmbiguousSettings,
}

pub fn discover_cloudpanel_sqlite(conn: &Connection) -> Result<(), DiscoveryError> {
    // Check for dangling foreign keys
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    if rows.next()?.is_some() {
        return Err(DiscoveryError::DanglingForeignKeys);
    }

    // Verify schema fingerprint
    let required_tables = vec!["site", "php_settings", "database", "database_user"];

    for table in &required_tables {
        let sql = format!("PRAGMA table_info('{}')", table);
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        if rows.next()?.is_none() {
            return Err(DiscoveryError::UnsupportedVersion(format!("Missing table: {}", table)));
        }
    }

    // Check specific columns in site
    let mut stmt = conn.prepare("PRAGMA table_info('site')")?;
    let mut rows = stmt.query([])?;
    let mut has_domain = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "domain_name" {
            has_domain = true;
        }
    }
    if !has_domain {
        return Err(DiscoveryError::UnsupportedVersion("Missing column 'domain_name' in 'site'".to_string()));
    }

    // Check specific columns in database
    let mut stmt = conn.prepare("PRAGMA table_info('database')")?;
    let mut rows = stmt.query([])?;
    let mut has_db_name = false;
    let mut has_site_id = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "database_name" {
            has_db_name = true;
        } else if name == "site_id" {
            has_site_id = true;
        }
    }
    if !has_db_name {
        return Err(DiscoveryError::UnsupportedVersion("Missing column 'database_name' in 'database'".to_string()));
    }
    if !has_site_id {
        return Err(DiscoveryError::UnsupportedVersion("Missing column 'site_id' in 'database'".to_string()));
    }

    // Check specific columns in php_settings
    let mut stmt = conn.prepare("PRAGMA table_info('php_settings')")?;
    let mut rows = stmt.query([])?;
    let mut has_setting_site_id = false;
    let mut has_setting_name = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "site_id" {
            has_setting_site_id = true;
        } else if name == "setting_name" {
            has_setting_name = true;
        }
    }
    if !has_setting_site_id {
        return Err(DiscoveryError::UnsupportedVersion("Missing column 'site_id' in 'php_settings'".to_string()));
    }
    if !has_setting_name {
        return Err(DiscoveryError::UnsupportedVersion("Missing column 'setting_name' in 'php_settings'".to_string()));
    }

    // Check for multiple databases per site
    let mut stmt = conn.prepare("SELECT site_id, COUNT(*) FROM \"database\" GROUP BY site_id HAVING COUNT(*) > 1")?;
    let mut rows = stmt.query([])?;
    if rows.next()?.is_some() {
        return Err(DiscoveryError::MultipleDatabasesPerSite);
    }

    // Check for ambiguous settings
    let mut stmt = conn.prepare("SELECT site_id, setting_name, COUNT(*) FROM php_settings GROUP BY site_id, setting_name HAVING COUNT(*) > 1")?;
    let mut rows = stmt.query([])?;
    if rows.next()?.is_some() {
        return Err(DiscoveryError::AmbiguousSettings);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Result as SqliteResult;

    fn setup_valid_schema(conn: &Connection) -> SqliteResult<()> {
        conn.execute(
            "CREATE TABLE site (
                id INTEGER PRIMARY KEY,
                domain_name TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE php_settings (
                id INTEGER PRIMARY KEY,
                site_id INTEGER NOT NULL,
                setting_name TEXT NOT NULL,
                setting_value TEXT,
                FOREIGN KEY(site_id) REFERENCES site(id)
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE \"database\" (
                id INTEGER PRIMARY KEY,
                site_id INTEGER NOT NULL,
                database_name TEXT NOT NULL,
                FOREIGN KEY(site_id) REFERENCES site(id)
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE database_user (
                id INTEGER PRIMARY KEY,
                database_id INTEGER NOT NULL,
                username TEXT NOT NULL,
                FOREIGN KEY(database_id) REFERENCES \"database\"(id)
            )",
            [],
        )?;
        Ok(())
    }

    #[test]
    fn test_valid_schema() -> SqliteResult<()> {
        let conn = Connection::open_in_memory()?;
        setup_valid_schema(&conn)?;
        assert!(discover_cloudpanel_sqlite(&conn).is_ok());
        Ok(())
    }

    #[test]
    fn test_missing_table_returns_unsupported_version() -> SqliteResult<()> {
        let conn = Connection::open_in_memory()?;
        conn.execute("CREATE TABLE site (id INTEGER PRIMARY KEY, domain_name TEXT)", [])?;
        conn.execute("CREATE TABLE php_settings (id INTEGER PRIMARY KEY, site_id INTEGER, setting_name TEXT, setting_value TEXT)", [])?;
        conn.execute("CREATE TABLE \"database\" (id INTEGER PRIMARY KEY, site_id INTEGER, database_name TEXT)", [])?;
        // Missing database_user table

        let result = discover_cloudpanel_sqlite(&conn);
        assert!(matches!(result, Err(DiscoveryError::UnsupportedVersion(msg)) if msg.contains("database_user")));
        Ok(())
    }

    #[test]
    fn test_missing_column_returns_unsupported_version() -> SqliteResult<()> {
        let conn = Connection::open_in_memory()?;
        // Missing domain_name
        conn.execute("CREATE TABLE site (id INTEGER PRIMARY KEY)", [])?;
        conn.execute("CREATE TABLE php_settings (id INTEGER PRIMARY KEY, site_id INTEGER, setting_name TEXT, setting_value TEXT)", [])?;
        conn.execute("CREATE TABLE \"database\" (id INTEGER PRIMARY KEY, site_id INTEGER, database_name TEXT)", [])?;
        conn.execute("CREATE TABLE database_user (id INTEGER PRIMARY KEY, database_id INTEGER, username TEXT)", [])?;

        let result = discover_cloudpanel_sqlite(&conn);
        assert!(matches!(result, Err(DiscoveryError::UnsupportedVersion(msg)) if msg.contains("domain_name")));
        Ok(())
    }

    #[test]
    fn test_renamed_column_returns_unsupported_version() -> SqliteResult<()> {
        let conn = Connection::open_in_memory()?;
        conn.execute("CREATE TABLE site (id INTEGER PRIMARY KEY, domain TEXT)", [])?;
        conn.execute("CREATE TABLE php_settings (id INTEGER PRIMARY KEY, site_id INTEGER, setting_name TEXT, setting_value TEXT)", [])?;
        conn.execute("CREATE TABLE \"database\" (id INTEGER PRIMARY KEY, site_id INTEGER, database_name TEXT)", [])?;
        conn.execute("CREATE TABLE database_user (id INTEGER PRIMARY KEY, database_id INTEGER, username TEXT)", [])?;

        let result = discover_cloudpanel_sqlite(&conn);
        assert!(matches!(result, Err(DiscoveryError::UnsupportedVersion(msg)) if msg.contains("domain_name")));
        Ok(())
    }

    #[test]
    fn test_dangling_foreign_key() -> SqliteResult<()> {
        let conn = Connection::open_in_memory()?;
        setup_valid_schema(&conn)?;

        // Insert a dangling foreign key (site_id 999 does not exist)
        conn.execute("INSERT INTO \"database\" (id, site_id, database_name) VALUES (1, 999, 'test_db')", [])?;

        let result = discover_cloudpanel_sqlite(&conn);
        assert!(matches!(result, Err(DiscoveryError::DanglingForeignKeys)));
        Ok(())
    }

    #[test]
    fn test_multiple_databases_per_site() -> SqliteResult<()> {
        let conn = Connection::open_in_memory()?;
        setup_valid_schema(&conn)?;

        conn.execute("INSERT INTO site (id, domain_name) VALUES (1, 'example.com')", [])?;
        conn.execute("INSERT INTO \"database\" (id, site_id, database_name) VALUES (1, 1, 'db1')", [])?;
        conn.execute("INSERT INTO \"database\" (id, site_id, database_name) VALUES (2, 1, 'db2')", [])?;

        let result = discover_cloudpanel_sqlite(&conn);
        assert!(matches!(result, Err(DiscoveryError::MultipleDatabasesPerSite)));
        Ok(())
    }

    #[test]
    fn test_ambiguous_settings() -> SqliteResult<()> {
        let conn = Connection::open_in_memory()?;
        setup_valid_schema(&conn)?;

        conn.execute("INSERT INTO site (id, domain_name) VALUES (1, 'example.com')", [])?;
        conn.execute("INSERT INTO php_settings (id, site_id, setting_name, setting_value) VALUES (1, 1, 'memory_limit', '128M')", [])?;
        conn.execute("INSERT INTO php_settings (id, site_id, setting_name, setting_value) VALUES (2, 1, 'memory_limit', '256M')", [])?;

        let result = discover_cloudpanel_sqlite(&conn);
        assert!(matches!(result, Err(DiscoveryError::AmbiguousSettings)));
        Ok(())
    }
}
