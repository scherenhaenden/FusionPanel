# Containers Module

This module provides utilities for managing and monitoring containers.

## Features
- **Health Checks**: Implements health probe policies with interval, retries, and timeout settings.
- **Log Streaming**: Provides a bounded ring-buffer log reader with redaction capabilities for sensitive data.

## Usage
Add to your project dependencies and use the `health` and `logs` modules.

## Testing
Run tests using:
```sh
cargo test --manifest-path Cargo.toml
```
