#![forbid(unsafe_code)]

use adopt01::{
    Configuration, Location, ReconciliationEngine, ReconciliationResult, RouteMap, SiteType,
};

fn default_config() -> Configuration {
    Configuration {
        site_type: SiteType::PhpFpm,
        root: "/var/www/html".to_string(),
        upstream_scheme: "http".to_string(),
        upstream_port: 8080,
        aliases: vec!["www.example.com".to_string()],
        route_map: RouteMap {
            locations: vec![Location {
                path: "/".to_string(),
                is_exact: false,
            }],
            headers: vec![("X-Frame-Options".to_string(), "DENY".to_string())],
            rewrites: vec!["^/old$ /new".to_string()],
            websocket_paths: vec!["/ws".to_string()],
        },
    }
}

#[test]
fn test_reconciliation_success() {
    let config = default_config();
    let result = ReconciliationEngine::reconcile(&config, &config, &config, &config);

    assert!(matches!(result, ReconciliationResult::Success(_)));
    assert_eq!(ReconciliationEngine::apply_migration(&result), Ok(()));
}

#[test]
fn test_conflict_site_type() {
    let panel_config = default_config();
    let mut web_config = default_config();
    web_config.site_type = SiteType::Proxy;

    let result = ReconciliationEngine::reconcile(&panel_config, &web_config, &panel_config, &panel_config);

    if let ReconciliationResult::Conflicts(conflicts) = &result {
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "site_type");
        assert_eq!(conflicts[0].panel_value, "PhpFpm");
        assert_eq!(conflicts[0].observed_value, "Proxy");
    } else {
        panic!("Expected conflicts");
    }

    assert!(ReconciliationEngine::apply_migration(&result).is_err());
}

#[test]
fn test_conflict_root() {
    let panel_config = default_config();
    let mut web_config = default_config();
    web_config.root = "/var/www/other".to_string();

    let result = ReconciliationEngine::reconcile(&panel_config, &web_config, &panel_config, &panel_config);

    if let ReconciliationResult::Conflicts(conflicts) = &result {
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "root");
        assert_eq!(conflicts[0].panel_value, "/var/www/html");
        assert_eq!(conflicts[0].observed_value, "web: /var/www/other, app: /var/www/html");
    } else {
        panic!("Expected conflicts");
    }

    assert!(ReconciliationEngine::apply_migration(&result).is_err());
}

#[test]
fn test_conflict_upstream() {
    let panel_config = default_config();
    let mut live_upstream = default_config();
    live_upstream.upstream_scheme = "https".to_string();
    live_upstream.upstream_port = 8443;

    let result = ReconciliationEngine::reconcile(&panel_config, &panel_config, &panel_config, &live_upstream);

    if let ReconciliationResult::Conflicts(conflicts) = &result {
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].field, "upstream_scheme");
        assert_eq!(conflicts[0].panel_value, "http");
        assert_eq!(conflicts[0].observed_value, "https");
        assert_eq!(conflicts[1].field, "upstream_port");
        assert_eq!(conflicts[1].panel_value, "8080");
        assert_eq!(conflicts[1].observed_value, "8443");
    } else {
        panic!("Expected conflicts");
    }

    assert!(ReconciliationEngine::apply_migration(&result).is_err());
}

#[test]
fn test_conflict_route_map_locations() {
    let panel_config = default_config();
    let mut web_config = default_config();
    web_config.route_map.locations.push(Location {
        path: "/api".to_string(),
        is_exact: true,
    });

    let result = ReconciliationEngine::reconcile(&panel_config, &web_config, &panel_config, &panel_config);

    if let ReconciliationResult::Conflicts(conflicts) = &result {
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "route_map.locations");
    } else {
        panic!("Expected conflicts");
    }

    assert!(ReconciliationEngine::apply_migration(&result).is_err());
}

#[test]
fn test_conflict_route_map_headers() {
    let panel_config = default_config();
    let mut web_config = default_config();
    web_config.route_map.headers.clear();

    let result = ReconciliationEngine::reconcile(&panel_config, &web_config, &panel_config, &panel_config);

    if let ReconciliationResult::Conflicts(conflicts) = &result {
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "route_map.headers");
    } else {
        panic!("Expected conflicts");
    }

    assert!(ReconciliationEngine::apply_migration(&result).is_err());
}

#[test]
fn test_conflict_aliases() {
    let panel_config = default_config();
    let mut web_config = default_config();
    web_config.aliases.push("alias.example.com".to_string());

    let result = ReconciliationEngine::reconcile(&panel_config, &web_config, &panel_config, &panel_config);

    if let ReconciliationResult::Conflicts(conflicts) = &result {
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "aliases");
    } else {
        panic!("Expected conflicts");
    }

    assert!(ReconciliationEngine::apply_migration(&result).is_err());
}
