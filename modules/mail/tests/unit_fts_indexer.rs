#![forbid(unsafe_code)]

use mail::fts_indexer::{FtsFlatcurveConfig, FtsOptimizer, MailboxIndexFootprint};

#[test]
fn test_default_config_generation() {
    let config = FtsFlatcurveConfig::default();
    let generated = config.generate_config();
    assert!(generated.contains("plugin {"));
    assert!(generated.contains("fts = flatcurve"));
    assert!(generated.contains("fts_flatcurve = substring_search=yes"));
    assert!(generated.contains("fts_autoindex = yes"));
    assert!(generated.contains("fts_filters = normalizer-icu snowball stopwords"));
    assert!(generated.contains("fts_tokenizer = generic max-length=255 min-length=3"));
    assert!(generated.contains("}"));
}

#[test]
fn test_custom_config_generation() {
    let config = FtsFlatcurveConfig::new(2, 128, vec!["normalizer-icu".to_string(), "snowball".to_string()])
        .with_substring_search(false);
    let generated = config.generate_config();
    assert!(generated.contains("fts_flatcurve = substring_search=no"));
    assert!(generated.contains("fts_filters = normalizer-icu snowball"));
    assert!(generated.contains("fts_tokenizer = generic max-length=128 min-length=2"));
}

#[test]
fn test_config_generation_no_filters() {
    let config = FtsFlatcurveConfig::new(3, 255, vec![]);
    let generated = config.generate_config();
    assert!(!generated.contains("fts_filters ="));
    assert!(generated.contains("fts_tokenizer = generic max-length=255 min-length=3"));
}

#[test]
fn test_optimization_ratio_calculation() {
    let footprint = MailboxIndexFootprint::new(1000, 200, 10);
    assert_eq!(footprint.optimization_ratio(), 0.2);

    let footprint_zero = MailboxIndexFootprint::new(0, 0, 0);
    assert_eq!(footprint_zero.optimization_ratio(), 0.0);
}

#[test]
fn test_fts_optimizer_plan_should_optimize() {
    let optimizer = FtsOptimizer::new(0.1, 100);
    let footprint = MailboxIndexFootprint::new(1000, 150, 5);
    let plan = optimizer.plan_optimization(&footprint);
    assert!(plan.should_optimize);
    assert_eq!(plan.estimated_freed_bytes, 150);
    assert!(plan.prune_stale_tokens);
}

#[test]
fn test_fts_optimizer_plan_below_ratio() {
    let optimizer = FtsOptimizer::new(0.2, 100);
    let footprint = MailboxIndexFootprint::new(1000, 150, 5);
    let plan = optimizer.plan_optimization(&footprint);
    assert!(!plan.should_optimize);
    assert_eq!(plan.estimated_freed_bytes, 0);
    assert!(!plan.prune_stale_tokens);
}

#[test]
fn test_fts_optimizer_plan_below_min_bytes() {
    let optimizer = FtsOptimizer::new(0.1, 200);
    let footprint = MailboxIndexFootprint::new(1000, 150, 5);
    let plan = optimizer.plan_optimization(&footprint);
    assert!(!plan.should_optimize);
    assert_eq!(plan.estimated_freed_bytes, 0);
    assert!(!plan.prune_stale_tokens);
}
