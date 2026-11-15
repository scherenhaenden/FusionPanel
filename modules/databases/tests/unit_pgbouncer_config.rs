#![forbid(unsafe_code)]

use databases::pgbouncer_config::{PgBouncerConfig, PgBouncerDatabase, PgBouncerUser, PoolMode};

#[test]
fn test_pgbouncer_ini_generation() {
    let config = PgBouncerConfig {
        databases: vec![
            PgBouncerDatabase {
                name: "test_db".to_string(),
                host: "127.0.0.1".to_string(),
                port: 5432,
                dbname: "real_test_db".to_string(),
                auth_user: None,
            },
            PgBouncerDatabase {
                name: "prod_db".to_string(),
                host: "10.0.0.1".to_string(),
                port: 5433,
                dbname: "real_prod_db".to_string(),
                auth_user: Some("admin_user".to_string()),
            }
        ],
        users: vec![],
        listen_addr: "*".to_string(),
        listen_port: 6432,
        pool_mode: PoolMode::Transaction,
        max_client_conn: 1000,
        default_pool_size: 20,
    };

    let ini = config.generate_ini();

    assert!(ini.contains("[databases]"));
    assert!(ini.contains("test_db = host=127.0.0.1 port=5432 dbname=real_test_db"));
    assert!(ini.contains("prod_db = host=10.0.0.1 port=5433 dbname=real_prod_db auth_user=admin_user"));

    assert!(ini.contains("[pgbouncer]"));
    assert!(ini.contains("listen_addr = *"));
    assert!(ini.contains("listen_port = 6432"));
    assert!(ini.contains("pool_mode = transaction"));
    assert!(ini.contains("max_client_conn = 1000"));
    assert!(ini.contains("default_pool_size = 20"));
    assert!(ini.contains("server_reset_query = DISCARD ALL"));
    assert!(ini.contains("auth_type = md5"));
    assert!(ini.contains("auth_file = /etc/pgbouncer/userlist.txt"));
}

#[test]
fn test_pgbouncer_userlist_generation() {
    let config = PgBouncerConfig {
        databases: vec![],
        users: vec![
            PgBouncerUser {
                username: "user1".to_string(),
                password_hash: "md5_hash1".to_string(),
            },
            PgBouncerUser {
                username: "user2".to_string(),
                password_hash: "md5_hash2".to_string(),
            }
        ],
        listen_addr: "127.0.0.1".to_string(),
        listen_port: 6432,
        pool_mode: PoolMode::Session,
        max_client_conn: 100,
        default_pool_size: 10,
    };

    let userlist = config.generate_userlist();

    assert!(userlist.contains("\"user1\" \"md5_hash1\""));
    assert!(userlist.contains("\"user2\" \"md5_hash2\""));
}
