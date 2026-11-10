#![forbid(unsafe_code)]

pub mod adapter;
pub mod binlog_retention;
pub mod health;
pub mod maintenance;
pub mod mariadb;
pub mod pool_telemetry;
pub mod postgres;
pub mod postgres_limits;
pub mod postgres_replication;
pub mod postgres_replication_lag;
pub mod postgres_replication_slots;
pub mod quota;
pub mod redis_cluster;
pub mod slow_query;
pub mod stat_statements_analyzer;
pub use stat_statements_analyzer::*;
