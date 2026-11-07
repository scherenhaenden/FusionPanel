#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum AliasError {
    #[error("Cyclic forwarding loop detected involving: {0}")]
    CyclicLoop(String),
    #[error("Invalid email address: {0}")]
    InvalidAddress(String),
    #[error("Catch-all domain mismatch: rule for {0} cannot route to {1}")]
    DomainMismatch(String, String),
    #[error("Empty destination list for alias: {0}")]
    EmptyDestination(String),
}

/// A parsed email address containing local part and domain part.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct EmailAddress {
    pub local_part: String,
    pub domain: String,
}

impl EmailAddress {
    pub fn parse(email: &str) -> Result<Self, AliasError> {
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(AliasError::InvalidAddress(email.to_string()));
        }
        Ok(Self {
            local_part: parts[0].to_lowercase(),
            domain: parts[1].to_lowercase(),
        })
    }
}

impl std::fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.local_part, self.domain)
    }
}

pub struct AliasRouter {
    /// Maps full email address -> list of destination addresses
    pub aliases: HashMap<EmailAddress, Vec<EmailAddress>>,
    /// Maps domain name -> list of destination addresses (catch-alls)
    pub catch_alls: HashMap<String, Vec<EmailAddress>>,
}

impl Default for AliasRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl AliasRouter {
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
            catch_alls: HashMap::new(),
        }
    }

    /// Adds a forwarding alias mapping a source to one or more destinations.
    pub fn add_alias(&mut self, source: &str, destinations: Vec<&str>) -> Result<(), AliasError> {
        let src = EmailAddress::parse(source)?;
        if destinations.is_empty() {
            return Err(AliasError::EmptyDestination(source.to_string()));
        }
        let mut dests = Vec::new();
        for dest in destinations {
            dests.push(EmailAddress::parse(dest)?);
        }

        // Ensure no loops after insertion
        let previous = self.aliases.insert(src.clone(), dests.clone());
        if let Err(e) = self.check_loops() {
            if let Some(prev) = previous {
                self.aliases.insert(src, prev);
            } else {
                self.aliases.remove(&src);
            }
            return Err(e);
        }

        Ok(())
    }

    /// Adds a catch-all rule for a domain.
    pub fn add_catch_all(
        &mut self,
        domain: &str,
        destinations: Vec<&str>,
    ) -> Result<(), AliasError> {
        let domain_lower = domain.to_lowercase();
        if destinations.is_empty() {
            return Err(AliasError::EmptyDestination(format!("@{}", domain)));
        }

        let mut dests = Vec::new();
        for dest in destinations {
            let dest_addr = EmailAddress::parse(dest)?;
            // Enforce domain boundary checks: A catch-all for domain X can only route to addresses within domain X
            if dest_addr.domain != domain_lower {
                return Err(AliasError::DomainMismatch(
                    domain_lower.clone(),
                    dest_addr.domain.clone(),
                ));
            }
            dests.push(dest_addr);
        }

        let previous = self.catch_alls.insert(domain_lower.clone(), dests.clone());
        // Verify no loops introduced by catch-all
        if let Err(e) = self.check_loops() {
            if let Some(prev) = previous {
                self.catch_alls.insert(domain_lower, prev);
            } else {
                self.catch_alls.remove(&domain_lower);
            }
            return Err(e);
        }

        Ok(())
    }

    /// Resolve aliases for a given address.
    pub fn resolve(&self, address: &EmailAddress) -> Vec<EmailAddress> {
        if let Some(dests) = self.aliases.get(address) {
            return dests.clone();
        }
        if let Some(dests) = self.catch_alls.get(&address.domain) {
            return dests.clone();
        }
        vec![address.clone()]
    }

    /// Recursively resolve aliases to check for cyclic loops across all defined rules.
    fn check_loops(&self) -> Result<(), AliasError> {
        for start in self.aliases.keys() {
            self.check_loop_from(start)?;
        }
        // Check loops that might originate from a catch-all
        for dests in self.catch_alls.values() {
            for dest in dests {
                self.check_loop_from(dest)?;
            }
        }
        Ok(())
    }

    fn check_loop_from(&self, start: &EmailAddress) -> Result<(), AliasError> {
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        self.dfs(start, &mut visited, &mut path)
    }

    fn dfs(
        &self,
        current: &EmailAddress,
        visited: &mut HashSet<EmailAddress>,
        path: &mut Vec<EmailAddress>,
    ) -> Result<(), AliasError> {
        if path.contains(current) {
            // Loop detected
            // Return error with the first duplicate item we encountered as the loop entry
            return Err(AliasError::CyclicLoop(current.to_string()));
        }
        if visited.contains(current) {
            return Ok(());
        }

        path.push(current.clone());
        visited.insert(current.clone());

        if let Some(dests) = self.aliases.get(current) {
            for dest in dests {
                self.dfs(dest, visited, path)?;
            }
        } else if let Some(dests) = self.catch_alls.get(&current.domain) {
            for dest in dests {
                if self.aliases.contains_key(dest) {
                    self.dfs(dest, visited, path)?;
                }
            }
        }

        path.pop();
        Ok(())
    }

    pub fn render_table(&self) -> String {
        let mut lines = Vec::new();
        let mut sources: Vec<_> = self.aliases.keys().collect();
        sources.sort_by_key(|a| a.to_string());

        for src in sources {
            let dests = &self.aliases[src];
            let dest_strs: Vec<_> = dests.iter().map(|d| d.to_string()).collect();
            lines.push(format!("{}: {}", src, dest_strs.join(", ")));
        }

        let mut catch_all_domains: Vec<_> = self.catch_alls.keys().collect();
        catch_all_domains.sort();
        for dom in catch_all_domains {
            let dests = &self.catch_alls[dom];
            let dest_strs: Vec<_> = dests.iter().map(|d| d.to_string()).collect();
            lines.push(format!("@*: {} -> {}", dom, dest_strs.join(", ")));
        }

        lines.join("\n")
    }
}
