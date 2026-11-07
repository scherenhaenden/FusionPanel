#![forbid(unsafe_code)]

use security::jail::{JailConfig, JailError, JailManager};
use std::time::{Duration, SystemTime};

#[test]
fn test_record_failure_triggers_ban() {
    let config = JailConfig {
        maxretry: 3,
        findtime: Duration::from_secs(60),
        bantime: Duration::from_secs(3600),
    };
    let mut manager = JailManager::new(config);
    let now = SystemTime::now();

    let ip = "192.168.1.100";

    // 1st failure - not banned
    assert_eq!(manager.record_failure(ip, now).unwrap(), false);
    assert_eq!(manager.is_banned(ip, now), false);

    // 2nd failure - not banned
    assert_eq!(manager.record_failure(ip, now).unwrap(), false);
    assert_eq!(manager.is_banned(ip, now), false);

    // 3rd failure - triggers ban
    assert_eq!(manager.record_failure(ip, now).unwrap(), true);
    assert_eq!(manager.is_banned(ip, now), true);

    // Subsequent failure - already banned
    assert_eq!(manager.record_failure(ip, now).unwrap(), true);
}

#[test]
fn test_findtime_expiration() {
    let config = JailConfig {
        maxretry: 3,
        findtime: Duration::from_secs(60),
        bantime: Duration::from_secs(3600),
    };
    let mut manager = JailManager::new(config);

    let now = SystemTime::now();
    let ip = "192.168.1.101";

    // 1st failure
    assert_eq!(manager.record_failure(ip, now).unwrap(), false);

    // Move time forward past findtime
    let later = now + Duration::from_secs(61);

    // 2nd failure at `later` - should act like 1st failure since previous expired
    assert_eq!(manager.record_failure(ip, later).unwrap(), false);

    // 3rd failure at `later`
    assert_eq!(manager.record_failure(ip, later).unwrap(), false);

    // Not banned because the first failure expired
    assert_eq!(manager.is_banned(ip, later), false);

    // 4th failure (3rd within new window)
    assert_eq!(manager.record_failure(ip, later).unwrap(), true);
    assert_eq!(manager.is_banned(ip, later), true);
}

#[test]
fn test_bantime_expiration() {
    let config = JailConfig {
        maxretry: 1,
        findtime: Duration::from_secs(60),
        bantime: Duration::from_secs(3600),
    };
    let mut manager = JailManager::new(config);

    let now = SystemTime::now();
    let ip = "192.168.1.102";

    // 1st failure - banned immediately due to maxretry=1
    assert_eq!(manager.record_failure(ip, now).unwrap(), true);
    assert_eq!(manager.is_banned(ip, now), true);

    // Check after partial bantime
    let later = now + Duration::from_secs(1800);
    assert_eq!(manager.is_banned(ip, later), true);

    // Check after full bantime - should be unbanned
    let expired = now + Duration::from_secs(3601);
    assert_eq!(manager.is_banned(ip, expired), false);
}

#[test]
fn test_manual_ban_unban() {
    let config = JailConfig::default();
    let mut manager = JailManager::new(config);
    let now = SystemTime::now();
    let ip = "192.168.1.103";

    assert_eq!(manager.is_banned(ip, now), false);

    manager.ban(ip, now);
    assert_eq!(manager.is_banned(ip, now), true);

    assert!(manager.unban(ip).is_ok());
    assert_eq!(manager.is_banned(ip, now), false);

    // Unban already unbanned
    assert!(matches!(manager.unban(ip), Err(JailError::IpNotBanned)));
}

#[test]
fn test_active_bans() {
    let config = JailConfig::default();
    let mut manager = JailManager::new(config);
    let now = SystemTime::now();
    let ip1 = "192.168.1.104";
    let ip2 = "192.168.1.105";

    manager.ban(ip1, now);
    manager.ban(ip2, now);

    let bans = manager.active_bans(now);
    assert_eq!(bans.len(), 2);
    assert!(bans.contains(&ip1.to_string()));
    assert!(bans.contains(&ip2.to_string()));

    // Advance time past ip1 ban time (using different times for ban)
    let config2 = JailConfig {
        bantime: Duration::from_secs(60),
        ..Default::default()
    };
    let mut manager2 = JailManager::new(config2);

    manager2.ban(ip1, now);
    manager2.ban(ip2, now + Duration::from_secs(3600));

    let future = now + Duration::from_secs(61);
    let active = manager2.active_bans(future);

    assert_eq!(active.len(), 1);
    assert!(active.contains(&ip2.to_string()));
}
