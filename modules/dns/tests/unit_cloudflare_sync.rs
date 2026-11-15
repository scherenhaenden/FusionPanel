#![forbid(unsafe_code)]

use dns::cloudflare_sync::{
    CloudflareSyncPlanner, LocalRecord, RecordType, RemoteRecord, SyncAction,
};

#[test]
fn test_cloudflare_sync_planner_empty() {
    let local = vec![];
    let remote = vec![];
    let actions = CloudflareSyncPlanner::plan(&local, &remote);
    assert!(actions.is_empty());
}

#[test]
fn test_cloudflare_sync_planner_create() {
    let local = vec![LocalRecord {
        name: "example.com".to_string(),
        record_type: RecordType::A,
        content: "1.2.3.4".to_string(),
        ttl: 300,
        proxied: false,
        priority: None,
    }];
    let remote = vec![];

    let actions = CloudflareSyncPlanner::plan(&local, &remote);
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0],
        SyncAction::Create(local[0].clone())
    );
}

#[test]
fn test_cloudflare_sync_planner_delete() {
    let local = vec![];
    let remote = vec![RemoteRecord {
        id: "id1".to_string(),
        name: "example.com".to_string(),
        record_type: RecordType::A,
        content: "1.2.3.4".to_string(),
        ttl: 300,
        proxied: false,
        priority: None,
    }];

    let actions = CloudflareSyncPlanner::plan(&local, &remote);
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0],
        SyncAction::Delete { id: "id1".to_string() }
    );
}

#[test]
fn test_cloudflare_sync_planner_no_action() {
    let local = vec![LocalRecord {
        name: "example.com".to_string(),
        record_type: RecordType::A,
        content: "1.2.3.4".to_string(),
        ttl: 300,
        proxied: false,
        priority: None,
    }];
    let remote = vec![RemoteRecord {
        id: "id1".to_string(),
        name: "example.com".to_string(),
        record_type: RecordType::A,
        content: "1.2.3.4".to_string(),
        ttl: 300,
        proxied: false,
        priority: None,
    }];

    let actions = CloudflareSyncPlanner::plan(&local, &remote);
    assert!(actions.is_empty());
}

#[test]
fn test_cloudflare_sync_planner_update_ttl() {
    let local = vec![LocalRecord {
        name: "example.com".to_string(),
        record_type: RecordType::A,
        content: "1.2.3.4".to_string(),
        ttl: 600,
        proxied: false,
        priority: None,
    }];
    let remote = vec![RemoteRecord {
        id: "id1".to_string(),
        name: "example.com".to_string(),
        record_type: RecordType::A,
        content: "1.2.3.4".to_string(),
        ttl: 300,
        proxied: false,
        priority: None,
    }];

    let actions = CloudflareSyncPlanner::plan(&local, &remote);
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0],
        SyncAction::Update {
            id: "id1".to_string(),
            new_record: local[0].clone(),
        }
    );
}

#[test]
fn test_cloudflare_sync_planner_update_content() {
    let local = vec![LocalRecord {
        name: "example.com".to_string(),
        record_type: RecordType::A,
        content: "2.3.4.5".to_string(),
        ttl: 300,
        proxied: false,
        priority: None,
    }];
    let remote = vec![RemoteRecord {
        id: "id1".to_string(),
        name: "example.com".to_string(),
        record_type: RecordType::A,
        content: "1.2.3.4".to_string(),
        ttl: 300,
        proxied: false,
        priority: None,
    }];

    let actions = CloudflareSyncPlanner::plan(&local, &remote);
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0],
        SyncAction::Update {
            id: "id1".to_string(),
            new_record: local[0].clone(),
        }
    );
}

#[test]
fn test_cloudflare_sync_planner_complex() {
    let local = vec![
        LocalRecord {
            name: "example.com".to_string(),
            record_type: RecordType::A,
            content: "1.1.1.1".to_string(),
            ttl: 300,
            proxied: false,
            priority: None,
        },
        LocalRecord {
            name: "example.com".to_string(),
            record_type: RecordType::A,
            content: "2.2.2.2".to_string(),
            ttl: 600, // ttl updated
            proxied: false,
            priority: None,
        },
        LocalRecord {
            name: "example.com".to_string(),
            record_type: RecordType::A,
            content: "3.3.3.3".to_string(), // new content
            ttl: 300,
            proxied: false,
            priority: None,
        },
    ];
    let remote = vec![
        RemoteRecord {
            id: "id1".to_string(),
            name: "example.com".to_string(),
            record_type: RecordType::A,
            content: "1.1.1.1".to_string(),
            ttl: 300,
            proxied: false,
            priority: None,
        },
        RemoteRecord {
            id: "id2".to_string(),
            name: "example.com".to_string(),
            record_type: RecordType::A,
            content: "2.2.2.2".to_string(),
            ttl: 300,
            proxied: false,
            priority: None,
        },
        RemoteRecord {
            id: "id3".to_string(),
            name: "example.com".to_string(),
            record_type: RecordType::A,
            content: "4.4.4.4".to_string(), // content to be deleted/replaced
            ttl: 300,
            proxied: false,
            priority: None,
        },
    ];

    let mut actions = CloudflareSyncPlanner::plan(&local, &remote);
    actions.sort();

    // We expect:
    // - id1: no action (match exact)
    // - id2: Update (match content 2.2.2.2, diff TTL)
    // - id3: Update (from 4.4.4.4 to 3.3.3.3) -> Wait, since contents differ, the algorithm
    //        matches exact contents first.
    // Let's trace exact contents:
    // Local "1.1.1.1" matches Remote "1.1.1.1" -> equal, no action.
    // Local "2.2.2.2" matches Remote "2.2.2.2" -> diff TTL -> Update id2.
    // Remaining local: "3.3.3.3".
    // Remaining remote: "4.4.4.4" (id3).
    // Algorithm matches remaining local to remaining remote up to min_len -> Update id3 to 3.3.3.3.

    assert_eq!(actions.len(), 2);

    assert!(actions.contains(&SyncAction::Update {
        id: "id2".to_string(),
        new_record: LocalRecord {
            name: "example.com".to_string(),
            record_type: RecordType::A,
            content: "2.2.2.2".to_string(),
            ttl: 600,
            proxied: false,
            priority: None,
        }
    }));

    assert!(actions.contains(&SyncAction::Update {
        id: "id3".to_string(),
        new_record: LocalRecord {
            name: "example.com".to_string(),
            record_type: RecordType::A,
            content: "3.3.3.3".to_string(),
            ttl: 300,
            proxied: false,
            priority: None,
        }
    }));
}
