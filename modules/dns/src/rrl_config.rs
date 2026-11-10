#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RrlConfig {
    pub responses_per_second: u32,
    pub window: Option<u32>,
    pub slip: Option<u32>,
    pub ipv4_prefix_length: Option<u32>,
    pub ipv6_prefix_length: Option<u32>,
    pub exempt_clients: Vec<String>,
}

impl Default for RrlConfig {
    fn default() -> Self {
        Self {
            responses_per_second: 5,
            window: None,
            slip: None,
            ipv4_prefix_length: None,
            ipv6_prefix_length: None,
            exempt_clients: Vec::new(),
        }
    }
}

impl RrlConfig {
    pub fn new(responses_per_second: u32) -> Self {
        Self {
            responses_per_second,
            ..Default::default()
        }
    }

    pub fn with_window(mut self, window: u32) -> Self {
        self.window = Some(window);
        self
    }

    pub fn with_slip(mut self, slip: u32) -> Self {
        self.slip = Some(slip);
        self
    }

    pub fn with_ipv4_prefix_length(mut self, length: u32) -> Self {
        self.ipv4_prefix_length = Some(length);
        self
    }

    pub fn with_ipv6_prefix_length(mut self, length: u32) -> Self {
        self.ipv6_prefix_length = Some(length);
        self
    }

    pub fn add_exempt_client(mut self, client: impl Into<String>) -> Self {
        self.exempt_clients.push(client.into());
        self
    }

    pub fn generate(&self) -> String {
        let mut config = String::new();
        config.push_str("rate-limit {\n");
        config.push_str(&format!("    responses-per-second {};\n", self.responses_per_second));

        if let Some(window) = self.window {
            config.push_str(&format!("    window {};\n", window));
        }

        if let Some(slip) = self.slip {
            config.push_str(&format!("    slip {};\n", slip));
        }

        if let Some(ipv4) = self.ipv4_prefix_length {
            config.push_str(&format!("    ipv4-prefix-length {};\n", ipv4));
        }

        if let Some(ipv6) = self.ipv6_prefix_length {
            config.push_str(&format!("    ipv6-prefix-length {};\n", ipv6));
        }

        if !self.exempt_clients.is_empty() {
            config.push_str("    exempt-clients { ");
            for client in &self.exempt_clients {
                config.push_str(&format!("{}; ", client));
            }
            config.push_str("};\n");
        }

        config.push_str("};");
        config
    }
}
