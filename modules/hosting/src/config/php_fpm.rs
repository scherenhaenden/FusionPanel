use crate::domain::{error::DomainError, error::Result, HostingSpec, SiteConfig};

pub fn generate_php_fpm_config(site: &SiteConfig) -> Result<String> {
    let php_spec = match &site.spec {
        HostingSpec::Php(p) => p,
        _ => {
            return Err(DomainError::InvalidInput(
                "Site is not configured for PHP".into(),
            ))
        }
    };

    let site_id = &site.id.0;
    let pool_name = php_spec.pool_name.as_str();
    let memory_limit = php_spec.memory_limit.as_str();
    let root = site.document_root.as_str();

    let user = format!("tenant_{}", site.tenant_id.0);

    let config = format!(
        "[{}]\n\
         user = {}\n\
         group = {}\n\
         listen = /run/php/php-fpm-{}.sock\n\
         listen.owner = {}\n\
         listen.group = {}\n\
         listen.mode = 0660\n\
         pm = dynamic\n\
         pm.max_children = 5\n\
         pm.start_servers = 2\n\
         pm.min_spare_servers = 1\n\
         pm.max_spare_servers = 3\n\
         php_admin_value[memory_limit] = {}\n\
         php_admin_value[open_basedir] = {}:/tmp\n",
        pool_name, user, user, site_id, user, user, memory_limit, root
    );

    Ok(config)
}
