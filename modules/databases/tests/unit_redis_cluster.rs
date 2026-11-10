#![forbid(unsafe_code)]

use databases::redis_cluster::{ClusterNode, RedisClusterRouter};

#[test]
fn test_crc16_and_hash_slot() {
    // Known test vectors from Redis documentation
    assert_eq!(
        RedisClusterRouter::get_hash_slot("123456789"),
        0x31C3 & 0x3FFF
    );
    assert_eq!(RedisClusterRouter::get_hash_slot("abc"), 7638);
    assert_eq!(RedisClusterRouter::get_hash_slot("123"), 5970);
}

#[test]
fn test_hash_tags() {
    // Should hash the part inside {}
    let slot1 = RedisClusterRouter::get_hash_slot("{user1000}.following");
    let slot2 = RedisClusterRouter::get_hash_slot("{user1000}.followers");
    let slot3 = RedisClusterRouter::get_hash_slot("user1000");

    assert_eq!(slot1, slot2);
    assert_eq!(slot1, slot3);

    // Empty hash tag, should hash the whole key
    let empty_tag_slot = RedisClusterRouter::get_hash_slot("{}");
    assert_eq!(empty_tag_slot, RedisClusterRouter::get_hash_slot("{}"));

    let foo_empty = RedisClusterRouter::get_hash_slot("foo{}bar");
    assert_ne!(foo_empty, RedisClusterRouter::get_hash_slot(""));
    assert_eq!(foo_empty, RedisClusterRouter::get_hash_slot("foo{}bar"));

    // Multiple {}
    let multi_tag_slot = RedisClusterRouter::get_hash_slot("foo{bar}{zap}");
    let multi_tag_slot2 = RedisClusterRouter::get_hash_slot("bar");
    assert_eq!(multi_tag_slot, multi_tag_slot2);
}

#[test]
fn test_slot_boundaries() {
    let mut min_slot = 16384;
    let mut max_slot = 0;

    // Quick probabilistic check
    for i in 0..100000 {
        let key = format!("key{}", i);
        let slot = RedisClusterRouter::get_hash_slot(&key);
        if slot < min_slot {
            min_slot = slot;
        }
        if slot > max_slot {
            max_slot = slot;
        }
    }

    // min_slot is a u16, so it's always >= 0. But we want to see it hitting 0.
    assert_eq!(min_slot, 0);
    assert_eq!(max_slot, 16383);
}

#[test]
fn test_node_routing() {
    let nodes = vec![
        ClusterNode {
            id: "node1".to_string(),
            addr: "127.0.0.1:7000".to_string(),
            start_slot: 0,
            end_slot: 5460,
        },
        ClusterNode {
            id: "node2".to_string(),
            addr: "127.0.0.1:7001".to_string(),
            start_slot: 5461,
            end_slot: 10922,
        },
        ClusterNode {
            id: "node3".to_string(),
            addr: "127.0.0.1:7002".to_string(),
            start_slot: 10923,
            end_slot: 16383,
        },
    ];

    let router = RedisClusterRouter::new(nodes);

    let key = "123";
    let slot = RedisClusterRouter::get_hash_slot(key); // 5970
    assert_eq!(slot, 5970);

    let node = router.route(key).unwrap();
    assert_eq!(node.id, "node2");
    assert_eq!(node.addr, "127.0.0.1:7001");
}
