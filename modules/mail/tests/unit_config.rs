#![forbid(unsafe_code)]

use mail::config::dovecot::{
    render_dovecot_passdb_dict, render_dovecot_sql_conf, DovecotSqlConfig,
};
use mail::config::postfix::{
    render_virtual_mailbox_domains, render_virtual_mailbox_maps, PostfixError,
};

#[test]
fn test_postfix_virtual_mailbox_domains_safe() {
    let domains = vec!["example.com", "test.net", "mail.example.org"];
    let result = render_virtual_mailbox_domains(&domains).unwrap();
    assert_eq!(result, "example.com OK\ntest.net OK\nmail.example.org OK\n");
}

#[test]
fn test_postfix_virtual_mailbox_domains_malicious() {
    let domains = vec!["example.com", "test.net; rm -rf /", "mail%s.com"];
    let err = render_virtual_mailbox_domains(&domains).unwrap_err();
    assert_eq!(
        err,
        PostfixError::InvalidDomain("test.net; rm -rf /".to_string())
    );

    let domains2 = vec!["example.com", "mail%s.com"];
    let err2 = render_virtual_mailbox_domains(&domains2).unwrap_err();
    assert_eq!(err2, PostfixError::InvalidDomain("mail%s.com".to_string()));
}

#[test]
fn test_postfix_virtual_mailbox_maps_safe() {
    let mailboxes = vec![("user1", "example.com"), ("admin", "test.net")];
    let result = render_virtual_mailbox_maps(&mailboxes).unwrap();
    assert_eq!(
        result,
        "user1@example.com example.com/user1/\nadmin@test.net test.net/admin/\n"
    );
}

#[test]
fn test_postfix_virtual_mailbox_maps_malicious() {
    let mailboxes = vec![("us%er", "example.com")];
    let err = render_virtual_mailbox_maps(&mailboxes).unwrap_err();
    assert_eq!(err, PostfixError::InvalidUser("us%er".to_string()));

    let mailboxes2 = vec![("user", "example.com; ls")];
    let err2 = render_virtual_mailbox_maps(&mailboxes2).unwrap_err();
    assert_eq!(
        err2,
        PostfixError::InvalidDomain("example.com; ls".to_string())
    );
}

#[test]
fn test_dovecot_sql_conf() {
    let config = DovecotSqlConfig::default();
    let result = render_dovecot_sql_conf(&config);
    assert!(result.contains("driver = mysql"));
    assert!(result.contains("password_query = SELECT username, domain, password FROM users WHERE username = '%n' AND domain = '%d'"));
}

#[test]
fn test_dovecot_passdb_dict() {
    let result = render_dovecot_passdb_dict();
    assert!(result.contains("driver = sql"));
    assert!(result.contains("args = /etc/dovecot/dovecot-sql.conf.ext"));
}
