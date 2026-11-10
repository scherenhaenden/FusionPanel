#![forbid(unsafe_code)]

use std::cmp::Ordering;

/// Represents the classification of a container for OOM priority ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerTier {
    /// Critical databases are heavily protected from OOM kills. (-900 to -1000)
    CriticalDatabase,
    /// Core services essential to the system.
    CoreService,
    /// Standard user applications.
    GeneralApplication,
    /// Background workers that can be easily restarted and are preferred for OOM eviction. (+500 to +1000)
    BackgroundBatchWorker,
}

impl ContainerTier {
    /// Provides a sensible default `oom_score_adj` based on the tier.
    pub fn default_score(&self) -> i32 {
        match self {
            ContainerTier::CriticalDatabase => -950,
            ContainerTier::CoreService => -250,
            ContainerTier::GeneralApplication => 0,
            ContainerTier::BackgroundBatchWorker => 750,
        }
    }

    /// Clamps the score to the valid range for the tier.
    pub fn clamp_score(&self, score: i32) -> i32 {
        match self {
            ContainerTier::CriticalDatabase => score.clamp(-1000, -900),
            ContainerTier::BackgroundBatchWorker => score.clamp(500, 1000),
            _ => score.clamp(-1000, 1000),
        }
    }
}

/// Configuration for a container's OOM priority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerOomConfig {
    pub container_id: String,
    pub tier: ContainerTier,
    pub custom_adjustment: Option<i32>,
}

impl ContainerOomConfig {
    pub fn new(container_id: impl Into<String>, tier: ContainerTier) -> Self {
        Self {
            container_id: container_id.into(),
            tier,
            custom_adjustment: None,
        }
    }

    pub fn with_custom_adjustment(mut self, adjustment: i32) -> Self {
        self.custom_adjustment = Some(adjustment);
        self
    }
}

/// Represents the final computed OOM rank of a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OomRank {
    pub container_id: String,
    pub score_adj: i32,
    pub tier: ContainerTier,
}

impl PartialOrd for OomRank {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OomRank {
    fn cmp(&self, other: &Self) -> Ordering {
        // We order OomRank such that higher score_adj (more likely to be killed) comes first.
        // If scores are equal, sort by container_id alphabetically for stable ordering.
        other.score_adj.cmp(&self.score_adj)
            .then_with(|| self.container_id.cmp(&other.container_id))
    }
}

/// A struct that ranks containers by their OOM priority.
pub struct OomRanker;

impl OomRanker {
    /// Ranks a list of container configurations and returns the computed ranks.
    /// The returned list is sorted by kill priority (highest score_adj first).
    pub fn rank(configs: &[ContainerOomConfig]) -> Vec<OomRank> {
        let mut ranks: Vec<OomRank> = configs
            .iter()
            .map(|config| {
                let initial_score = config
                    .custom_adjustment
                    .unwrap_or_else(|| config.tier.default_score());

                let clamped_score = config.tier.clamp_score(initial_score);

                OomRank {
                    container_id: config.container_id.clone(),
                    score_adj: clamped_score,
                    tier: config.tier,
                }
            })
            .collect();

        ranks.sort();
        ranks
    }
}
