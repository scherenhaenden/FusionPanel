#![forbid(unsafe_code)]

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HestiaSiteMetadata {
    pub domain: String,
    pub aliases: Vec<String>,
    pub ssl_enabled: bool,
    pub php_version: Option<String>,
    pub web_backend: String,
}

pub struct HestiaCpImporter;

impl HestiaCpImporter {
    pub fn parse_file<P: AsRef<Path>>(path: P) -> io::Result<Vec<HestiaSiteMetadata>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut sites = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(site) = Self::parse_line(line) {
                sites.push(site);
            }
        }

        Ok(sites)
    }

    pub fn parse_line(line: &str) -> Option<HestiaSiteMetadata> {
        let mut domain = String::new();
        let mut aliases = Vec::new();
        let mut ssl_enabled = false;
        let mut php_version = None;
        let mut web_backend = String::new();
        let mut _docroot = String::new(); 

        let mut current_key = String::new();
        let mut current_value = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '=' && !in_quotes {
                if let Some(&'\'') = chars.peek() {
                    chars.next(); // consume quote
                    in_quotes = true;
                }
            } else if c == '\'' && in_quotes {
                in_quotes = false;
                
                match current_key.as_str() {
                    "DOMAIN" => domain = current_value.clone(),
                    "ALIAS" => {
                        if !current_value.is_empty() {
                            aliases = current_value.split(',').map(|s| s.trim().to_string()).collect();
                        }
                    }
                    "SSL" => ssl_enabled = current_value == "yes",
                    "PHP_VERSION" => php_version = Some(current_value.clone()),
                    "WEB_BACKEND" | "BACKEND" => web_backend = current_value.clone(),
                    "DOCROOT" => _docroot = current_value.clone(),
                    _ => {}
                }
                
                current_key.clear();
                current_value.clear();

                // Skip spaces between pairs
                while let Some(&' ') = chars.peek() {
                    chars.next();
                }
            } else if in_quotes {
                current_value.push(c);
            } else {
                current_key.push(c);
            }
        }

        if domain.is_empty() {
            return None;
        }

        if !Self::is_valid_domain(&domain) {
            return None;
        }

        Some(HestiaSiteMetadata {
            domain,
            aliases,
            ssl_enabled,
            php_version,
            web_backend,
        })
    }

    fn is_valid_domain(domain: &str) -> bool {
        if domain.contains("..") || domain.contains('/') || domain.contains('\\') || domain.contains('\0') {
            return false;
        }
        for c in domain.chars() {
            if c.is_control() {
                return false;
            }
        }
        true
    }
}
