#![forbid(unsafe_code)]

use containers::oom_priority::{
    ContainerOomConfig, ContainerTier, OomRank, OomRanker,
};

#[test]
fn test_default_scores() {
    assert_eq!(ContainerTier::CriticalDatabase.default_score(), -950);
    assert_eq!(ContainerTier::CoreService.default_score(), -250);
    assert_eq!(ContainerTier::GeneralApplication.default_score(), 0);
    assert_eq!(ContainerTier::BackgroundBatchWorker.default_score(), 750);
}

#[test]
fn test_clamp_scores() {
    // Critical databases bounds: -1000 to -900
    assert_eq!(ContainerTier::CriticalDatabase.clamp_score(-1500), -1000);
    assert_eq!(ContainerTier::CriticalDatabase.clamp_score(-950), -950);
    assert_eq!(ContainerTier::CriticalDatabase.clamp_score(-500), -900);
    assert_eq!(ContainerTier::CriticalDatabase.clamp_score(0), -900);

    // Background workers bounds: 500 to 1000
    assert_eq!(ContainerTier::BackgroundBatchWorker.clamp_score(0), 500);
    assert_eq!(ContainerTier::BackgroundBatchWorker.clamp_score(750), 750);
    assert_eq!(ContainerTier::BackgroundBatchWorker.clamp_score(1500), 1000);
    assert_eq!(ContainerTier::BackgroundBatchWorker.clamp_score(-500), 500);

    // General application bounds: -1000 to 1000
    assert_eq!(ContainerTier::GeneralApplication.clamp_score(1500), 1000);
    assert_eq!(ContainerTier::GeneralApplication.clamp_score(-1500), -1000);
    assert_eq!(ContainerTier::GeneralApplication.clamp_score(0), 0);
    assert_eq!(ContainerTier::GeneralApplication.clamp_score(500), 500);

    // Core service bounds: -1000 to 1000
    assert_eq!(ContainerTier::CoreService.clamp_score(1500), 1000);
    assert_eq!(ContainerTier::CoreService.clamp_score(-1500), -1000);
    assert_eq!(ContainerTier::CoreService.clamp_score(-250), -250);
}

#[test]
fn test_oom_ranker() {
    let configs = vec![
        ContainerOomConfig::new("app1", ContainerTier::GeneralApplication),
        ContainerOomConfig::new("db1", ContainerTier::CriticalDatabase),
        ContainerOomConfig::new("worker1", ContainerTier::BackgroundBatchWorker),
        ContainerOomConfig::new("db2", ContainerTier::CriticalDatabase)
            .with_custom_adjustment(-800), // Will be clamped to -900
        ContainerOomConfig::new("worker2", ContainerTier::BackgroundBatchWorker)
            .with_custom_adjustment(2000), // Will be clamped to 1000
        ContainerOomConfig::new("core1", ContainerTier::CoreService),
    ];

    let ranks = OomRanker::rank(&configs);

    let _expected_ranks = [
        OomRank {
            container_id: "worker2".to_string(),
            score_adj: 1000,
            tier: ContainerTier::BackgroundBatchWorker,
        },
        OomRank {
            container_id: "worker1".to_string(),
            score_adj: 750,
            tier: ContainerTier::BackgroundBatchWorker,
        },
        OomRank {
            container_id: "app1".to_string(),
            score_adj: 0,
            tier: ContainerTier::GeneralApplication,
        },
        OomRank {
            container_id: "core1".to_string(),
            score_adj: -250,
            tier: ContainerTier::CoreService,
        },
        OomRank {
            container_id: "db1".to_string(),
            score_adj: -950,
            tier: ContainerTier::CriticalDatabase,
        },
        OomRank {
            container_id: "db2".to_string(),
            score_adj: -900,
            tier: ContainerTier::CriticalDatabase,
        },
    ];

    // Note: The ordering is highest score_adj first.
    // Let's check the order manually to match the OomRank Ord logic.
    assert_eq!(ranks[0].container_id, "worker2"); // 1000
    assert_eq!(ranks[1].container_id, "worker1"); // 750
    assert_eq!(ranks[2].container_id, "app1");    // 0
    assert_eq!(ranks[3].container_id, "core1");   // -250
    assert_eq!(ranks[4].container_id, "db2");     // -900
    assert_eq!(ranks[5].container_id, "db1");     // -950
}

#[test]
fn test_oom_ranker_stable_sorting() {
    let configs = vec![
        ContainerOomConfig::new("workerB", ContainerTier::BackgroundBatchWorker)
            .with_custom_adjustment(800),
        ContainerOomConfig::new("workerA", ContainerTier::BackgroundBatchWorker)
            .with_custom_adjustment(800),
        ContainerOomConfig::new("dbZ", ContainerTier::CriticalDatabase)
            .with_custom_adjustment(-950),
        ContainerOomConfig::new("dbX", ContainerTier::CriticalDatabase)
            .with_custom_adjustment(-950),
    ];

    let ranks = OomRanker::rank(&configs);

    assert_eq!(ranks[0].container_id, "workerA");
    assert_eq!(ranks[0].score_adj, 800);
    assert_eq!(ranks[1].container_id, "workerB");
    assert_eq!(ranks[1].score_adj, 800);
    assert_eq!(ranks[2].container_id, "dbX");
    assert_eq!(ranks[2].score_adj, -950);
    assert_eq!(ranks[3].container_id, "dbZ");
    assert_eq!(ranks[3].score_adj, -950);
}
