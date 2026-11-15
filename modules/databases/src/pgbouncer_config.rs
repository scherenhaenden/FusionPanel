#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolMode {
    Session,
    Transaction,
    Statement,
}

impl fmt::Display for PoolMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoolMode::Session => write!(f, "session"),
            PoolMode::Transaction => write!(f, "transaction"),
            PoolMode::Statement => write!(f, "statement"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgBouncerDatabase {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub auth_user: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgBouncerUser {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgBouncerConfig {
    pub databases: Vec<PgBouncerDatabase>,
    pub users: Vec<PgBouncerUser>,
    pub listen_addr: String,
    pub listen_port: u16,
    pub pool_mode: PoolMode,
    pub max_client_conn: u32,
    pub default_pool_size: u32,
}

impl PgBouncerConfig {
    pub fn generate_ini(&self) -> String {
        let mut ini = String::new();

        ini.push_str("[databases]\n");
        for db in &self.databases {
            ini.push_str(&format!("{} = host={} port={} dbname={}", db.name, db.host, db.port, db.dbname));
            if let Some(auth_user) = &db.auth_user {
                ini.push_str(&format!(" auth_user={}", auth_user));
            }
            ini.push('\n');
        }

        ini.push_str("\n[pgbouncer]\n");
        ini.push_str(&format!("listen_addr = {}\n", self.listen_addr));
        ini.push_str(&format!("listen_port = {}\n", self.listen_port));
        ini.push_str("auth_type = md5\n");
        ini.push_str("auth_file = /etc/pgbouncer/userlist.txt\n");
        ini.push_str(&format!("pool_mode = {}\n", self.pool_mode));
        ini.push_str(&format!("max_client_conn = {}\n", self.max_client_conn));
        ini.push_str(&format!("default_pool_size = {}\n", self.default_pool_size));
        ini.push_str("server_reset_query = DISCARD ALL\n");

        ini
    }

    pub fn generate_userlist(&self) -> String {
        let mut list = String::new();
        for user in &self.users {
            list.push_str(&format!("\"{}\" \"{}\"\n", user.username, user.password_hash));
        }
        list
    }
}
