use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    crypto::CryptoEngine,
    types::{ObservedState, SecretError, SecretMaterial, SecretRef},
};

/// A stored secret record
#[derive(Debug, Serialize, Deserialize)]
pub struct SecretRecord {
    pub id: String,
    pub version: u32,
    pub scopes: Vec<String>,
    #[serde(with = "hex")]
    pub ciphertext: Vec<u8>,
}

mod hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

pub struct SecretStore {
    storage_dir: PathBuf,
    engine: CryptoEngine,
}

impl SecretStore {
    pub fn new<P: AsRef<Path>>(
        storage_dir: P,
        master_key: &[u8],
    ) -> Result<Self, SecretError> {
        let storage_dir = storage_dir.as_ref().to_path_buf();
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir).map_err(|e| {
                SecretError::DependencyUnavailable(format!("Could not create store dir: {}", e))
            })?;
        }

        let engine = CryptoEngine::new(master_key)?;

        Ok(Self { storage_dir, engine })
    }

    pub fn rotate_master_key(&mut self, new_master_key: &[u8]) -> Result<(), SecretError> {
        let new_engine = CryptoEngine::new(new_master_key)?;

        let mut new_records = HashMap::new();

        // Re-encrypt all records
        for entry in fs::read_dir(&self.storage_dir)
            .map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?
        {
            let entry = entry.map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let data = fs::read_to_string(&path)
                .map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?;
            let mut record: SecretRecord = serde_json::from_str(&data)
                .map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?;

            let plaintext = self.engine.decrypt(&record.ciphertext)?;
            let new_ciphertext = new_engine.encrypt(&plaintext)?;

            record.ciphertext = new_ciphertext;
            new_records.insert(path, record);
        }

        // Write all new records
        for (path, record) in new_records {
            let data = serde_json::to_string_pretty(&record)
                .map_err(|e| SecretError::OperationFailed(e.to_string()))?;
            fs::write(path, data)
                .map_err(|e| SecretError::OperationFailed(e.to_string()))?;
        }

        self.engine = new_engine;
        Ok(())
    }

    pub fn create_secret(
        &self,
        tenant_id: &str,
        scope: &str,
        material: &SecretMaterial,
    ) -> Result<SecretRef, SecretError> {
        let id = Uuid::new_v4().to_string();
        let ciphertext = self.engine.encrypt(material)?;

        let record = SecretRecord {
            id: id.clone(),
            version: 1,
            scopes: vec![format!("{}:{}", tenant_id, scope)],
            ciphertext,
        };

        self.save_record(&record)?;

        Ok(SecretRef { id, version: 1 })
    }

    pub fn read_secret(
        &self,
        id: &str,
        tenant_id: &str,
        scope: &str,
    ) -> Result<SecretMaterial, SecretError> {
        let record = self.load_record(id)?;

        let required_scope = format!("{}:{}", tenant_id, scope);
        if !record.scopes.contains(&required_scope) {
            return Err(SecretError::Unauthorized(format!(
                "Scope {} not authorized for secret {}",
                required_scope, id
            )));
        }

        self.engine.decrypt(&record.ciphertext)
    }

    pub fn get_state(&self, id: &str) -> Result<ObservedState, SecretError> {
        let record = self.load_record(id)?;
        Ok(ObservedState {
            secret_id: record.id,
            current_version: record.version,
            scopes: record.scopes,
        })
    }

    fn file_path(&self, id: &str) -> PathBuf {
        self.storage_dir.join(format!("{}.json", id))
    }

    fn save_record(&self, record: &SecretRecord) -> Result<(), SecretError> {
        let path = self.file_path(&record.id);
        let data = serde_json::to_string_pretty(record)
            .map_err(|e| SecretError::OperationFailed(e.to_string()))?;
        fs::write(path, data).map_err(|e| SecretError::OperationFailed(e.to_string()))?;
        Ok(())
    }

    fn load_record(&self, id: &str) -> Result<SecretRecord, SecretError> {
        let path = self.file_path(id);
        if !path.exists() {
            return Err(SecretError::NotFound(id.to_string()));
        }
        let data = fs::read_to_string(path)
            .map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?;
        serde_json::from_str(&data).map_err(|e| SecretError::OperationFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_and_read_secret() {
        let dir = tempdir().unwrap();
        let master_key = CryptoEngine::generate_key();
        let store = SecretStore::new(dir.path(), &master_key).unwrap();

        let tenant = "tenant1";
        let scope = "db_password";
        let material = SecretMaterial::new(b"db_secret_123".to_vec());

        let secret_ref = store.create_secret(tenant, scope, &material).unwrap();
        assert_eq!(secret_ref.version, 1);

        let read_material = store.read_secret(&secret_ref.id, tenant, scope).unwrap();
        assert_eq!(read_material.as_bytes(), material.as_bytes());

        let state = store.get_state(&secret_ref.id).unwrap();
        assert_eq!(state.secret_id, secret_ref.id);
        assert_eq!(state.current_version, 1);
        assert_eq!(state.scopes, vec![format!("{}:{}", tenant, scope)]);
    }

    #[test]
    fn test_access_denied() {
        let dir = tempdir().unwrap();
        let master_key = CryptoEngine::generate_key();
        let store = SecretStore::new(dir.path(), &master_key).unwrap();

        let material = SecretMaterial::new(b"db_secret_123".to_vec());
        let secret_ref = store.create_secret("tenant1", "db_password", &material).unwrap();

        // Wrong tenant
        let result = store.read_secret(&secret_ref.id, "tenant2", "db_password");
        assert!(matches!(result.unwrap_err(), SecretError::Unauthorized(_)));

        // Wrong scope
        let result = store.read_secret(&secret_ref.id, "tenant1", "api_key");
        assert!(matches!(result.unwrap_err(), SecretError::Unauthorized(_)));
    }
}
