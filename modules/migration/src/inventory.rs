use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InventoryItemType {
    Site,
    Database,
    DatabaseUser,
    User,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryItem {
    pub id: String,
    pub item_type: InventoryItemType,
    pub name: String,
    pub version: Option<String>,
    /// Stable identifiers of source resources this item belongs to. A site may
    /// have several databases; each database can have several database users.
    pub associations: Vec<String>,
    /// Non-secret source attributes. Passwords, private keys and credentials
    /// must never be added here.
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationInventory {
    pub source_panel: String,
    pub source_version: String,
    pub schema_profile: String,
    pub items: Vec<InventoryItem>,
}

impl MigrationInventory {
    pub fn new(source_panel: String, source_version: String, schema_profile: String) -> Self {
        Self {
            source_panel,
            source_version,
            schema_profile,
            items: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_roundtrip_preserves_many_to_many_associations() {
        let mut inventory =
            MigrationInventory::new("CloudPanel".into(), "2.5.4".into(), "v2-site-db".into());
        for (id, site) in [
            ("db_10", "site_1"),
            ("db_11", "site_1"),
            ("db_12", "site_2"),
        ] {
            inventory.items.push(InventoryItem {
                id: id.into(),
                item_type: InventoryItemType::Database,
                name: id.into(),
                version: None,
                associations: vec![site.into()],
                metadata: HashMap::new(),
            });
        }
        let encoded = serde_json::to_string(&inventory).unwrap();
        let decoded: MigrationInventory = serde_json::from_str(&encoded).unwrap();
        assert_eq!(inventory, decoded);
        assert_eq!(
            decoded
                .items
                .iter()
                .filter(|item| item.associations == ["site_1"])
                .count(),
            2
        );
    }
}
