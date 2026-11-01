#![forbid(unsafe_code)]

#[cfg(test)]
mod malformed_data_tests {
    use fusionpanel_jobs::{JobStore, JobStoreError};
    use rusqlite::Connection;
    use serde_json::json;

    #[test]
    fn test_malformed_data() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_jobs_malformed_{}.db", uuid::Uuid::new_v4()));

        {
            let store = JobStore::new(&db_path).unwrap();
            store.create("job", &json!({})).unwrap();
        }

        // Intentionally corrupt the database state using direct connection
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute("UPDATE jobs SET state = 'invalid_state'", [])
                .unwrap();
        }

        // Now test handling of malformed data via JobStore
        {
            let store = JobStore::new(&db_path).unwrap();
            let res = store.list();
            assert!(res.is_err());
            match res {
                Err(JobStoreError::Database(rusqlite::Error::InvalidColumnType(col, _, _))) => {
                    assert_eq!(col, 3); // state column
                }
                other => panic!("Expected Database InvalidColumnType error, got {:?}", other),
            }
        }

        std::fs::remove_file(db_path).unwrap();
    }
}
