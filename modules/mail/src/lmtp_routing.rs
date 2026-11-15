#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeliveryTarget {
    Lmtp { host: String, port: u16 },
    UnixSocket(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResolvedRoute {
    pub recipient: String,
    pub target: DeliveryTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingError {
    AliasLoopDetected(String),
    RecipientNotFound(String),
}

#[derive(Default)]
pub struct LmtpRouter {
    mailboxes: HashMap<String, DeliveryTarget>,
    aliases: HashMap<String, Vec<String>>,
}

impl LmtpRouter {
    pub fn new() -> Self {
        Self {
            mailboxes: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn add_mailbox(&mut self, address: String, target: DeliveryTarget) {
        self.mailboxes.insert(address, target);
    }

    pub fn add_alias(&mut self, alias: String, targets: Vec<String>) {
        self.aliases.insert(alias, targets);
    }

    pub fn resolve(&self, address: &str) -> Result<Vec<ResolvedRoute>, RoutingError> {
        let mut routes = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();

        self.resolve_recursive(address.to_string(), &mut visited, &mut stack, &mut routes)?;

        routes.sort();
        routes.dedup();

        Ok(routes)
    }

    fn resolve_recursive(
        &self,
        current: String,
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
        routes: &mut Vec<ResolvedRoute>,
    ) -> Result<(), RoutingError> {
        if stack.contains(&current) {
            return Err(RoutingError::AliasLoopDetected(current));
        }

        if visited.contains(&current) {
            return Ok(());
        }

        stack.push(current.clone());
        visited.insert(current.clone());

        if let Some(targets) = self.aliases.get(&current) {
            for target in targets {
                self.resolve_recursive(target.clone(), visited, stack, routes)?;
            }
        } else if let Some(target) = self.mailboxes.get(&current) {
            routes.push(ResolvedRoute {
                recipient: current.clone(),
                target: target.clone(),
            });
        } else {
            return Err(RoutingError::RecipientNotFound(current));
        }

        stack.pop();
        Ok(())
    }
}
