#![forbid(unsafe_code)]

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticationResults {
    pub authserv_id: String,
    pub results: Vec<AuthResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthResult {
    pub method: String,
    pub result: String,
    pub reason: Option<String>,
    pub properties: Vec<AuthProperty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthProperty {
    pub ptype: String,
    pub property: String,
    pub value: String,
}

impl fmt::Display for AuthenticationResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Authentication-Results: {}", self.authserv_id)?;
        if self.results.is_empty() {
            write!(f, "; none")?;
        } else {
            for res in &self.results {
                write!(f, "; {}={}", res.method, res.result)?;
                if let Some(ref reason) = res.reason {
                    write!(f, " reason=\"{}\"", reason)?;
                }
                for prop in &res.properties {
                    write!(f, " {}.{}={}", prop.ptype, prop.property, prop.value)?;
                }
            }
        }
        Ok(())
    }
}

impl FromStr for AuthenticationResults {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let s = if s.to_lowercase().starts_with("authentication-results:") {
            &s["Authentication-Results:".len()..].trim()
        } else {
            s
        };

        if s.is_empty() {
            return Err("Empty Authentication-Results".to_string());
        }

        // We can split by ';' for multiple results, but need to be careful with ';' inside quotes.
        // Let's implement a simple state machine to parse correctly.
        let mut chars = s.chars().peekable();

        let mut authserv_id = String::new();
        while let Some(&c) = chars.peek() {
            if c == ';' {
                break;
            }
            authserv_id.push(c);
            chars.next();
        }
        authserv_id = authserv_id.trim().to_string();

        if authserv_id.is_empty() {
            return Err("Missing authserv-id".to_string());
        }

        let mut results = Vec::new();

        while chars.peek().is_some() {
            let mut part = String::new();
            let mut in_quotes = false;
            let mut in_parens = false;

            if chars.peek() == Some(&';') {
                chars.next();
            }

            while let Some(&c) = chars.peek() {
                if c == '"' {
                    in_quotes = !in_quotes;
                } else if c == '(' && !in_quotes {
                    in_parens = true;
                } else if c == ')' && !in_quotes {
                    in_parens = false;
                } else if c == ';' && !in_quotes && !in_parens {
                    break;
                }
                part.push(c);
                chars.next();
            }

            let part = part.trim();
            if part.is_empty() || part == "none" {
                continue;
            }

            // Parse method result
            let mut tokens = Vec::new();
            let mut curr_token = String::new();
            let mut in_quotes = false;
            let mut in_parens = false;

            for c in part.chars() {
                if c == '"' {
                    in_quotes = !in_quotes;
                    curr_token.push(c);
                } else if c == '(' && !in_quotes {
                    in_parens = true;
                    // Ignore comments? RFC says comments are allowed. Let's just include them or strip them.
                    // For now, let's keep them in the token if we want or just let it break?
                    // Actually, let's treat spaces inside parens as NOT breaking tokens, or just strip comments.
                    // To keep it simple, spaces inside parens don't break token if we just want to ignore or keep it.
                    curr_token.push(c);
                } else if c == ')' && !in_quotes {
                    in_parens = false;
                    curr_token.push(c);
                } else if c.is_whitespace() && !in_quotes && !in_parens {
                    if !curr_token.is_empty() {
                        tokens.push(curr_token.clone());
                        curr_token.clear();
                    }
                } else {
                    curr_token.push(c);
                }
            }
            if !curr_token.is_empty() {
                tokens.push(curr_token);
            }

            if tokens.is_empty() {
                continue;
            }

            let method_res = &tokens[0];
            let (method, result) = if let Some((m, r)) = method_res.split_once('=') {
                (m.to_string(), r.to_string())
            } else {
                return Err(format!("Invalid method result format: {}", method_res));
            };

            let mut reason = None;
            let mut properties = Vec::new();

            for token in tokens.into_iter().skip(1) {
                if token.starts_with("reason=") {
                    let r = &token["reason=".len()..];
                    let r = r.trim_matches('"').to_string();
                    reason = Some(r);
                } else if let Some((left, right)) = token.split_once('=') {
                    if let Some((ptype, prop)) = left.split_once('.') {
                        properties.push(AuthProperty {
                            ptype: ptype.to_string(),
                            property: prop.to_string(),
                            value: right.trim_matches('"').to_string(),
                        });
                    }
                }
            }

            results.push(AuthResult {
                method,
                result,
                reason,
                properties,
            });
        }

        Ok(AuthenticationResults {
            authserv_id,
            results,
        })
    }
}
