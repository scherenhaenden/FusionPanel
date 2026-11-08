#![forbid(unsafe_code)]

#[path = "../src/maintenance.rs"]
pub mod maintenance;

use maintenance::{MaintenanceRecommendation, TableMaintenanceAnalyzer, TableStats};

#[test]
fn test_postgres_analysis_vacuum_needed() {
    let analyzer = TableMaintenanceAnalyzer::default();

    // threshold = 50 + 0.20 * 1000 = 250
    let stats = TableStats {
        table_name: "users".to_string(),
        table_size_bytes: 1024 * 1024,
        dead_tuples: Some(300),
        live_tuples: Some(1000),
        data_free_bytes: None,
        freelist_pages: None,
        page_size: None,
    };

    assert_eq!(analyzer.analyze_postgres(&stats), MaintenanceRecommendation::Vacuum);
}

#[test]
fn test_postgres_analysis_vacuum_not_needed() {
    let analyzer = TableMaintenanceAnalyzer::default();

    // threshold = 50 + 0.20 * 1000 = 250
    let stats = TableStats {
        table_name: "users".to_string(),
        table_size_bytes: 1024 * 1024,
        dead_tuples: Some(200),
        live_tuples: Some(1000),
        data_free_bytes: None,
        freelist_pages: None,
        page_size: None,
    };

    assert_eq!(analyzer.analyze_postgres(&stats), MaintenanceRecommendation::None);
}

#[test]
fn test_mysql_analysis_optimize_needed() {
    let analyzer = TableMaintenanceAnalyzer::default();

    let stats = TableStats {
        table_name: "logs".to_string(),
        table_size_bytes: 50 * 1024 * 1024, // 50MB
        dead_tuples: None,
        live_tuples: None,
        data_free_bytes: Some(15 * 1024 * 1024), // 15MB free (ratio = 15 / 65 ~ 23% > 10%)
        freelist_pages: None,
        page_size: None,
    };

    assert_eq!(analyzer.analyze_mysql(&stats), MaintenanceRecommendation::Optimize);
}

#[test]
fn test_mysql_analysis_optimize_not_needed_due_to_size() {
    let analyzer = TableMaintenanceAnalyzer::default();

    let stats = TableStats {
        table_name: "logs".to_string(),
        table_size_bytes: 5 * 1024 * 1024,
        dead_tuples: None,
        live_tuples: None,
        data_free_bytes: Some(1 * 1024 * 1024), // 1MB free < 10MB min free bytes
        freelist_pages: None,
        page_size: None,
    };

    assert_eq!(analyzer.analyze_mysql(&stats), MaintenanceRecommendation::None);
}

#[test]
fn test_mysql_analysis_optimize_not_needed_due_to_ratio() {
    let analyzer = TableMaintenanceAnalyzer::default();

    let stats = TableStats {
        table_name: "logs".to_string(),
        table_size_bytes: 500 * 1024 * 1024, // 500MB
        dead_tuples: None,
        live_tuples: None,
        data_free_bytes: Some(20 * 1024 * 1024), // 20MB free (ratio = 20 / 520 ~ 3.8% < 10%)
        freelist_pages: None,
        page_size: None,
    };

    assert_eq!(analyzer.analyze_mysql(&stats), MaintenanceRecommendation::None);
}

#[test]
fn test_sqlite_analysis_vacuum_needed() {
    let analyzer = TableMaintenanceAnalyzer::default();

    let stats = TableStats {
        table_name: "sqlite_schema".to_string(),
        table_size_bytes: 1000 * 4096, // Total size = 1000 pages
        dead_tuples: None,
        live_tuples: None,
        data_free_bytes: None,
        freelist_pages: Some(200), // 20% fragmentation
        page_size: Some(4096),
    };

    assert_eq!(analyzer.analyze_sqlite(&stats), MaintenanceRecommendation::Vacuum);
}

#[test]
fn test_sqlite_analysis_vacuum_not_needed() {
    let analyzer = TableMaintenanceAnalyzer::default();

    let stats = TableStats {
        table_name: "sqlite_schema".to_string(),
        table_size_bytes: 1000 * 4096, // Total size = 1000 pages
        dead_tuples: None,
        live_tuples: None,
        data_free_bytes: None,
        freelist_pages: Some(50), // 5% fragmentation
        page_size: Some(4096),
    };

    assert_eq!(analyzer.analyze_sqlite(&stats), MaintenanceRecommendation::None);
}

#[test]
fn test_query_builders() {
    let pg_query = TableMaintenanceAnalyzer::build_pg_query();
    assert!(pg_query.contains("pg_relation_size(oid)"));
    assert!(pg_query.contains("n_dead_tup"));

    let mysql_query = TableMaintenanceAnalyzer::build_mysql_query("my_db");
    assert!(mysql_query.contains("data_free"));
    assert!(mysql_query.contains("table_schema = 'my_db'"));

    assert_eq!(TableMaintenanceAnalyzer::build_sqlite_freelist_query(), "PRAGMA freelist_count;");
    assert_eq!(TableMaintenanceAnalyzer::build_sqlite_page_size_query(), "PRAGMA page_size;");
    assert_eq!(TableMaintenanceAnalyzer::build_sqlite_page_count_query(), "PRAGMA page_count;");
}
