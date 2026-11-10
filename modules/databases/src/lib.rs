#![forbid(unsafe_code)]

pub mod adapter;
pub mod binlog_retention;
pub mod health;
pub mod maintenance;
pub mod mariadb;
pub mod pool_telemetry;
pub mod postgres;
pub mod quota;
pub mod postgres_checkpointer;
pub mod postgres_limits;
pub mod postgres_replication;
pub mod postgres_replication_slots;
pub mod redis_cluster;
pub mod slow_query;
pub mod postgres_replication_lag;
