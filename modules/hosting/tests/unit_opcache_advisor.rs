#![forbid(unsafe_code)]

use hosting::opcache_advisor::{OpcacheMetrics};

#[test]
fn test_cache_hit_rate_normal() {
    let metrics = OpcacheMetrics {
        used_memory: 100,
        free_memory: 100,
        wasted_memory: 0,
        interned_strings_usage: 0,
        jit_buffer_size: 0,
        jit_buffer_free: 0,
        hits: 900,
        misses: 100,
    };
    let advice = metrics.advise();
    assert_eq!(advice.cache_hit_rate, 90.0);
}

#[test]
fn test_cache_hit_rate_zero_requests() {
    let metrics = OpcacheMetrics {
        used_memory: 100,
        free_memory: 100,
        wasted_memory: 0,
        interned_strings_usage: 0,
        jit_buffer_size: 0,
        jit_buffer_free: 0,
        hits: 0,
        misses: 0,
    };
    let advice = metrics.advise();
    assert_eq!(advice.cache_hit_rate, 100.0); // handled correctly
}

#[test]
fn test_high_wasted_memory_restart() {
    let mut metrics = OpcacheMetrics {
        used_memory: 4000,
        free_memory: 5000,
        wasted_memory: 1000, // 10% of 10000
        interned_strings_usage: 0,
        jit_buffer_size: 0,
        jit_buffer_free: 0,
        hits: 10,
        misses: 0,
    };

    let advice = metrics.advise();
    assert!(advice.high_wasted_memory_restart);

    // Below 5%
    metrics.wasted_memory = 400; // 400 / 9400 = 4.2%
    let advice = metrics.advise();
    assert!(!advice.high_wasted_memory_restart);
}

#[test]
fn test_recommend_memory_consumption() {
    // Normal, plenty of memory
    let metrics = OpcacheMetrics {
        used_memory: 4000,
        free_memory: 5000,
        wasted_memory: 1000,
        interned_strings_usage: 0,
        jit_buffer_size: 0,
        jit_buffer_free: 0,
        hits: 10,
        misses: 0,
    };
    let advice = metrics.advise();
    assert_eq!(advice.recommended_memory_consumption, 10000);

    // Low free memory, recommend increase
    let metrics = OpcacheMetrics {
        used_memory: 9000,
        free_memory: 500, // 500 / 10500 < 10%
        wasted_memory: 1000,
        interned_strings_usage: 0,
        jit_buffer_size: 0,
        jit_buffer_free: 0,
        hits: 10,
        misses: 0,
    };
    let advice = metrics.advise();
    assert_eq!(advice.recommended_memory_consumption, 10500 + 5250); // 15750
}

#[test]
fn test_recommend_jit_buffer_size() {
    // Normal, plenty of JIT space
    let metrics = OpcacheMetrics {
        used_memory: 0,
        free_memory: 0,
        wasted_memory: 0,
        interned_strings_usage: 0,
        jit_buffer_size: 10000,
        jit_buffer_free: 5000,
        hits: 0,
        misses: 0,
    };
    let advice = metrics.advise();
    assert_eq!(advice.recommended_jit_buffer_size, 10000);

    // Low JIT space, recommend increase
    let metrics = OpcacheMetrics {
        used_memory: 0,
        free_memory: 0,
        wasted_memory: 0,
        interned_strings_usage: 0,
        jit_buffer_size: 10000,
        jit_buffer_free: 500, // < 10%
        hits: 0,
        misses: 0,
    };
    let advice = metrics.advise();
    assert_eq!(advice.recommended_jit_buffer_size, 15000); // 10000 + 5000
}
