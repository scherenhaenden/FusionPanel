#![forbid(unsafe_code)]

use databases::innodb_advisor::*;

#[test]
fn test_parse_innodb_metrics_success() {
    let status_output = "
Innodb_buffer_pool_read_requests 1000000
Innodb_buffer_pool_reads 5000
Innodb_buffer_pool_pages_total 8192
Other_metric 123
";
    let metrics = parse_innodb_metrics(status_output).unwrap();
    assert_eq!(metrics.read_requests, 1000000);
    assert_eq!(metrics.reads, 5000);
    assert_eq!(metrics.pages_total, 8192);
}

#[test]
fn test_parse_innodb_metrics_missing() {
    let status_output = "
Innodb_buffer_pool_read_requests 1000000
Innodb_buffer_pool_pages_total 8192
";
    let metrics = parse_innodb_metrics(status_output);
    assert_eq!(metrics, None);
}

#[test]
fn test_calculate_hit_ratio() {
    let metrics = InnodbMetrics {
        read_requests: 10000,
        reads: 500,
        pages_total: 8192,
    };
    // 100 - (500 * 100 / 10000) = 100 - 5 = 95.0
    let hit_ratio = calculate_hit_ratio(&metrics);
    assert_eq!(hit_ratio, 95.0);
}

#[test]
fn test_calculate_hit_ratio_zero_reads() {
    let metrics = InnodbMetrics {
        read_requests: 10000,
        reads: 0,
        pages_total: 8192,
    };
    let hit_ratio = calculate_hit_ratio(&metrics);
    assert_eq!(hit_ratio, 100.0);
}

#[test]
fn test_calculate_hit_ratio_zero_read_requests() {
    let metrics = InnodbMetrics {
        read_requests: 0,
        reads: 0,
        pages_total: 8192,
    };
    let hit_ratio = calculate_hit_ratio(&metrics);
    assert_eq!(hit_ratio, 100.0);
}

#[test]
fn test_advise_buffer_pool_size_increase() {
    let advice = advise_buffer_pool_size(90.0, 16_000_000_000, 4_000_000_000);
    assert_eq!(advice, SizingAdvice::IncreaseBufferPool);
}

#[test]
fn test_advise_buffer_pool_size_optimal_low_ratio_no_memory() {
    // Low hit ratio, but not enough memory to recommend increase
    let advice = advise_buffer_pool_size(90.0, 6_000_000_000, 4_000_000_000);
    assert_eq!(advice, SizingAdvice::Optimal);
}

#[test]
fn test_advise_buffer_pool_size_decrease() {
    // High hit ratio, taking up a lot of memory
    let advice = advise_buffer_pool_size(99.99, 16_000_000_000, 10_000_000_000);
    assert_eq!(advice, SizingAdvice::DecreaseBufferPool);
}

#[test]
fn test_advise_buffer_pool_size_optimal_high_ratio_small_memory() {
    // High hit ratio, but not taking much memory, so optimal
    let advice = advise_buffer_pool_size(99.99, 16_000_000_000, 2_000_000_000);
    assert_eq!(advice, SizingAdvice::Optimal);
}

#[test]
fn test_advise_buffer_pool_size_optimal() {
    let advice = advise_buffer_pool_size(98.0, 16_000_000_000, 4_000_000_000);
    assert_eq!(advice, SizingAdvice::Optimal);
}

#[test]
fn test_evaluate_innodb_performance() {
    let status_output = "
Innodb_buffer_pool_read_requests 1000000
Innodb_buffer_pool_reads 50000
Innodb_buffer_pool_pages_total 8192
";
    // 100 - (50000 * 100 / 1000000) = 100 - 5 = 95.0
    let result = evaluate_innodb_performance(status_output, 16_000_000_000, 4_000_000_000).unwrap();
    assert_eq!(result.hit_ratio, 95.0);
    assert_eq!(result.advice, SizingAdvice::Optimal);
}
