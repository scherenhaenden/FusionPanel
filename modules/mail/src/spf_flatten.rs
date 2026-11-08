#![forbid(unsafe_code)]

use std::collections::HashSet;

pub trait DnsResolver {
    fn resolve_txt(&self, domain: &str) -> Result<Vec<String>, String>;
    fn resolve_a(&self, domain: &str) -> Result<Vec<String>, String>;
    fn resolve_aaaa(&self, domain: &str) -> Result<Vec<String>, String>;
    fn resolve_mx(&self, domain: &str) -> Result<Vec<String>, String>;
}

pub struct SpfFlattener<'a> {
    resolver: &'a dyn DnsResolver,
}

impl<'a> SpfFlattener<'a> {
    pub fn new(resolver: &'a dyn DnsResolver) -> Self {
        Self { resolver }
    }

    pub fn flatten(&self, domain: &str) -> Result<String, String> {
        let mut ips_v4 = HashSet::new();
        let mut ips_v6 = HashSet::new();
        let mut visited = HashSet::new();
        let mut lookups = 0;
        let mut final_qualifier = "-all".to_string();

        self.flatten_recursive(
            domain,
            domain,
            &mut ips_v4,
            &mut ips_v6,
            &mut visited,
            &mut lookups,
            &mut final_qualifier,
        )?;

        let mut parts = vec!["v=spf1".to_string()];

        let mut v4_sorted: Vec<_> = ips_v4.into_iter().collect();
        v4_sorted.sort();
        for ip in v4_sorted {
            parts.push(format!("ip4:{}", ip));
        }

        let mut v6_sorted: Vec<_> = ips_v6.into_iter().collect();
        v6_sorted.sort();
        for ip in v6_sorted {
            parts.push(format!("ip6:{}", ip));
        }

        parts.push(final_qualifier);

        Ok(parts.join(" "))
    }

    fn check_lookup_limit(lookups: &mut usize) -> Result<(), String> {
        *lookups += 1;
        if *lookups > 10 {
            Err("DNS lookup limit exceeded (RFC 7208)".to_string())
        } else {
            Ok(())
        }
    }

    fn flatten_recursive(
        &self,
        domain: &str,
        root_domain: &str,
        ips_v4: &mut HashSet<String>,
        ips_v6: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        lookups: &mut usize,
        final_qualifier: &mut String,
    ) -> Result<(), String> {
        if !visited.insert(domain.to_string()) {
            return Ok(()); // Avoid cycles
        }

        Self::check_lookup_limit(lookups)?;
        let txt_records = self.resolver.resolve_txt(domain)?;

        let spf_record = txt_records.iter().find(|r| r.starts_with("v=spf1"));

        if let Some(record) = spf_record {
            let terms = record.split_whitespace().skip(1);

            let mut has_redirect = false;
            let mut redirect_target = "";

            for term in terms {
                let mechanism = if term.starts_with('+')
                    || term.starts_with('-')
                    || term.starts_with('~')
                    || term.starts_with('?')
                {
                    &term[1..]
                } else {
                    term
                };

                // capture qualifier for the top domain
                if mechanism == "all" {
                    if domain == root_domain {
                        *final_qualifier = term.to_string();
                    }
                    continue;
                }

                let mut parts = mechanism.splitn(2, ':');
                let name_with_slash = parts.next().unwrap_or("").to_lowercase();
                let arg = parts.next().unwrap_or("");

                // name could be a/24 so we need to separate name from slash for 'a' and 'mx' without colon
                let mut name = name_with_slash.as_str();
                let mut implicit_cidr = "";
                if !name.starts_with("ip4")
                    && !name.starts_with("ip6")
                    && !name.starts_with("include")
                    && name.contains('/')
                {
                    let mut nparts = name.splitn(2, '/');
                    name = nparts.next().unwrap();
                    implicit_cidr = nparts.next().unwrap_or("");
                }

                match name {
                    "ip4" => {
                        ips_v4.insert(arg.to_string());
                    }
                    "ip6" => {
                        ips_v6.insert(arg.to_string());
                    }
                    "include" => {
                        self.flatten_recursive(
                            arg,
                            root_domain,
                            ips_v4,
                            ips_v6,
                            visited,
                            lookups,
                            final_qualifier,
                        )?;
                    }
                    "a" => {
                        let target = if arg.is_empty() { domain } else { arg };
                        let mut target_domain = target;
                        let mut cidr4 = "";
                        let mut cidr6 = "";

                        let parts: Vec<&str> = target.split('/').collect();
                        if !parts.is_empty() {
                            target_domain = parts[0];
                        }
                        if parts.len() == 2 {
                            cidr4 = parts[1];
                        }
                        if parts.len() == 3 {
                            cidr4 = parts[1];
                            cidr6 = parts[2];
                        }

                        if !implicit_cidr.is_empty() {
                            if implicit_cidr.contains('/') {
                                let p: Vec<&str> = implicit_cidr.split('/').collect();
                                cidr4 = p[0];
                                cidr6 = p[1];
                            } else {
                                cidr4 = implicit_cidr;
                            }
                        }

                        let cidr4_suffix = if !cidr4.is_empty() {
                            format!("/{}", cidr4)
                        } else {
                            "".to_string()
                        };
                        let cidr6_suffix = if !cidr6.is_empty() {
                            format!("/{}", cidr6)
                        } else {
                            "".to_string()
                        };

                        Self::check_lookup_limit(lookups)?;
                        if let Ok(a_records) = self.resolver.resolve_a(target_domain) {
                            for ip in a_records {
                                ips_v4.insert(format!("{}{}", ip, cidr4_suffix));
                            }
                        }

                        if let Ok(aaaa_records) = self.resolver.resolve_aaaa(target_domain) {
                            for ip in aaaa_records {
                                ips_v6.insert(format!("{}{}", ip, cidr6_suffix));
                            }
                        }
                    }
                    "mx" => {
                        let target = if arg.is_empty() { domain } else { arg };
                        let mut target_domain = target;
                        let mut cidr4 = "";
                        let mut cidr6 = "";

                        let parts: Vec<&str> = target.split('/').collect();
                        if !parts.is_empty() {
                            target_domain = parts[0];
                        }
                        if parts.len() == 2 {
                            cidr4 = parts[1];
                        }
                        if parts.len() == 3 {
                            cidr4 = parts[1];
                            cidr6 = parts[2];
                        }

                        if !implicit_cidr.is_empty() {
                            if implicit_cidr.contains('/') {
                                let p: Vec<&str> = implicit_cidr.split('/').collect();
                                cidr4 = p[0];
                                cidr6 = p[1];
                            } else {
                                cidr4 = implicit_cidr;
                            }
                        }

                        let cidr4_suffix = if !cidr4.is_empty() {
                            format!("/{}", cidr4)
                        } else {
                            "".to_string()
                        };
                        let cidr6_suffix = if !cidr6.is_empty() {
                            format!("/{}", cidr6)
                        } else {
                            "".to_string()
                        };

                        Self::check_lookup_limit(lookups)?;
                        if let Ok(mx_records) = self.resolver.resolve_mx(target_domain) {
                            for mx in mx_records {
                                if let Ok(a_records) = self.resolver.resolve_a(&mx) {
                                    for ip in a_records {
                                        ips_v4.insert(format!("{}{}", ip, cidr4_suffix));
                                    }
                                }
                                if let Ok(aaaa_records) = self.resolver.resolve_aaaa(&mx) {
                                    for ip in aaaa_records {
                                        ips_v6.insert(format!("{}{}", ip, cidr6_suffix));
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        let mut kv = mechanism.splitn(2, '=');
                        let key = kv.next().unwrap_or("").to_lowercase();
                        let val = kv.next().unwrap_or("");
                        if key == "redirect" {
                            has_redirect = true;
                            redirect_target = val;
                        }
                    }
                }
            }

            if has_redirect {
                self.flatten_recursive(
                    redirect_target,
                    root_domain,
                    ips_v4,
                    ips_v6,
                    visited,
                    lookups,
                    final_qualifier,
                )?;
            }
        }

        Ok(())
    }
}
