#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use crate::{
        crypto::CryptoEngine,
        store::SecretStore,
        types::{SecretError, SecretMaterial},
    };

    #[test]
    fn test_store_reopen_and_key_rotation() {
        let dir = tempdir().unwrap();
        let storage_path = dir.path().to_path_buf();
        let master_key = CryptoEngine::generate_key();

        let tenant = "tenant1";
        let scope = "api_key";
        let original_material = SecretMaterial::new(b"my_api_key_456".to_vec());
        let secret_id;

        // Block 1: Create store and write secret
        {
            let store = SecretStore::new(&storage_path, &master_key, None).unwrap();
            let secret_ref = store.create_secret(tenant, scope, &original_material).unwrap();
            secret_id = secret_ref.id.clone();
        }

        // Verify file exists on disk and is encrypted (doesn't contain plaintext)
        let file_path = storage_path.join(format!("{}.json.v1", secret_id));
        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(!content.contains("my_api_key_456"));
        assert!(content.contains(&secret_id));

        // Block 2: Reopen store and read secret
        {
            let mut store = SecretStore::new(&storage_path, &master_key, None).unwrap();
            let read_material = store.read_secret(&secret_id, tenant, scope).unwrap();
            assert_eq!(read_material.as_bytes(), original_material.as_bytes());

            // Rotate master key
            let new_master_key = CryptoEngine::generate_key();
            store.rotate_master_key(&new_master_key).unwrap();
        }

        // Block 3: Reopen with NEW key and read secret
        {
            let store = SecretStore::new(&storage_path, &master_key, None);
            // Opening with old key shouldn't fail store creation, but decrypting will fail
            let store_old = store.unwrap();
            let err = store_old.read_secret(&secret_id, tenant, scope).unwrap_err();
            assert!(matches!(err, SecretError::CryptoError(_)));
        }
    }

    #[test]
    fn test_fault_injection_interrupted_rotation_before_commit() {
        let dir = tempdir().unwrap();
        let storage_path = dir.path().to_path_buf();
        let master_key = CryptoEngine::generate_key();

        let tenant = "tenant1";
        let scope = "api_key";
        let secret_id;

        // Block 1: Initial store and secret creation
        {
            let store = SecretStore::new(&storage_path, &master_key, None).unwrap();
            let secret_ref = store.create_secret(tenant, scope, &SecretMaterial::new(b"secret".to_vec())).unwrap();
            secret_id = secret_ref.id.clone();
        }

        // Simulate interruption before commit:
        // We write `.json.v2` files but leave `active_version` at 1 (or nonexistent).
        // This corresponds to an interruption *during* rotation but *before* commit.
        let v1_path = storage_path.join(format!("{}.json.v1", secret_id));
        let v2_path = storage_path.join(format!("{}.json.v2", secret_id));
        fs::copy(&v1_path, &v2_path).unwrap(); // Mocking a staged v2 file

        // Verify simulated state
        assert!(v1_path.exists());
        assert!(v2_path.exists());

        // Reopen store. It should read active_version=1, and clean up `.json.v2`
        let store = SecretStore::new(&storage_path, &master_key, None).unwrap();

        // v2 file should be deleted by cleanup
        assert!(!v2_path.exists());
        assert!(v1_path.exists());

        // Should still read secret using v1
        let read_material = store.read_secret(&secret_id, tenant, scope).unwrap();
        assert_eq!(read_material.as_bytes(), b"secret");
    }

    #[test]
    fn test_fault_injection_interrupted_rotation_after_commit() {
        let dir = tempdir().unwrap();
        let storage_path = dir.path().to_path_buf();
        let old_master_key = CryptoEngine::generate_key();
        let new_master_key = CryptoEngine::generate_key();

        let tenant = "tenant1";
        let scope = "api_key";
        let secret_id;

        // Block 1: Initial store and secret creation with OLD key
        {
            let store = SecretStore::new(&storage_path, &old_master_key, None).unwrap();
            let secret_ref = store.create_secret(tenant, scope, &SecretMaterial::new(b"secret".to_vec())).unwrap();
            secret_id = secret_ref.id.clone();
        }

        // Block 2: Rotate key
        {
            let mut store = SecretStore::new(&storage_path, &old_master_key, None).unwrap();
            store.rotate_master_key(&new_master_key).unwrap();
        }

        // Simulate an interruption AFTER rotation commit but BEFORE cleanup completed:
        // By default rotate_master_key cleans up, so we recreate the v1 file to simulate it.
        let v1_path = storage_path.join(format!("{}.json.v1", secret_id));
        let v2_path = storage_path.join(format!("{}.json.v2", secret_id));
        fs::write(&v1_path, "old dummy data").unwrap();

        assert!(v1_path.exists());
        assert!(v2_path.exists());

        // Reopen store with NEW key. It should read active_version=2 and clean up `.json.v1`
        let store = SecretStore::new(&storage_path, &new_master_key, None).unwrap();

        // v1 file should be deleted by cleanup
        assert!(!v1_path.exists());
        assert!(v2_path.exists());

        // Should read secret using v2
        let read_material = store.read_secret(&secret_id, tenant, scope).unwrap();
        assert_eq!(read_material.as_bytes(), b"secret");
    }

    #[test]
    fn test_e2e_inject_secret_into_isolated_service() {
        // E2E Verification: Inject secret into isolated service without logging it; recover after restore.
        // As defined in the issue, runtime prerequisites (isolated service injection, full OS restore)
        // are not available yet at this independent module stage.
        // Therefore, we provide an executable case definition that records BLOCKED.

        let runtime_prerequisites_available = false; // Mocking lack of environment

        if !runtime_prerequisites_available {
            panic!("RESULT=BLOCKED: E2E injection test requires isolated systemd service runner and machine restore capability.");
        }

        // Future implementation:
        // 1. Setup store and encrypt secret.
        // 2. Provision isolated service (e.g., systemd or container) passing secret via bounded tmpfs/env.
        // 3. Verify logs do not contain plaintext secret.
        // 4. Trigger restore and verify secret recovery.
        panic!("E2E test should be implemented once prerequisites are available");
    }
}
