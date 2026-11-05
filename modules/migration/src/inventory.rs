use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InventoryItemType {
    Site,
    Database,
    User,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryItem {
    pub id: String,
    pub item_type: InventoryItemType,
    pub name: String,
    pub version: Option<String>,
    pub metadata: HashMap<String, String>,
    pub associations: Vec<String>, // Added to preserve site/database associations
    pub ownership: Option<String>,
    pub aliases: Vec<String>,
    pub schedules: Vec<String>,
    pub variants: Vec<String>,
    pub user_mappings: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationInventory {
    pub source_panel: String,
    pub source_version: String,
    pub items: Vec<InventoryItem>,
}

impl MigrationInventory {
    pub fn new(source_panel: String, source_version: String) -> Self {
        Self {
            source_panel,
            source_version,
            items: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_serialization() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.3.4".to_string());
        let mut user_mappings = HashMap::new();
        user_mappings.insert("admin".to_string(), "admin_user".to_string());

        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "test.com".to_string(),
            version: None,
            metadata: HashMap::new(),
            associations: vec!["db_1".to_string(), "db_2".to_string()],
            ownership: Some("admin".to_string()),
            aliases: vec!["www.test.com".to_string()],
            schedules: vec!["daily_backup".to_string()],
            variants: vec!["php81".to_string()],
            user_mappings,
        });

        let json = serde_json::to_string(&inventory).unwrap();
        let deserialized: MigrationInventory = serde_json::from_str(&json).unwrap();

        assert_eq!(inventory, deserialized);
    }
}
