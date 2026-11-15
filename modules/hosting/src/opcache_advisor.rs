#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpcacheMetrics {
    pub used_memory: usize,
    pub free_memory: usize,
    pub wasted_memory: usize,
    pub interned_strings_usage: usize,
    pub jit_buffer_size: usize,
    pub jit_buffer_free: usize,
    pub hits: usize,
    pub misses: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpcacheAdvice {
    pub cache_hit_rate: f64,
    pub high_wasted_memory_restart: bool,
    pub recommended_memory_consumption: usize,
    pub recommended_jit_buffer_size: usize,
}

impl OpcacheMetrics {
    pub fn advise(&self) -> OpcacheAdvice {
        let total_requests = self.hits + self.misses;
        let cache_hit_rate = if total_requests == 0 {
            100.0
        } else {
            (self.hits as f64 * 100.0) / (total_requests as f64)
        };

        let total_memory = self.used_memory + self.free_memory + self.wasted_memory;

        let high_wasted_memory_restart = if total_memory == 0 {
            false
        } else {
            let wasted_percentage = (self.wasted_memory as f64 * 100.0) / (total_memory as f64);
            wasted_percentage > 5.0
        };

        // Recommend increasing memory if free is less than 10% of total
        let recommended_memory_consumption = if total_memory > 0 && (self.free_memory as f64) < (total_memory as f64 * 0.10) {
            total_memory + (total_memory / 2) // increase by 50%
        } else {
            total_memory
        };

        // Recommend increasing JIT buffer size if free is less than 10% of total
        let recommended_jit_buffer_size = if self.jit_buffer_size > 0 && (self.jit_buffer_free as f64) < (self.jit_buffer_size as f64 * 0.10) {
            self.jit_buffer_size + (self.jit_buffer_size / 2)
        } else {
            self.jit_buffer_size
        };

        OpcacheAdvice {
            cache_hit_rate,
            high_wasted_memory_restart,
            recommended_memory_consumption,
            recommended_jit_buffer_size,
        }
    }
}
