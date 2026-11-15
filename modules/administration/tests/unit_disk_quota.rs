#![forbid(unsafe_code)]

use administration::disk_quota::{Dqblk, QuotaAlert, QuotaData, QuotaEnforcer, QuotaError};

#[test]
fn test_dqblk_conversion() {
    let dqblk = Dqblk {
        dqb_bhardlimit: 1000,
        dqb_bsoftlimit: 800,
        dqb_curspace: 500,
        dqb_ihardlimit: 100,
        dqb_isoftlimit: 80,
        dqb_curinodes: 50,
        dqb_btime: 0,
        dqb_itime: 0,
        dqb_valid: 0,
    };

    let data: QuotaData = (&dqblk).into();

    assert_eq!(data.user, "");
    assert_eq!(data.space_used, 500);
    assert_eq!(data.space_soft_limit, 800);
    assert_eq!(data.space_hard_limit, 1000);
    assert_eq!(data.inode_used, 50);
    assert_eq!(data.inode_soft_limit, 80);
    assert_eq!(data.inode_hard_limit, 100);
}

#[test]
fn test_parse_repquota_line_valid() {
    let line = "jdoe      --   50000   40000   60000              50     40     60";
    let data = QuotaEnforcer::parse_repquota_line(line).unwrap();

    assert_eq!(data.user, "jdoe");
    assert_eq!(data.space_used, 50000);
    assert_eq!(data.space_soft_limit, 40000);
    assert_eq!(data.space_hard_limit, 60000);
    assert_eq!(data.inode_used, 50);
    assert_eq!(data.inode_soft_limit, 40);
    assert_eq!(data.inode_hard_limit, 60);
}

#[test]
fn test_parse_repquota_line_valid_exceeded() {
    let line = "jdoe      +-   70000   40000   60000              50     40     60";
    let data = QuotaEnforcer::parse_repquota_line(line).unwrap();

    assert_eq!(data.user, "jdoe");
    assert_eq!(data.space_used, 70000);
}

#[test]
fn test_parse_repquota_line_invalid_format() {
    let line = "jdoe";
    assert_eq!(QuotaEnforcer::parse_repquota_line(line), Err(QuotaError::InvalidFormat));

    let line = "jdoe --- 50000 40000 60000 50 40 60";
    assert_eq!(QuotaEnforcer::parse_repquota_line(line), Err(QuotaError::InvalidFormat));

    let line = "jdoe -- 50000 40000 60000 50 40";
    assert_eq!(QuotaEnforcer::parse_repquota_line(line), Err(QuotaError::InvalidFormat));
}

#[test]
fn test_evaluate_quotas_no_alerts() {
    let enforcer = QuotaEnforcer::new(90.0);
    let data = QuotaData {
        user: "jdoe".to_string(),
        space_used: 100,
        space_soft_limit: 200,
        space_hard_limit: 300,
        inode_used: 10,
        inode_soft_limit: 20,
        inode_hard_limit: 30,
    };

    let alerts = enforcer.evaluate(&data);
    assert!(alerts.is_empty());
}

#[test]
fn test_evaluate_quotas_space_hard_exceeded() {
    let enforcer = QuotaEnforcer::new(90.0);
    let data = QuotaData {
        user: "jdoe".to_string(),
        space_used: 350,
        space_soft_limit: 200,
        space_hard_limit: 300,
        inode_used: 10,
        inode_soft_limit: 20,
        inode_hard_limit: 30,
    };

    let alerts = enforcer.evaluate(&data);
    assert_eq!(alerts, vec![QuotaAlert::SpaceHardLimitExceeded(350, 300)]);
}

#[test]
fn test_evaluate_quotas_space_soft_exceeded() {
    let enforcer = QuotaEnforcer::new(90.0);
    let data = QuotaData {
        user: "jdoe".to_string(),
        space_used: 250,
        space_soft_limit: 200,
        space_hard_limit: 300,
        inode_used: 10,
        inode_soft_limit: 20,
        inode_hard_limit: 30,
    };

    let alerts = enforcer.evaluate(&data);
    assert_eq!(alerts, vec![QuotaAlert::SpaceSoftLimitExceeded(250, 200)]);
}

#[test]
fn test_evaluate_quotas_inode_hard_exceeded() {
    let enforcer = QuotaEnforcer::new(90.0);
    let data = QuotaData {
        user: "jdoe".to_string(),
        space_used: 100,
        space_soft_limit: 200,
        space_hard_limit: 300,
        inode_used: 35,
        inode_soft_limit: 20,
        inode_hard_limit: 30,
    };

    let alerts = enforcer.evaluate(&data);
    assert_eq!(alerts, vec![QuotaAlert::InodeHardLimitExceeded(35, 30)]);
}

#[test]
fn test_evaluate_quotas_inode_soft_exceeded() {
    let enforcer = QuotaEnforcer::new(90.0);
    let data = QuotaData {
        user: "jdoe".to_string(),
        space_used: 100,
        space_soft_limit: 200,
        space_hard_limit: 300,
        inode_used: 25,
        inode_soft_limit: 20,
        inode_hard_limit: 30,
    };

    let alerts = enforcer.evaluate(&data);
    assert!(alerts.contains(&QuotaAlert::InodeSoftLimitExceeded(25, 20)));
}

#[test]
fn test_evaluate_quotas_inode_threshold_exceeded() {
    let enforcer = QuotaEnforcer::new(90.0);
    let data = QuotaData {
        user: "jdoe".to_string(),
        space_used: 100,
        space_soft_limit: 200,
        space_hard_limit: 300,
        inode_used: 95,
        inode_soft_limit: 100, // Not exceeding soft limit
        inode_hard_limit: 100,
    };

    let alerts = enforcer.evaluate(&data);
    assert_eq!(alerts, vec![QuotaAlert::InodeThresholdExceeded {
        used: 95,
        total: 100,
        threshold_pct: 90.0,
    }]);
}

#[test]
fn test_evaluate_quotas_multiple_alerts() {
    let enforcer = QuotaEnforcer::new(90.0);
    let data = QuotaData {
        user: "jdoe".to_string(),
        space_used: 250,
        space_soft_limit: 200,
        space_hard_limit: 300,
        inode_used: 25,
        inode_soft_limit: 20,
        inode_hard_limit: 30,
    };

    let alerts = enforcer.evaluate(&data);
    assert_eq!(alerts.len(), 2);
    assert!(alerts.contains(&QuotaAlert::SpaceSoftLimitExceeded(250, 200)));
    assert!(alerts.contains(&QuotaAlert::InodeSoftLimitExceeded(25, 20)));
}
