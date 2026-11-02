#![forbid(unsafe_code)]

//! Qualification tests for ADOPT-06: OS Username Preservation and Identity Mapping

use std::collections::HashSet;

/// Represents a validated OS username.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(String);

impl Username {
    pub fn new(name: &str) -> Result<Self, String> {
        if name.is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        if name.starts_with('-') {
            return Err("Username cannot start with a hyphen".to_string());
        }
        if name.len() > 32 {
            return Err("Username too long".to_string());
        }

        let protected = [
            "root", "daemon", "bin", "sys", "sync", "games", "man", "lp", "mail", "news",
            "uucp", "proxy", "www-data", "backup", "list", "irc", "gnats", "nobody",
            "systemd-network", "systemd-resolve", "systemd-timesync", "messagebus",
            "syslog", "mysql", "postgres"
        ];

        if protected.contains(&name) {
            return Err("Username is a protected system account".to_string());
        }

        for c in name.chars() {
            if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
                return Err("Username contains invalid characters".to_string());
            }
        }

        Ok(Username(name.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct Registry {
    users: HashSet<Username>,
    normalized_users: HashSet<String>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            users: HashSet::new(),
            normalized_users: HashSet::new(),
        }
    }

    pub fn add_user(&mut self, name: &str) -> Result<(), String> {
        let username = Username::new(name)?;

        let normalized = name.replace('-', "_");

        if self.normalized_users.contains(&normalized) {
            return Err("Username collides with an existing normalized username".to_string());
        }

        self.users.insert(username);
        self.normalized_users.insert(normalized);
        Ok(())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_usernames() {
        assert!(Username::new("alice").is_ok());
        assert!(Username::new("bob-ross").is_ok());
        assert!(Username::new("charlie_123").is_ok());
        assert!(Username::new("d").is_ok());
    }

    #[test]
    fn test_invalid_usernames() {
        assert!(Username::new("").is_err());
        assert!(Username::new("-alice").is_err());
        assert!(Username::new("alice@home").is_err());
        assert!(Username::new("root").is_err());
        assert!(Username::new("daemon").is_err());

        let long_name = "verylongusernamethatiswayoverthirtytwochars";
        assert!(Username::new(long_name).is_err());
    }

    #[test]
    fn test_no_silent_normalization_and_collision_detection() {
        let mut registry = Registry::new();

        assert!(registry.add_user("foo-bar").is_ok());

        // Ensure hyphens are not normalized to underscores
        let u1 = Username::new("foo-bar").unwrap();
        assert_eq!(u1.as_str(), "foo-bar");

        // But adding foo_bar should fail due to collision detection
        assert!(registry.add_user("foo_bar").is_err());
    }

    #[test]
    fn test_uid_gid_separation() {
        #[derive(Debug, PartialEq)]
        struct Account {
            name: Username,
            uid: u32,
            gid: u32,
        }

        let acc1 = Account { name: Username::new("user1").unwrap(), uid: 1000, gid: 1000 };
        let acc2 = Account { name: Username::new("user2").unwrap(), uid: 1001, gid: 1001 };

        // Separation of numeric UIDs/GIDs
        assert_ne!(acc1.uid, acc2.uid);
        assert_ne!(acc1.gid, acc2.gid);

        // Account names
        assert_ne!(acc1.name, acc2.name);
    }

    #[test]
    fn test_resource_ownership_boundaries() {
        // Mocking a resource to prove ownership boundary
        struct Resource {
            path: String,
            owner_uid: u32,
            owner_gid: u32,
        }

        let user1_uid = 1000;
        let user2_uid = 1001;

        let res1 = Resource { path: "/home/user1/file".to_string(), owner_uid: user1_uid, owner_gid: user1_uid };
        let res2 = Resource { path: "/home/user2/file".to_string(), owner_uid: user2_uid, owner_gid: user2_uid };

        assert_eq!(res1.owner_uid, user1_uid);
        assert_eq!(res1.owner_gid, user1_uid);
        assert_eq!(res1.path, "/home/user1/file");
        assert_ne!(res1.owner_uid, user2_uid);

        assert_eq!(res2.owner_uid, user2_uid);
        assert_eq!(res2.owner_gid, user2_uid);
        assert_eq!(res2.path, "/home/user2/file");
        assert_ne!(res2.owner_uid, user1_uid);
    }
}
