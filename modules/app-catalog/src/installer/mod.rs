#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

pub struct CatalogInstaller;

impl CatalogInstaller {
    pub fn new() -> Self {
        Self
    }

    pub fn render_compose(template: &str, config: &HashMap<String, String>) -> String {
        let mut result = template.to_string();

        let mut keys: Vec<&String> = config.keys().collect();
        // Sort keys by length descending to prevent partial prefix replacement
        keys.sort_by(|a, b| b.len().cmp(&a.len()));

        for k in keys {
            let v = config.get(k).unwrap();
            let var_1 = format!("${{{}}}", k);
            let var_2 = format!("${}", k);
            result = result.replace(&var_1, v);
            result = result.replace(&var_2, v);
        }
        result
    }

    pub fn allocate_port(min: u16, max: u16, used_ports: &[u16]) -> Option<u16> {
        for port in min..=max {
            if !used_ports.contains(&port) {
                return Some(port);
            }
        }
        None
    }

    pub fn generate_password(length: usize) -> Result<String, std::io::Error> {
        let mut file = File::open("/dev/urandom")?;
        let mut buf = vec![0u8; length];
        file.read_exact(&mut buf)?;

        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let password: String = buf
            .into_iter()
            .map(|b| CHARSET[(b as usize) % CHARSET.len()] as char)
            .collect();

        Ok(password)
    }
}

impl Default for CatalogInstaller {
    fn default() -> Self {
        Self::new()
    }
}
