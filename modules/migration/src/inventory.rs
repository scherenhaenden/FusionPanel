use serde::{Deserialize, Serialize};

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
    pub metadata: std::collections::HashMap<String, String>,
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
        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "test.com".to_string(),
            version: None,
            metadata: std::collections::HashMap::new(),
        });

        let json = serde_json::to_string(&inventory).unwrap();
        let deserialized: MigrationInventory = serde_json::from_str(&json).unwrap();

        assert_eq!(inventory, deserialized);
    }
}
