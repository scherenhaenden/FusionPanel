#![forbid(unsafe_code)]

use security::jail::{JailConfig, JailError, JailManager};
use std::time::{Duration, SystemTime};

#[test]
fn test_record_failure_triggers_ban() {
    let config = JailConfig {
        maxretry: 3,
        findtime: Duration::from_secs(60),
        bantime: Duration::from_secs(3600),
        subnet_ban_threshold: 5,
    };
    let mut manager = JailManager::new(config);
    let now = SystemTime::now();

    let ip = "192.168.1.100";

    // 1st failure - not banned
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), now)
            .unwrap(),
        false
    );
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), now), false);

    // 2nd failure - not banned
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), now)
            .unwrap(),
        false
    );
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), now), false);

    // 3rd failure - triggers ban
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), now)
            .unwrap(),
        true
    );
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), now), true);

    // Subsequent failure - already banned
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), now)
            .unwrap(),
        true
    );
}

#[test]
fn test_findtime_expiration() {
    let config = JailConfig {
        maxretry: 3,
        findtime: Duration::from_secs(60),
        bantime: Duration::from_secs(3600),
        subnet_ban_threshold: 5,
    };
    let mut manager = JailManager::new(config);

    let now = SystemTime::now();
    let ip = "192.168.1.101";

    // 1st failure
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), now)
            .unwrap(),
        false
    );

    // Move time forward past findtime
    let later = now + Duration::from_secs(61);

    // 2nd failure at `later` - should act like 1st failure since previous expired
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), later)
            .unwrap(),
        false
    );

    // 3rd failure at `later`
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), later)
            .unwrap(),
        false
    );

    // Not banned because the first failure expired
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), later), false);

    // 4th failure (3rd within new window)
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), later)
            .unwrap(),
        true
    );
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), later), true);
}

#[test]
fn test_bantime_expiration() {
    let config = JailConfig {
        maxretry: 1,
        findtime: Duration::from_secs(60),
        bantime: Duration::from_secs(3600),
        subnet_ban_threshold: 5,
    };
    let mut manager = JailManager::new(config);

    let now = SystemTime::now();
    let ip = "192.168.1.102";

    // 1st failure - banned immediately due to maxretry=1
    assert_eq!(
        manager
            .record_failure("sshd", ip.parse().unwrap(), now)
            .unwrap(),
        true
    );
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), now), true);

    // Check after partial bantime
    let later = now + Duration::from_secs(1800);
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), later), true);

    // Check after full bantime - should be unbanned
    let expired = now + Duration::from_secs(3601);
    assert_eq!(
        manager.is_banned("sshd", ip.parse().unwrap(), expired),
        false
    );
}

#[test]
fn test_manual_ban_unban() {
    let config = JailConfig::default();
    let mut manager = JailManager::new(config);
    let now = SystemTime::now();
    let ip = "192.168.1.103";

    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), now), false);

    manager.ban("sshd", ip.parse().unwrap(), now);
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), now), true);

    assert!(manager.unban("sshd", ip.parse().unwrap()).is_ok());
    assert_eq!(manager.is_banned("sshd", ip.parse().unwrap(), now), false);

    // Unban already unbanned
    assert!(matches!(
        manager.unban("sshd", ip.parse().unwrap()),
        Err(JailError::IpNotBanned)
    ));
}

#[test]
fn test_active_bans() {
    let config = JailConfig::default();
    let mut manager = JailManager::new(config);
    let now = SystemTime::now();
    let ip1 = "192.168.1.104";
    let ip2 = "192.168.1.105";

    manager.ban("sshd", ip1.parse().unwrap(), now);
    manager.ban("sshd", ip2.parse().unwrap(), now);

    let bans = manager.active_bans(now);
    assert_eq!(bans.len(), 2);
    assert!(bans.contains(&format!("sshd:{}", ip1)));
    assert!(bans.contains(&format!("sshd:{}", ip2)));

    // Advance time past ip1 ban time (using different times for ban)
    let config2 = JailConfig {
        bantime: Duration::from_secs(60),
        ..Default::default()
    };
    let mut manager2 = JailManager::new(config2);

    manager2.ban("sshd", ip1.parse().unwrap(), now);
    manager2.ban(
        "sshd",
        ip2.parse().unwrap(),
        now + Duration::from_secs(3600),
    );

    let future = now + Duration::from_secs(61);
    let active = manager2.active_bans(future);

    assert_eq!(active.len(), 1);
    assert!(active.contains(&format!("sshd:{}", ip2)));
}

use security::jail::{JailAction, JailLogEvent};
use std::net::IpAddr;

#[test]
fn test_parse_log_line() {
    let line = "2023-10-27 10:00:00,000 fail2ban.actions [123]: NOTICE [sshd] Ban 192.168.1.100";
    let event = JailLogEvent::parse(line).unwrap();
    assert_eq!(event.jail, "sshd");
    assert_eq!(event.action, JailAction::Ban);
    assert_eq!(event.ip, "192.168.1.100".parse::<IpAddr>().unwrap());

    let line2 = "2023-10-27 10:00:00,000 fail2ban.filter [123]: INFO [nginx-http-auth] Found 2001:db8::1 - 2023-10-27 10:00:00";
    let event2 = JailLogEvent::parse(line2).unwrap();
    assert_eq!(event2.jail, "nginx-http-auth");
    assert_eq!(event2.action, JailAction::Found);
    assert_eq!(event2.ip, "2001:db8::1".parse::<IpAddr>().unwrap());
}

#[test]
fn test_parse_long_line() {
    let long_line = format!("{} [sshd] Ban 192.168.1.100", "a".repeat(1024));
    let err = JailLogEvent::parse(&long_line).unwrap_err();
    assert!(matches!(err, security::jail::JailError::LineTooLong));
}

#[test]
fn test_subnet_aggregation() {
    let config = JailConfig {
        maxretry: 3,
        findtime: Duration::from_secs(60),
        bantime: Duration::from_secs(3600),
        subnet_ban_threshold: 3,
    };
    let mut manager = JailManager::new(config);
    let now = SystemTime::now();

    manager.ban("sshd", "192.168.1.10".parse().unwrap(), now);
    manager.ban("sshd", "192.168.1.20".parse().unwrap(), now);
    manager.ban("sshd", "192.168.1.30".parse().unwrap(), now);

    // Any other IP in this /24 should now be banned
    assert_eq!(
        manager.is_banned("sshd", "192.168.1.40".parse().unwrap(), now),
        true
    );
    // An IP in a different subnet should not be banned
    assert_eq!(
        manager.is_banned("sshd", "192.168.2.10".parse().unwrap(), now),
        false
    );
    // An IP in a different jail should not be banned
    assert_eq!(
        manager.is_banned("nginx", "192.168.1.40".parse().unwrap(), now),
        false
    );

    // IPv6 subnet test
    manager.ban("sshd", "2001:db8:1234::1".parse().unwrap(), now);
    manager.ban("sshd", "2001:db8:1234::2".parse().unwrap(), now);
    manager.ban("sshd", "2001:db8:1234::3".parse().unwrap(), now);

    assert_eq!(
        manager.is_banned("sshd", "2001:db8:1234::4".parse().unwrap(), now),
        true
    );
    assert_eq!(
        manager.is_banned("sshd", "2001:db8:5678::1".parse().unwrap(), now),
        false
    );
}

#[test]
fn test_duplicate_ips_and_jails() {
    let config = JailConfig::default();
    let mut manager = JailManager::new(config);
    let now = SystemTime::now();
    let ip: IpAddr = "10.0.0.1".parse().unwrap();

    manager.ban("sshd", ip, now);
    manager.ban("nginx", ip, now);

    let active_bans = manager.active_bans(now);
    assert!(active_bans.contains(&"sshd:10.0.0.1".to_string()));
    assert!(active_bans.contains(&"nginx:10.0.0.1".to_string()));

    manager.unban("sshd", ip).unwrap();
    assert_eq!(manager.is_banned("sshd", ip, now), false);
    assert_eq!(manager.is_banned("nginx", ip, now), true); // nginx ban should still be active
}
