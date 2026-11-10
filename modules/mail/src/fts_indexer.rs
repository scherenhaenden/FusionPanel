#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq)]
pub struct FtsFlatcurveConfig {
    pub min_token_length: usize,
    pub max_token_length: usize,
    pub tokenization_filters: Vec<String>,
    pub substring_search: bool,
}

impl Default for FtsFlatcurveConfig {
    fn default() -> Self {
        Self {
            min_token_length: 3,
            max_token_length: 255,
            tokenization_filters: vec!["normalizer-icu".to_string(), "snowball".to_string(), "stopwords".to_string()],
            substring_search: true,
        }
    }
}

impl FtsFlatcurveConfig {
    pub fn new(min_token_length: usize, max_token_length: usize, tokenization_filters: Vec<String>) -> Self {
        Self {
            min_token_length,
            max_token_length,
            tokenization_filters,
            substring_search: true,
        }
    }

    pub fn with_substring_search(mut self, enabled: bool) -> Self {
        self.substring_search = enabled;
        self
    }

    pub fn generate_config(&self) -> String {
        let mut config = String::new();
        config.push_str("plugin {\n");
        config.push_str("  fts = flatcurve\n");

        if self.substring_search {
            config.push_str("  fts_flatcurve = substring_search=yes\n");
        } else {
            config.push_str("  fts_flatcurve = substring_search=no\n");
        }

        config.push_str("  fts_autoindex = yes\n");

        if !self.tokenization_filters.is_empty() {
            let filters = self.tokenization_filters.join(" ");
            config.push_str(&format!("  fts_filters = {}\n", filters));
        }

        config.push_str(&format!(
            "  fts_tokenizer = generic max-length={} min-length={}\n",
            self.max_token_length, self.min_token_length
        ));

        config.push_str("}\n");
        config
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MailboxIndexFootprint {
    pub total_size_bytes: u64,
    pub stale_tokens_size_bytes: u64,
    pub stale_tokens_count: u64,
}

impl MailboxIndexFootprint {
    pub fn new(total_size_bytes: u64, stale_tokens_size_bytes: u64, stale_tokens_count: u64) -> Self {
        Self {
            total_size_bytes,
            stale_tokens_size_bytes,
            stale_tokens_count,
        }
    }

    pub fn optimization_ratio(&self) -> f64 {
        if self.total_size_bytes == 0 {
            return 0.0;
        }
        self.stale_tokens_size_bytes as f64 / self.total_size_bytes as f64
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FtsOptimizerPlan {
    pub should_optimize: bool,
    pub estimated_freed_bytes: u64,
    pub prune_stale_tokens: bool,
}

#[derive(Debug, Clone)]
pub struct FtsOptimizer {
    pub ratio_threshold: f64,
    pub min_stale_bytes: u64,
}

impl FtsOptimizer {
    pub fn new(ratio_threshold: f64, min_stale_bytes: u64) -> Self {
        Self {
            ratio_threshold,
            min_stale_bytes,
        }
    }

    pub fn plan_optimization(&self, footprint: &MailboxIndexFootprint) -> FtsOptimizerPlan {
        let ratio = footprint.optimization_ratio();
        let should_optimize = ratio >= self.ratio_threshold && footprint.stale_tokens_size_bytes >= self.min_stale_bytes;

        FtsOptimizerPlan {
            should_optimize,
            estimated_freed_bytes: if should_optimize { footprint.stale_tokens_size_bytes } else { 0 },
            prune_stale_tokens: should_optimize,
        }
    }
}
