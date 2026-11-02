1. **Define Error Types**: Create specific error types for WP-CLI execution (e.g., Timeout, CommandFailed, ParsingError) in `modules/wordpress/src/wp_cli/error.rs` using `thiserror`.
2. **Implement WP-CLI Execution Wrapper**:
   - Create `modules/wordpress/src/wp_cli/executor.rs`.
   - Implement an executor struct/function that runs `wp` with standard flags (e.g., `--path=<site-path>`, `--allow-root` if needed, etc.).
   - Use `tokio::process::Command` to run the WP-CLI tool.
   - Enforce timeouts using `tokio::time::timeout`.
   - Ensure the command builder is strongly typed to avoid shell injection vulnerabilities (i.e. strictly passing arguments via `Command::arg`).
3. **Implement Specific Command Builders and Output Parsers**:
   - Create `modules/wordpress/src/wp_cli/commands.rs`.
   - Implement strongly typed functions or builders for key WP-CLI commands (like checking core version, site info, or similar standard operations if the issue hints at specific ones, or a general flexible builder). Since this is a general wrapper for WP-CLI, providing a generic structured command runner and a few typed examples (e.g., `wp core info --format=json`) is standard.
   - Implement JSON parsing for commands that return JSON.
4. **Implement Unit Tests**:
   - Create `modules/wordpress/tests/unit_wp_cli.rs`.
   - Write tests mocking or testing the typed command builder logic and parser logic. Since running actual WP-CLI might not be available or stable in the test environment, testing the argument generation and output parsing is key. (Could test basic execution if a dummy `wp` script is created for the tests).
5. **Pre-commit checks**: Run `cargo fmt`, `cargo clippy`, and `cargo test` on the module. Call `pre_commit_instructions`.
6. **Submit PR**: Create a branch `task-w03-26-wp-cli`, commit changes, create PR against `master`, report URL.
