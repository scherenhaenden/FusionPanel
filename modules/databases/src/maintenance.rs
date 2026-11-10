#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaintenanceRecommendation {
    None,
    Vacuum,
    Optimize,
}

#[derive(Debug, Clone)]
pub struct TableStats {
    pub table_name: String,
    pub table_size_bytes: u64,

    // Engine specific
    // PostgreSQL
    pub dead_tuples: Option<u64>,
    pub live_tuples: Option<u64>,

    // MySQL/MariaDB
    pub data_free_bytes: Option<u64>,

    // SQLite
    pub freelist_pages: Option<u64>,
    pub page_size: Option<u64>,
}

pub struct TableMaintenanceAnalyzer {
    // Configurable thresholds
    pub postgres_autovacuum_threshold: f64, // percentage, e.g., 0.20 for 20%
    pub postgres_autovacuum_base: u64,      // minimum dead tuples

    pub mysql_fragmentation_ratio: f64, // data_free / (table_size + data_free)
    pub mysql_min_free_bytes: u64,

    pub sqlite_fragmentation_ratio: f64,
}

impl Default for TableMaintenanceAnalyzer {
    fn default() -> Self {
        Self {
            postgres_autovacuum_threshold: 0.20,
            postgres_autovacuum_base: 50,

            mysql_fragmentation_ratio: 0.10, // 10% fragmentation
            mysql_min_free_bytes: 10 * 1024 * 1024, // 10MB

            sqlite_fragmentation_ratio: 0.10,
        }
    }
}

impl TableMaintenanceAnalyzer {
    pub fn analyze_postgres(&self, stats: &TableStats) -> MaintenanceRecommendation {
        if let (Some(dead), Some(live)) = (stats.dead_tuples, stats.live_tuples) {
            let threshold = self.postgres_autovacuum_base as f64
                + (self.postgres_autovacuum_threshold * live as f64);
            if dead as f64 > threshold {
                return MaintenanceRecommendation::Vacuum;
            }
        }
        MaintenanceRecommendation::None
    }

    pub fn analyze_mysql(&self, stats: &TableStats) -> MaintenanceRecommendation {
        if let Some(free_bytes) = stats.data_free_bytes {
            if free_bytes >= self.mysql_min_free_bytes {
                let total = stats.table_size_bytes.saturating_add(free_bytes);
                if total > 0 {
                    let ratio = free_bytes as f64 / total as f64;
                    if ratio >= self.mysql_fragmentation_ratio {
                        return MaintenanceRecommendation::Optimize;
                    }
                }
            }
        }
        MaintenanceRecommendation::None
    }

    pub fn analyze_sqlite(&self, stats: &TableStats) -> MaintenanceRecommendation {
        if let (Some(free_pages), Some(page_size)) = (stats.freelist_pages, stats.page_size) {
            let free_bytes = free_pages.saturating_mul(page_size);
            let total = stats.table_size_bytes; // For SQLite, let's consider this size in bytes which usually includes free pages or doesn't depending on PRAGMA page_count.
                                                // Let's assume total_size = page_count * page_size.
            if total > 0 {
                let ratio = free_bytes as f64 / total as f64;
                if ratio >= self.sqlite_fragmentation_ratio {
                    return MaintenanceRecommendation::Vacuum;
                }
            }
        }
        MaintenanceRecommendation::None
    }

    pub fn build_pg_query() -> &'static str {
        "SELECT relname as table_name, \
                pg_relation_size(oid) as table_size_bytes, \
                n_dead_tup as dead_tuples, \
                n_live_tup as live_tuples \
         FROM pg_stat_user_tables;"
    }

    pub fn build_mysql_query(db_name: &str) -> String {
        format!(
            "SELECT table_name, data_length + index_length as table_size_bytes, data_free as data_free_bytes \
             FROM information_schema.tables \
             WHERE table_schema = '{}';",
            db_name
        )
    }

    pub fn build_sqlite_freelist_query() -> &'static str {
        "PRAGMA freelist_count;"
    }

    pub fn build_sqlite_page_size_query() -> &'static str {
        "PRAGMA page_size;"
    }

    pub fn build_sqlite_page_count_query() -> &'static str {
        "PRAGMA page_count;"
    }
}
