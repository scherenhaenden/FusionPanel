#![forbid(unsafe_code)]

use std::cmp;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum TunerError {
    #[error("Zero max children concurrency is invalid")]
    ZeroConcurrency,
    #[error("Buffer allocation ceiling exceeded")]
    BufferCeilingExceeded,
    #[error("Invalid CPU core count")]
    InvalidCoreCount,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FastcgiConfig {
    pub keepalive: u32,
    pub fastcgi_buffer_size: String,
    pub fastcgi_buffers: String,
    pub fastcgi_busy_buffers_size: String,
    pub timeout_clamp: u32,
}

pub struct FastcgiKeepaliveTuner;

impl FastcgiKeepaliveTuner {
    /// Computes optimal keepalive and buffer sizes for Nginx reverse proxies
    ///
    /// Arguments:
    /// * `max_children` - The PHP-FPM worker concurrency (pm.max_children)
    /// * `cpu_cores` - The number of CPU cores available
    /// * `throughput` - Expected request throughput (requests/second)
    pub fn compute_nginx(
        max_children: u32,
        cpu_cores: u32,
        throughput: u32,
    ) -> Result<FastcgiConfig, TunerError> {
        if max_children == 0 {
            return Err(TunerError::ZeroConcurrency);
        }
        if cpu_cores == 0 {
            return Err(TunerError::InvalidCoreCount);
        }

        // Keepalive pool size heuristic
        // Normally, set keepalive to about 10-25% of max_children, or based on throughput
        let base_keepalive = (max_children as f32 * 0.2) as u32;
        let throughput_keepalive = cmp::max(1, throughput / 10);
        let core_multiplier = cmp::max(1, cpu_cores / 2);

        let keepalive = cmp::min(
            max_children, // Never exceed max_children
            cmp::max(
                cmp::max(base_keepalive, throughput_keepalive),
                core_multiplier * 4
            )
        );

        // Ensure minimum keepalive
        let keepalive = cmp::max(1, keepalive);

        // Buffer calculation
        // Default values for standard small/medium workloads
        let mut buffer_size_kb = 4;
        let mut num_buffers = 8;

        if throughput > 500 || max_children > 50 {
            buffer_size_kb = 8;
            num_buffers = 16;
        }

        if throughput > 1000 || max_children > 100 {
            buffer_size_kb = 16;
            num_buffers = 16;
        }

        if throughput > 5000 || max_children > 500 {
            buffer_size_kb = 32;
            num_buffers = 32;
        }

        // Buffer allocation ceiling check
        // Nginx fastcgi_buffers <number> <size>
        let total_buffer_bytes = (buffer_size_kb * 1024) * num_buffers;
        // Limit to 4MB total buffers per connection to avoid memory exhaustion
        if total_buffer_bytes > (4 * 1024 * 1024) {
            return Err(TunerError::BufferCeilingExceeded);
        }

        let fastcgi_buffer_size = format!("{}k", buffer_size_kb);
        let fastcgi_buffers = format!("{} {}k", num_buffers, buffer_size_kb);
        // Busy buffers size is typically 2x the buffer_size
        let fastcgi_busy_buffers_size = format!("{}k", buffer_size_kb * 2);

        // Timeout clamps based on concurrency vs throughput risk
        let timeout_clamp = if throughput > max_children * 10 {
            10 // High pressure, short timeout
        } else if throughput > max_children * 2 {
            30
        } else {
            60 // Standard PHP timeout clamp
        };

        Ok(FastcgiConfig {
            keepalive,
            fastcgi_buffer_size,
            fastcgi_buffers,
            fastcgi_busy_buffers_size,
            timeout_clamp,
        })
    }

    /// Emits the Nginx upstream block configuration snippet
    pub fn emit_nginx_upstream_snippet(upstream_name: &str, config: &FastcgiConfig) -> String {
        format!(
            "upstream {} {{\n\
            \x20\x20\x20\x20# PHP-FPM Backend\n\
            \x20\x20\x20\x20server unix:/run/php/php-fpm.sock;\n\
            \x20\x20\x20\x20keepalive {};\n\
            }}\n",
            upstream_name, config.keepalive
        )
    }

    /// Emits the Nginx location block configuration snippet
    pub fn emit_nginx_location_snippet(config: &FastcgiConfig) -> String {
        format!(
            "fastcgi_keep_conn on;\n\
            fastcgi_buffer_size {};\n\
            fastcgi_buffers {};\n\
            fastcgi_busy_buffers_size {};\n\
            fastcgi_read_timeout {}s;\n",
            config.fastcgi_buffer_size,
            config.fastcgi_buffers,
            config.fastcgi_busy_buffers_size,
            config.timeout_clamp
        )
    }

    /// Emits Apache proxy configuration snippet
    pub fn emit_apache_snippet(config: &FastcgiConfig) -> String {
        // Apache mod_proxy_fcgi uses enablereuse=on for keepalive
        // We use keepalive size as a hint for max connections if needed
        format!(
            "<Proxy \"fcgi://localhost/\" enablereuse=on max={}>\n\
            \x20\x20\x20\x20ProxySet timeout={}\n\
            </Proxy>\n",
            config.keepalive,
            config.timeout_clamp
        )
    }
}
