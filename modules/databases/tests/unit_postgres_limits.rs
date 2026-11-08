#![forbid(unsafe_code)]

use databases::postgres_limits::{PostgresLimitManager, LimitError};

#[test]
fn test_format_connection_limit_valid() {
    assert_eq!(
        PostgresLimitManager::format_connection_limit("user1", 10),
        Ok("ALTER ROLE \"user1\" CONNECTION LIMIT 10;".to_string())
    );
    assert_eq!(
        PostgresLimitManager::format_connection_limit("admin_user", -1),
        Ok("ALTER ROLE \"admin_user\" CONNECTION LIMIT -1;".to_string())
    );
    assert_eq!(
        PostgresLimitManager::format_connection_limit("guest", 0),
        Ok("ALTER ROLE \"guest\" CONNECTION LIMIT 0;".to_string())
    );
}

#[test]
fn test_format_connection_limit_invalid() {
    // Invalid usernames
    assert_eq!(
        PostgresLimitManager::format_connection_limit("user'", 10),
        Err(LimitError::InvalidUsername)
    );
    assert_eq!(
        PostgresLimitManager::format_connection_limit("user;", 10),
        Err(LimitError::InvalidUsername)
    );
    assert_eq!(
        PostgresLimitManager::format_connection_limit("user space", 10),
        Err(LimitError::InvalidUsername)
    );
    assert_eq!(
        PostgresLimitManager::format_connection_limit("", 10),
        Err(LimitError::InvalidUsername)
    );
    assert_eq!(
        PostgresLimitManager::format_connection_limit(&"a".repeat(64), 10),
        Err(LimitError::InvalidUsername)
    );

    // Invalid limits
    assert_eq!(
        PostgresLimitManager::format_connection_limit("user1", -2),
        Err(LimitError::InvalidLimit)
    );
    assert_eq!(
        PostgresLimitManager::format_connection_limit("user1", -100),
        Err(LimitError::InvalidLimit)
    );
}

#[test]
fn test_format_statement_timeout_valid() {
    assert_eq!(
        PostgresLimitManager::format_statement_timeout("user1", 5000),
        Ok("ALTER ROLE \"user1\" SET statement_timeout = '5000ms';".to_string())
    );
    assert_eq!(
        PostgresLimitManager::format_statement_timeout("db_admin", 0),
        Ok("ALTER ROLE \"db_admin\" SET statement_timeout = '0ms';".to_string())
    );
}

#[test]
fn test_format_statement_timeout_invalid() {
    assert_eq!(
        PostgresLimitManager::format_statement_timeout("user\"", 5000),
        Err(LimitError::InvalidUsername)
    );
    assert_eq!(
        PostgresLimitManager::format_statement_timeout("user--", 5000),
        Err(LimitError::InvalidUsername)
    );
}

#[test]
fn test_format_reset_statement_timeout_valid() {
    assert_eq!(
        PostgresLimitManager::format_reset_statement_timeout("user1"),
        Ok("ALTER ROLE \"user1\" RESET statement_timeout;".to_string())
    );
    assert_eq!(
        PostgresLimitManager::format_reset_statement_timeout("db_admin"),
        Ok("ALTER ROLE \"db_admin\" RESET statement_timeout;".to_string())
    );
}

#[test]
fn test_format_reset_statement_timeout_invalid() {
    assert_eq!(
        PostgresLimitManager::format_reset_statement_timeout("user\""),
        Err(LimitError::InvalidUsername)
    );
    assert_eq!(
        PostgresLimitManager::format_reset_statement_timeout("user--"),
        Err(LimitError::InvalidUsername)
    );
}
