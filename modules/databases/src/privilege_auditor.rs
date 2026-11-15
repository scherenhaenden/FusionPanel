#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq)]
pub struct Grant {
    pub privileges: Vec<String>,
    pub object: String,
    pub user: String,
    pub host: String,
    pub has_grant_option: bool,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrivilegeViolation {
    pub violation_type: String,
    pub description: String,
    pub raw_grant: String,
}

impl Grant {
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if !raw.to_uppercase().starts_with("GRANT ") {
            return None;
        }

        let on_idx = find_outside_quotes(raw, " ON ")?;
        let privs_str = &raw[6..on_idx].trim();

        let after_on = &raw[on_idx + 4..];

        let to_idx = find_outside_quotes(after_on, " TO ")?;
        let object_str = &after_on[..to_idx].trim();

        let after_to = &after_on[to_idx + 4..];

        let has_grant_option = find_outside_quotes(after_to, "WITH GRANT OPTION").is_some()
            || find_outside_quotes(after_to, " WITH GRANT OPTION").is_some();

        let mut end_of_user_host = after_to.len();
        if let Some(idx) = find_outside_quotes(after_to, " IDENTIFIED ") {
            end_of_user_host = end_of_user_host.min(idx);
        }
        if let Some(idx) = find_outside_quotes(after_to, " REQUIRE ") {
            end_of_user_host = end_of_user_host.min(idx);
        }
        if let Some(idx) = find_outside_quotes(after_to, " WITH ") {
            end_of_user_host = end_of_user_host.min(idx);
        }

        let user_host_str = after_to[..end_of_user_host].trim();

        let user;
        let mut host = String::new();

        if let Some(at_idx) = find_outside_quotes(user_host_str, "@") {
            user = unquote(user_host_str[..at_idx].trim());
            host = unquote(user_host_str[at_idx + 1..].trim());
        } else {
            user = unquote(user_host_str);
        }

        let mut privileges = Vec::new();
        let privs_upper = privs_str.to_uppercase();
        if privs_upper == "ALL PRIVILEGES" || privs_upper == "ALL" {
            privileges.push("ALL PRIVILEGES".to_string());
        } else {
            let mut current = String::new();
            let mut in_parens = 0;
            for c in privs_str.chars() {
                if c == '(' {
                    in_parens += 1;
                    current.push(c);
                } else if c == ')' {
                    in_parens -= 1;
                    current.push(c);
                } else if c == ',' && in_parens <= 0 {
                    privileges.push(current.trim().to_uppercase());
                    current.clear();
                } else {
                    current.push(c);
                }
            }
            if !current.trim().is_empty() {
                privileges.push(current.trim().to_uppercase());
            }
        }

        Some(Grant {
            privileges,
            object: object_str.to_string(),
            user,
            host,
            has_grant_option,
            raw: raw.to_string(),
        })
    }
}

pub struct PrivilegeAuditor;

impl PrivilegeAuditor {
    pub fn audit(grants: &[Grant]) -> Vec<PrivilegeViolation> {
        let mut violations = Vec::new();

        for grant in grants {
            if grant.host == "%" {
                violations.push(PrivilegeViolation {
                    violation_type: "WildcardHost".to_string(),
                    description: "Host is set to global wildcard '%'".to_string(),
                    raw_grant: grant.raw.clone(),
                });
            } else if grant.host.contains('%') {
                violations.push(PrivilegeViolation {
                    violation_type: "PartialWildcardHost".to_string(),
                    description: format!("Host contains wildcard: {}", grant.host),
                    raw_grant: grant.raw.clone(),
                });
            }

            if grant.has_grant_option {
                violations.push(PrivilegeViolation {
                    violation_type: "GrantOption".to_string(),
                    description: "User has GRANT OPTION privilege".to_string(),
                    raw_grant: grant.raw.clone(),
                });
            }

            for priv_name in &grant.privileges {
                if priv_name == "ALL PRIVILEGES" || priv_name == "ALL" {
                    if grant.object == "*.*" {
                        violations.push(PrivilegeViolation {
                            violation_type: "GlobalAllPrivileges".to_string(),
                            description: "User has ALL PRIVILEGES globally".to_string(),
                            raw_grant: grant.raw.clone(),
                        });
                    }
                    continue;
                }

                let p = priv_name.to_uppercase();
                if p.contains("SUPER") {
                    violations.push(PrivilegeViolation {
                        violation_type: "DangerousPrivilege".to_string(),
                        description: "User has SUPER privilege".to_string(),
                        raw_grant: grant.raw.clone(),
                    });
                }
                if p.contains("FILE") {
                    violations.push(PrivilegeViolation {
                        violation_type: "DangerousPrivilege".to_string(),
                        description: "User has FILE privilege".to_string(),
                        raw_grant: grant.raw.clone(),
                    });
                }
                if p.contains("SHUTDOWN") {
                    violations.push(PrivilegeViolation {
                        violation_type: "DangerousPrivilege".to_string(),
                        description: "User has SHUTDOWN privilege".to_string(),
                        raw_grant: grant.raw.clone(),
                    });
                }
            }
        }

        violations
    }

    pub fn parse_and_audit(raw_grants: &[&str]) -> Vec<PrivilegeViolation> {
        let grants: Vec<Grant> = raw_grants.iter().filter_map(|&s| Grant::parse(s)).collect();
        Self::audit(&grants)
    }
}

fn find_outside_quotes(s: &str, needle: &str) -> Option<usize> {
    let mut in_quote = None;
    let mut i = 0;
    let chars: Vec<char> = s.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();

    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = in_quote {
            if c == q {
                in_quote = None;
            } else if c == '\\' && i + 1 < chars.len() {
                i += 1;
            }
        } else {
            if c == '\'' || c == '"' || c == '`' {
                in_quote = Some(c);
            } else {
                if chars.len() - i >= needle_chars.len() {
                    let mut is_match = true;
                    for j in 0..needle_chars.len() {
                        if chars[i + j].to_ascii_uppercase() != needle_chars[j].to_ascii_uppercase() {
                            is_match = false;
                            break;
                        }
                    }
                    if is_match {
                        let byte_offset = s.chars().take(i).map(|ch| ch.len_utf8()).sum();
                        return Some(byte_offset);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn unquote(s: &str) -> String {
    if s.len() >= 2 {
        let first = s.chars().next().unwrap();
        let last = s.chars().last().unwrap();
        if (first == '\'' && last == '\'') || (first == '"' && last == '"') || (first == '`' && last == '`') {
            return s[1..s.len()-1].to_string();
        }
    }
    s.to_string()
}
