#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Database {
    pub id: String,
    pub name: String,
    pub owner_site_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseUser {
    pub id: String,
    pub db_id: String,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseGrant {
    pub id: String,
    pub user_id: String,
    pub db_id: String,
    pub privileges: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretReference {
    pub id: String,
    pub db_id: String,
    pub secret_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationConnection {
    pub id: String,
    pub db_id: String,
    pub site_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteExport {
    pub site: Site,
    pub databases: Vec<DatabaseExport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseExport {
    pub database: Database,
    pub users: Vec<DatabaseUser>,
    pub grants: Vec<DatabaseGrant>,
    pub secret_references: Vec<SecretReference>,
    pub application_connections: Vec<ApplicationConnection>,
}

pub struct Registry {
    pub sites: Vec<Site>,
    pub databases: Vec<Database>,
    pub users: Vec<DatabaseUser>,
    pub grants: Vec<DatabaseGrant>,
    pub secrets: Vec<SecretReference>,
    pub connections: Vec<ApplicationConnection>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            sites: vec![],
            databases: vec![],
            users: vec![],
            grants: vec![],
            secrets: vec![],
            connections: vec![],
        }
    }

    pub fn export_site(
        &self,
        site_id: &str,
        simulate_failure_db_id: Option<&str>,
    ) -> Result<SiteExport, String> {
        let site = self
            .sites
            .iter()
            .find(|s| s.id == site_id)
            .ok_or("Site not found")?
            .clone();

        let site_databases: Vec<_> = self
            .databases
            .iter()
            .filter(|d| d.owner_site_id == site_id)
            .cloned()
            .collect();
        let mut database_exports = Vec::new();

        for db in site_databases {
            if let Some(fail_id) = simulate_failure_db_id {
                if fail_id == db.id {
                    return Err(format!("Export failed or omitted for database: {}", db.id));
                }
            }

            let users: Vec<_> = self
                .users
                .iter()
                .filter(|u| u.db_id == db.id)
                .cloned()
                .collect();
            let grants: Vec<_> = self
                .grants
                .iter()
                .filter(|g| g.db_id == db.id)
                .cloned()
                .collect();
            let secrets: Vec<_> = self
                .secrets
                .iter()
                .filter(|s| s.db_id == db.id)
                .cloned()
                .collect();
            let connections: Vec<_> = self
                .connections
                .iter()
                .filter(|c| c.db_id == db.id && c.site_id == site_id)
                .cloned()
                .collect();

            database_exports.push(DatabaseExport {
                database: db,
                users,
                grants,
                secret_references: secrets,
                application_connections: connections,
            });
        }

        Ok(SiteExport {
            site,
            databases: database_exports,
        })
    }

    pub fn restore_site(&mut self, export: SiteExport) -> Result<(), String> {
        self.sites.push(export.site);
        for db_exp in export.databases {
            self.databases.push(db_exp.database);
            self.users.extend(db_exp.users);
            self.grants.extend(db_exp.grants);
            self.secrets.extend(db_exp.secret_references);
            self.connections.extend(db_exp.application_connections);
        }
        Ok(())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preservation_of_multiple_databases() {
        let mut registry = Registry::new();

        let site = Site {
            id: "site-1".to_string(),
            name: "example.com".to_string(),
        };
        registry.sites.push(site.clone());

        let db1 = Database {
            id: "db-1".to_string(),
            name: "db_one".to_string(),
            owner_site_id: "site-1".to_string(),
        };
        registry.databases.push(db1.clone());
        registry.users.push(DatabaseUser {
            id: "u-1".to_string(),
            db_id: "db-1".to_string(),
            username: "user_one".to_string(),
        });
        registry.grants.push(DatabaseGrant {
            id: "g-1".to_string(),
            user_id: "u-1".to_string(),
            db_id: "db-1".to_string(),
            privileges: vec!["ALL".to_string()],
        });
        registry.secrets.push(SecretReference {
            id: "sec-1".to_string(),
            db_id: "db-1".to_string(),
            secret_id: "vault-123".to_string(),
        });
        registry.connections.push(ApplicationConnection {
            id: "conn-1".to_string(),
            db_id: "db-1".to_string(),
            site_id: "site-1".to_string(),
        });

        let db2 = Database {
            id: "db-2".to_string(),
            name: "db_two".to_string(),
            owner_site_id: "site-1".to_string(),
        };
        registry.databases.push(db2.clone());
        registry.users.push(DatabaseUser {
            id: "u-2".to_string(),
            db_id: "db-2".to_string(),
            username: "user_two".to_string(),
        });
        registry.grants.push(DatabaseGrant {
            id: "g-2".to_string(),
            user_id: "u-2".to_string(),
            db_id: "db-2".to_string(),
            privileges: vec!["READ".to_string()],
        });
        registry.secrets.push(SecretReference {
            id: "sec-2".to_string(),
            db_id: "db-2".to_string(),
            secret_id: "vault-456".to_string(),
        });
        registry.connections.push(ApplicationConnection {
            id: "conn-2".to_string(),
            db_id: "db-2".to_string(),
            site_id: "site-1".to_string(),
        });

        let export = registry.export_site("site-1", None).unwrap();

        assert_eq!(export.databases.len(), 2);

        let mut new_registry = Registry::new();
        new_registry.restore_site(export).unwrap();

        assert_eq!(new_registry.sites.len(), 1);
        assert_eq!(new_registry.databases.len(), 2);
        assert_eq!(new_registry.users.len(), 2);
        assert_eq!(new_registry.grants.len(), 2);
        assert_eq!(new_registry.secrets.len(), 2);
        assert_eq!(new_registry.connections.len(), 2);
    }

    #[test]
    fn test_enumeration_handles_all_source_associations() {
        let mut registry = Registry::new();

        registry.sites.push(Site {
            id: "site-1".to_string(),
            name: "example.com".to_string(),
        });

        registry.databases.push(Database {
            id: "db-1".to_string(),
            name: "custom_db_alpha".to_string(),
            owner_site_id: "site-1".to_string(),
        });
        registry.databases.push(Database {
            id: "db-2".to_string(),
            name: "custom_db_beta".to_string(),
            owner_site_id: "site-1".to_string(),
        });
        registry.databases.push(Database {
            id: "db-3".to_string(),
            name: "custom_db_gamma".to_string(),
            owner_site_id: "site-1".to_string(),
        });

        let export = registry.export_site("site-1", None).unwrap();

        assert_eq!(export.databases.len(), 3);
        assert!(export
            .databases
            .iter()
            .any(|d| d.database.name == "custom_db_alpha"));
        assert!(export
            .databases
            .iter()
            .any(|d| d.database.name == "custom_db_beta"));
        assert!(export
            .databases
            .iter()
            .any(|d| d.database.name == "custom_db_gamma"));
    }

    #[test]
    fn test_failure_verification_when_db_export_omitted_or_fails() {
        let mut registry = Registry::new();

        registry.sites.push(Site {
            id: "site-1".to_string(),
            name: "example.com".to_string(),
        });

        registry.databases.push(Database {
            id: "db-1".to_string(),
            name: "db_one".to_string(),
            owner_site_id: "site-1".to_string(),
        });
        registry.databases.push(Database {
            id: "db-2".to_string(),
            name: "db_two".to_string(),
            owner_site_id: "site-1".to_string(),
        });

        let export_result = registry.export_site("site-1", Some("db-2"));

        assert!(export_result.is_err());
        assert_eq!(
            export_result.unwrap_err(),
            "Export failed or omitted for database: db-2"
        );
    }
}
