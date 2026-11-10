#![forbid(unsafe_code)]

use std::collections::HashSet;

/// Configuration builder for Nginx HTTP/3 QUIC directives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http3QuicConfig {
    pub port: u16,
    pub ipv6: bool,
    pub alt_svc_max_age: u32,
    pub ssl_early_data: bool,
}

impl Default for Http3QuicConfig {
    fn default() -> Self {
        Self {
            port: 443,
            ipv6: true,
            alt_svc_max_age: 86400,
            ssl_early_data: true,
        }
    }
}

impl Http3QuicConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn ipv6(mut self, ipv6: bool) -> Self {
        self.ipv6 = ipv6;
        self
    }

    pub fn alt_svc_max_age(mut self, age: u32) -> Self {
        self.alt_svc_max_age = age;
        self
    }

    pub fn ssl_early_data(mut self, enable: bool) -> Self {
        self.ssl_early_data = enable;
        self
    }

    /// Generates the Nginx directives for HTTP/3 QUIC.
    /// Uses the guard to determine if `reuseport` should be appended to avoid conflicts.
    pub fn generate_directives(&self, guard: &mut UdpListenerGuard) -> String {
        let mut out = String::new();

        let ipv4_reuse = if guard.reserve_reuseport(false, self.port) { " reuseport" } else { "" };
        out.push_str(&format!("listen {} quic{};\n", self.port, ipv4_reuse));

        if self.ipv6 {
            let ipv6_reuse = if guard.reserve_reuseport(true, self.port) { " reuseport" } else { "" };
            out.push_str(&format!("listen [::]:{} quic{};\n", self.port, ipv6_reuse));
        }

        out.push_str(&format!(
            "add_header Alt-Svc 'h3=\":{}\"; ma={}' always;\n",
            self.port, self.alt_svc_max_age
        ));

        if self.ssl_early_data {
            out.push_str("ssl_early_data on;\n");
        }

        out
    }
}

/// Tracks assigned UDP listeners for QUIC to prevent port conflicts and ensure safety.
/// In Nginx, `reuseport` can only be defined once per IP:port combination.
#[derive(Debug, Default)]
pub struct UdpListenerGuard {
    reuseport_assigned: HashSet<(bool, u16)>,
}

impl UdpListenerGuard {
    pub fn new() -> Self {
        Self {
            reuseport_assigned: HashSet::new(),
        }
    }

    /// Attempts to reserve the `reuseport` flag for a specific IP type and port.
    /// Returns `true` if successfully reserved (meaning `reuseport` can be safely used).
    /// Returns `false` if it was already reserved (meaning `reuseport` would cause a conflict).
    pub fn reserve_reuseport(&mut self, is_ipv6: bool, port: u16) -> bool {
        self.reuseport_assigned.insert((is_ipv6, port))
    }

    /// Releases the `reuseport` reservation.
    pub fn release_reuseport(&mut self, is_ipv6: bool, port: u16) {
        self.reuseport_assigned.remove(&(is_ipv6, port));
    }

    /// Clears all reservations.
    pub fn clear(&mut self) {
        self.reuseport_assigned.clear();
    }
}
