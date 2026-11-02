#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteType {
    Static,
    Proxy,
    PhpFpm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: String,
    pub is_exact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteMap {
    pub locations: Vec<Location>,
    pub headers: Vec<(String, String)>,
    pub rewrites: Vec<String>,
    pub websocket_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    pub site_type: SiteType,
    pub root: String,
    pub upstream_scheme: String,
    pub upstream_port: u16,
    pub aliases: Vec<String>,
    pub route_map: RouteMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationConflict {
    pub field: String,
    pub panel_value: String,
    pub observed_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationResult {
    Success(Configuration),
    Conflicts(Vec<ReconciliationConflict>),
}

pub struct ReconciliationEngine;

impl ReconciliationEngine {
    pub fn reconcile(
        panel_metadata: &Configuration,
        web_server_config: &Configuration,
        app_config: &Configuration,
        live_upstream: &Configuration,
    ) -> ReconciliationResult {
        let mut conflicts = Vec::new();

        // 1. Reconcile Site Type
        if panel_metadata.site_type != web_server_config.site_type {
            conflicts.push(ReconciliationConflict {
                field: "site_type".to_string(),
                panel_value: format!("{:?}", panel_metadata.site_type),
                observed_value: format!("{:?}", web_server_config.site_type),
            });
        }

        // 2. Reconcile Root Directory
        if panel_metadata.root != web_server_config.root || panel_metadata.root != app_config.root {
            conflicts.push(ReconciliationConflict {
                field: "root".to_string(),
                panel_value: panel_metadata.root.clone(),
                observed_value: format!("web: {}, app: {}", web_server_config.root, app_config.root),
            });
        }

        // 3. Reconcile Upstream Scheme & Port (comparing against live upstream primarily)
        if panel_metadata.upstream_scheme != live_upstream.upstream_scheme {
            conflicts.push(ReconciliationConflict {
                field: "upstream_scheme".to_string(),
                panel_value: panel_metadata.upstream_scheme.clone(),
                observed_value: live_upstream.upstream_scheme.clone(),
            });
        }

        if panel_metadata.upstream_port != live_upstream.upstream_port {
            conflicts.push(ReconciliationConflict {
                field: "upstream_port".to_string(),
                panel_value: panel_metadata.upstream_port.to_string(),
                observed_value: live_upstream.upstream_port.to_string(),
            });
        }

        // 4. Reconcile Aliases
        if panel_metadata.aliases != web_server_config.aliases {
            conflicts.push(ReconciliationConflict {
                field: "aliases".to_string(),
                panel_value: format!("{:?}", panel_metadata.aliases),
                observed_value: format!("{:?}", web_server_config.aliases),
            });
        }

        // 5. Reconcile Route Maps
        if panel_metadata.route_map.locations != web_server_config.route_map.locations {
            conflicts.push(ReconciliationConflict {
                field: "route_map.locations".to_string(),
                panel_value: format!("{:?}", panel_metadata.route_map.locations),
                observed_value: format!("{:?}", web_server_config.route_map.locations),
            });
        }

        if panel_metadata.route_map.headers != web_server_config.route_map.headers {
            conflicts.push(ReconciliationConflict {
                field: "route_map.headers".to_string(),
                panel_value: format!("{:?}", panel_metadata.route_map.headers),
                observed_value: format!("{:?}", web_server_config.route_map.headers),
            });
        }

        if panel_metadata.route_map.rewrites != web_server_config.route_map.rewrites {
            conflicts.push(ReconciliationConflict {
                field: "route_map.rewrites".to_string(),
                panel_value: format!("{:?}", panel_metadata.route_map.rewrites),
                observed_value: format!("{:?}", web_server_config.route_map.rewrites),
            });
        }

        if panel_metadata.route_map.websocket_paths != web_server_config.route_map.websocket_paths {
             conflicts.push(ReconciliationConflict {
                field: "route_map.websocket_paths".to_string(),
                panel_value: format!("{:?}", panel_metadata.route_map.websocket_paths),
                observed_value: format!("{:?}", web_server_config.route_map.websocket_paths),
            });
        }

        if conflicts.is_empty() {
            // Assume panel metadata is the source of truth if there are no conflicts
            ReconciliationResult::Success(panel_metadata.clone())
        } else {
            ReconciliationResult::Conflicts(conflicts)
        }
    }

    pub fn apply_migration(result: &ReconciliationResult) -> Result<(), String> {
        match result {
            ReconciliationResult::Success(_) => {
                // Apply migration logic goes here
                Ok(())
            },
            ReconciliationResult::Conflicts(conflicts) => {
                // Fail closed
                let mut error_msg = "Migration refused due to unresolved conflicts:\n".to_string();
                for conflict in conflicts {
                    error_msg.push_str(&format!("- Field '{}': Expected '{}', Found '{}'\n", conflict.field, conflict.panel_value, conflict.observed_value));
                }
                Err(error_msg)
            }
        }
    }
}
