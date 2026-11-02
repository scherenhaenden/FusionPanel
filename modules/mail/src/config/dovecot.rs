// dovecot-sql.conf.ext configuration rendering

pub struct DovecotSqlConfig<'a> {
    pub driver: &'a str,
    pub connect: &'a str,
    pub default_pass_scheme: &'a str,
    pub password_query: &'a str,
    pub user_query: &'a str,
}

impl<'a> Default for DovecotSqlConfig<'a> {
    fn default() -> Self {
        Self {
            driver: "mysql",
            connect: "host=127.0.0.1 dbname=mail user=mail password=mail",
            default_pass_scheme: "ARGON2ID",
            password_query: "SELECT username, domain, password FROM users WHERE username = '%n' AND domain = '%d'",
            user_query: "SELECT maildir, 5000 AS uid, 5000 AS gid FROM users WHERE username = '%n' AND domain = '%d'",
        }
    }
}

pub fn render_dovecot_sql_conf(config: &DovecotSqlConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!("driver = {}\n", config.driver));
    out.push_str(&format!("connect = {}\n", config.connect));
    out.push_str(&format!(
        "default_pass_scheme = {}\n",
        config.default_pass_scheme
    ));
    out.push_str(&format!("password_query = {}\n", config.password_query));
    out.push_str(&format!("user_query = {}\n", config.user_query));
    out
}

pub fn render_dovecot_passdb_dict() -> String {
    // Basic static config block for passdb in dovecot.conf if needed, but
    // usually we'd just reference the sql conf
    "passdb {\n  driver = sql\n  args = /etc/dovecot/dovecot-sql.conf.ext\n}\n".to_string()
}
