#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhpTuning {
    pub opcache_memory_consumption: u32,
    pub revalidate_freq: u32,
    pub request_slowlog_timeout: String,
    pub trace_logging_depth: u32,
}

impl Default for PhpTuning {
    fn default() -> Self {
        Self {
            opcache_memory_consumption: 128,
            revalidate_freq: 2,
            request_slowlog_timeout: "5s".to_string(),
            trace_logging_depth: 20,
        }
    }
}

impl PhpTuning {
    pub fn new(
        opcache_memory_consumption: u32,
        revalidate_freq: u32,
        request_slowlog_timeout: impl Into<String>,
        trace_logging_depth: u32,
    ) -> Self {
        Self {
            opcache_memory_consumption,
            revalidate_freq,
            request_slowlog_timeout: request_slowlog_timeout.into(),
            trace_logging_depth,
        }
    }

    pub fn generate_php_ini_fragment(&self) -> String {
        format!(
            "opcache.memory_consumption={}\n\
             opcache.revalidate_freq={}\n",
            self.opcache_memory_consumption, self.revalidate_freq
        )
    }

    pub fn generate_pool_fragment(&self, site_name: &str) -> String {
        format!(
            "request_slowlog_timeout = {}\n\
             request_slowlog_trace_depth = {}\n\
             slowlog = /var/log/fusionpanel/{}-slow.log\n",
            self.request_slowlog_timeout, self.trace_logging_depth, site_name
        )
    }
}
