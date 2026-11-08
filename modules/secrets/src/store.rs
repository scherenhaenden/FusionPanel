use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    os::unix::fs::OpenOptionsExt,
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
    #[serde(with = "hex")]
    pub encrypted_dek_master: Vec<u8>,
    #[serde(default, with = "hex_opt")]
    pub encrypted_dek_backup: Option<Vec<u8>>,
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

mod hex_opt {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match bytes {
            Some(b) => serializer.serialize_some(&hex::encode(b)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(s) => hex::decode(&s).map(Some).map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

pub struct SecretStore {
    storage_dir: PathBuf,
    engine: CryptoEngine,
    backup_engine: Option<CryptoEngine>,
    current_key_version: u32,
}

impl SecretStore {
    fn read_active_version(storage_dir: &Path) -> u32 {
        let path = storage_dir.join("active_version");
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(version) = data.trim().parse::<u32>() {
                return version;
            }
        }
        1
    }

    fn write_active_version(storage_dir: &Path, version: u32) -> Result<(), SecretError> {
        let path = storage_dir.join("active_version");
        let tmp_path = storage_dir.join("active_version.tmp");

        Self::write_with_permissions(&tmp_path, version.to_string().as_bytes())?;

        fs::rename(&tmp_path, &path).map_err(|e| {
            SecretError::OperationFailed(format!("Failed to commit active_version: {}", e))
        })?;
        Ok(())
    }

    fn write_with_permissions(path: &Path, data: &[u8]) -> Result<(), SecretError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| SecretError::OperationFailed(format!("Failed to open file: {}", e)))?;

        use std::io::Write;
        file.write_all(data)
            .map_err(|e| SecretError::OperationFailed(format!("Failed to write data: {}", e)))?;
        Ok(())
    }

    pub fn new<P: AsRef<Path>>(
        storage_dir: P,
        master_key: &[u8],
        backup_key: Option<&[u8]>,
    ) -> Result<Self, SecretError> {
        let storage_dir = storage_dir.as_ref().to_path_buf();
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir).map_err(|e| {
                SecretError::DependencyUnavailable(format!("Could not create store dir: {}", e))
            })?;
        }

        let engine = CryptoEngine::new(master_key)?;
        let backup_engine = match backup_key {
            Some(bk) => Some(CryptoEngine::new(bk)?),
            None => None,
        };
        let current_key_version = Self::read_active_version(&storage_dir);

        let store = Self {
            storage_dir,
            engine,
            backup_engine,
            current_key_version,
        };

        // Cleanup stale files during startup (recovery phase)
        store.cleanup_stale_files()?;

        Ok(store)
    }

    fn cleanup_stale_files(&self) -> Result<(), SecretError> {
        let suffix = format!(".v{}", self.current_key_version);
        for entry in fs::read_dir(&self.storage_dir)
            .map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?
        {
            let entry = entry.map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?;
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            // Files might be `<id>.json` (legacy), `<id>.json.v<X>`, active_version
            if file_name.starts_with("active_version") {
                continue;
            }

            if file_name.ends_with(".json") {
                // Legacy file, migrate to current version
                let new_path = PathBuf::from(format!(
                    "{}.v{}",
                    path.to_string_lossy(),
                    self.current_key_version
                ));
                let _ = fs::rename(&path, &new_path);
            } else if file_name.contains(".json.v") && !file_name.ends_with(&suffix) {
                // Stale file from interrupted rotation, or old version file after successful rotation
                let _ = fs::remove_file(&path);
            }
        }
        Ok(())
    }

    pub fn rotate_master_key(&mut self, new_master_key: &[u8]) -> Result<(), SecretError> {
        let new_engine = CryptoEngine::new(new_master_key)?;
        let new_version = self.current_key_version + 1;
        let mut new_records = HashMap::new();

        // Phase 1: Staged Generation (Re-encrypt all records to new version files)
        let suffix = format!(".json.v{}", self.current_key_version);
        for entry in fs::read_dir(&self.storage_dir)
            .map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?
        {
            let entry = entry.map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?;
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            if !file_name.ends_with(&suffix) {
                continue;
            }

            let data = fs::read_to_string(&path)
                .map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?;
            let mut record: SecretRecord = serde_json::from_str(&data)
                .map_err(|e| SecretError::DependencyUnavailable(e.to_string()))?;

            let aad = Self::construct_aad(&record.id, &record.scopes);

            // Decrypt DEK with old master key
            let dek_material = self.engine.decrypt(&record.encrypted_dek_master, &aad)?;

            // Re-encrypt DEK with new master key
            let new_encrypted_dek_master = new_engine.encrypt(&dek_material, &aad)?;

            record.encrypted_dek_master = new_encrypted_dek_master;
            // record.ciphertext and record.encrypted_dek_backup remain untouched

            // Store target path for new version
            let new_file_name = file_name.replace(&suffix, &format!(".json.v{}", new_version));
            let new_path = self.storage_dir.join(new_file_name);
            new_records.insert(new_path, record);
        }

        // Write all new records (staged generation)
        for (path, record) in new_records {
            let data = serde_json::to_string_pretty(&record)
                .map_err(|e| SecretError::OperationFailed(e.to_string()))?;
            Self::write_with_permissions(&path, data.as_bytes())?;
        }

        // Phase 2: Commit (Update active_version atomically)
        Self::write_active_version(&self.storage_dir, new_version)?;

        // Phase 3: Recovery (Update in-memory state and clean up old versions)
        self.engine = new_engine;
        self.current_key_version = new_version;
        self.cleanup_stale_files()?;

        Ok(())
    }

    fn construct_aad(id: &str, scopes: &[String]) -> Vec<u8> {
        let scopes_str = scopes.join(",");
        format!("{}:{}", id, scopes_str).into_bytes()
    }

    pub fn create_secret(
        &self,
        tenant_id: &str,
        scope: &str,
        material: &SecretMaterial,
    ) -> Result<SecretRef, SecretError> {
        let id = Uuid::new_v4().to_string();
        let scopes = vec![format!("{}:{}", tenant_id, scope)];

        // Generate DEK
        let dek_bytes = CryptoEngine::generate_key();
        let dek_engine = CryptoEngine::new(&dek_bytes)?;
        let dek_material = SecretMaterial::new(dek_bytes);

        let aad = Self::construct_aad(&id, &scopes);

        // Encrypt secret material with DEK
        let ciphertext = dek_engine.encrypt(material, &aad)?;

        // Encrypt DEK with master key
        let encrypted_dek_master = self.engine.encrypt(&dek_material, &aad)?;

        // Encrypt DEK with backup key if available
        let encrypted_dek_backup = if let Some(backup_engine) = &self.backup_engine {
            Some(backup_engine.encrypt(&dek_material, &aad)?)
        } else {
            None
        };

        let record = SecretRecord {
            id: id.clone(),
            version: 1,
            scopes,
            ciphertext,
            encrypted_dek_master,
            encrypted_dek_backup,
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

        let aad = Self::construct_aad(&record.id, &record.scopes);

        // Decrypt DEK with master key
        let dek_material = self.engine.decrypt(&record.encrypted_dek_master, &aad)?;
        let dek_engine = CryptoEngine::new(dek_material.as_bytes())?;

        // Decrypt secret with DEK
        dek_engine.decrypt(&record.ciphertext, &aad)
    }

    pub fn recover_secret(
        &self,
        id: &str,
        tenant_id: &str,
        scope: &str,
    ) -> Result<SecretMaterial, SecretError> {
        let backup_engine = self.backup_engine.as_ref().ok_or_else(|| {
            SecretError::DependencyUnavailable("Backup key not configured".to_string())
        })?;

        let record = self.load_record(id)?;

        let required_scope = format!("{}:{}", tenant_id, scope);
        if !record.scopes.contains(&required_scope) {
            return Err(SecretError::Unauthorized(format!(
                "Scope {} not authorized for secret {}",
                required_scope, id
            )));
        }

        let encrypted_dek_backup = record.encrypted_dek_backup.as_ref().ok_or_else(|| {
            SecretError::NotFound("Backup encrypted DEK not found on record".to_string())
        })?;

        let aad = Self::construct_aad(&record.id, &record.scopes);

        // Decrypt DEK with backup key
        let dek_material = backup_engine.decrypt(encrypted_dek_backup, &aad)?;
        let dek_engine = CryptoEngine::new(dek_material.as_bytes())?;

        // Decrypt secret with DEK
        dek_engine.decrypt(&record.ciphertext, &aad)
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
        self.storage_dir
            .join(format!("{}.json.v{}", id, self.current_key_version))
    }

    fn save_record(&self, record: &SecretRecord) -> Result<(), SecretError> {
        let path = self.file_path(&record.id);
        let data = serde_json::to_string_pretty(record)
            .map_err(|e| SecretError::OperationFailed(e.to_string()))?;
        Self::write_with_permissions(&path, data.as_bytes())?;
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
        let store = SecretStore::new(dir.path(), &master_key, None).unwrap();

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
        let store = SecretStore::new(dir.path(), &master_key, None).unwrap();

        let material = SecretMaterial::new(b"db_secret_123".to_vec());
        let secret_ref = store
            .create_secret("tenant1", "db_password", &material)
            .unwrap();

        // Wrong tenant
        let result = store.read_secret(&secret_ref.id, "tenant2", "db_password");
        assert!(matches!(result.unwrap_err(), SecretError::Unauthorized(_)));

        // Wrong scope
        let result = store.read_secret(&secret_ref.id, "tenant1", "api_key");
        assert!(matches!(result.unwrap_err(), SecretError::Unauthorized(_)));
    }

    #[test]
    fn test_metadata_integrity_tampering() {
        let dir = tempdir().unwrap();
        let master_key = CryptoEngine::generate_key();
        let store = SecretStore::new(dir.path(), &master_key, None).unwrap();

        let material = SecretMaterial::new(b"secret".to_vec());
        let secret_ref = store.create_secret("tenant1", "scope1", &material).unwrap();

        // Read file and tamper with scopes
        let file_path = store.file_path(&secret_ref.id);
        let data = fs::read_to_string(&file_path).unwrap();
        let mut record: SecretRecord = serde_json::from_str(&data).unwrap();
        record.scopes = vec!["tenant2:scope1".to_string()];
        let tampered_data = serde_json::to_string_pretty(&record).unwrap();
        fs::write(&file_path, tampered_data).unwrap();

        // Attempt to read with tampered scope should fail due to AAD mismatch
        let result = store.read_secret(&secret_ref.id, "tenant2", "scope1");
        assert!(matches!(result.unwrap_err(), SecretError::CryptoError(_)));
    }

    #[test]
    fn test_recover_secret() {
        let dir = tempdir().unwrap();
        let master_key = CryptoEngine::generate_key();
        let backup_key = CryptoEngine::generate_key();

        let store = SecretStore::new(dir.path(), &master_key, Some(&backup_key)).unwrap();

        let material = SecretMaterial::new(b"critical_data".to_vec());
        let secret_ref = store.create_secret("tenant1", "scope1", &material).unwrap();

        // Reading normally works
        let read_material = store
            .read_secret(&secret_ref.id, "tenant1", "scope1")
            .unwrap();
        assert_eq!(read_material.as_bytes(), material.as_bytes());

        // Recovering with backup key works
        let recovered_material = store
            .recover_secret(&secret_ref.id, "tenant1", "scope1")
            .unwrap();
        assert_eq!(recovered_material.as_bytes(), material.as_bytes());

        // Now imagine master key is lost, open store with NO master key but valid backup key
        // Note: the current `new` requires a master key, so we simulate a lost master key by
        // providing a new random one.
        let random_master_key = CryptoEngine::generate_key();
        let store_lost_master =
            SecretStore::new(dir.path(), &random_master_key, Some(&backup_key)).unwrap();

        // Reading with new (wrong) master key fails
        assert!(store_lost_master
            .read_secret(&secret_ref.id, "tenant1", "scope1")
            .is_err());

        // But recovery still works!
        let recovered_material_2 = store_lost_master
            .recover_secret(&secret_ref.id, "tenant1", "scope1")
            .unwrap();
        assert_eq!(recovered_material_2.as_bytes(), material.as_bytes());
    }

    #[test]
    fn test_recover_secret_no_backup_key() {
        let dir = tempdir().unwrap();
        let master_key = CryptoEngine::generate_key();

        // No backup key provided
        let store = SecretStore::new(dir.path(), &master_key, None).unwrap();

        let material = SecretMaterial::new(b"critical_data".to_vec());
        let secret_ref = store.create_secret("tenant1", "scope1", &material).unwrap();

        // Attempting to recover fails because no backup key is configured in store
        let result = store.recover_secret(&secret_ref.id, "tenant1", "scope1");
        assert!(matches!(
            result.unwrap_err(),
            SecretError::DependencyUnavailable(_)
        ));
    }
}
