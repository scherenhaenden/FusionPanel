# Test Fixtures

This directory contains isolated Compose fixtures with generated, private test credentials.

## MariaDB Fixture Usage

To use the MariaDB fixture, follow these steps:

1. **Start the MariaDB service:**
   ```bash
   python3 tests/fixtures/compose.py up mariadb
   ```

2. **Wait for the service to be healthy:**
   ```bash
   python3 tests/fixtures/compose.py wait mariadb
   ```

3. **Get the connection environment configuration:**
   ```bash
   python3 tests/fixtures/compose.py env
   ```
   (Outputs JSON with host, port, root_user, root_password_env_name, app_user, database)

4. **Run a SQL client statement:**
   ```bash
   python3 tests/fixtures/compose.py client "SELECT 1"
   ```

5. **Stop and verify clean:**
   ```bash
   python3 tests/fixtures/compose.py down
   python3 tests/fixtures/compose.py verify-clean
   ```

If Docker or an image is missing, the script will output a `BLOCKED: <reason>` message and exit non-zero.
