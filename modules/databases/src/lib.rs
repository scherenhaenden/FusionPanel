#![forbid(unsafe_code)]

pub mod adapter;
pub mod health;
pub mod maintenance;
pub mod mariadb;
pub mod pool_telemetry;
pub mod postgres;
pub mod quota;
pub mod postgres_limits;
pub mod postgres_replication;
pub mod postgres_replication_slots;
pub mod slow_query;
