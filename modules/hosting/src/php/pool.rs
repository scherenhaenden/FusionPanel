#![forbid(unsafe_code)]

pub fn generate_pool(
    site_name: &str,
    user: &str,
    group: &str,
    document_root: &str,
    max_children: u32,
    start_servers: u32,
) -> String {
    format!(
        "[{site_name}]\n\
         user = {user}\n\
         group = {group}\n\
         listen = /run/php/php-fpm-{site_name}.sock\n\
         listen.owner = {user}\n\
         listen.group = {group}\n\
         listen.mode = 0660\n\
         pm = dynamic\n\
         pm.max_children = {max_children}\n\
         pm.start_servers = {start_servers}\n\
         pm.min_spare_servers = 1\n\
         pm.max_spare_servers = 3\n\
         php_admin_value[open_basedir] = {document_root}:/tmp\n\
         php_admin_flag[disable_functions] = exec,passthru,shell_exec,system\n"
    )
}
