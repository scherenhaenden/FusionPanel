use migration::discovery::CloudPanelDiscovery;
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn integration_dry_run_plan_generation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("cloudpanel.sqlite");

    {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE setting (name TEXT PRIMARY KEY, value TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO setting (name, value) VALUES ('version', '2.4.1')",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE site (id INTEGER PRIMARY KEY, domain_name TEXT, php_version TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO site (id, domain_name, php_version) VALUES (1, 'mysite.com', '8.2')",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE database (id INTEGER PRIMARY KEY, database_name TEXT, site_id INTEGER)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO database (id, database_name, site_id) VALUES (10, 'my_db_1', 1)",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE user (id INTEGER PRIMARY KEY, user_name TEXT, role TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user (id, user_name, role) VALUES (100, 'admin_user', 'admin')",
            [],
        )
        .unwrap();
    }

    let inventory = CloudPanelDiscovery::discover(&db_path).unwrap();
    let plan = migration::planner::Planner::create_plan(&inventory, true);

    assert_eq!(plan.is_dry_run, true);
    assert_eq!(plan.actions.len(), 3);
}
