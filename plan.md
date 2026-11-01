1. **Explore & Define Missing Validations**:
   - Check `domain/validation.rs` for `DocumentRoot`, `PoolName`, `PhpVersion`, `MemoryLimit`, `UpstreamUrl` wrappers.
   - Define wrappers enforcing constraints upon instantiation/deserialization.
2. **Update `SiteConfig` & `HostingSpec`**:
   - Change `String` types to these new validated wrapper types in `domain/site.rs`.
   - Update `SiteConfig::new` to handle or require these types.
3. **Fix Injection in Configuration Generation**:
   - In `config/nginx.rs` and `config/php_fpm.rs`, update the config formatting to use the new typed accessors instead of direct strings.
   - Fix the explicit validated tenant identity mapping in `config/php_fpm.rs` (replace `www-data` with `tenant_{}`).
4. **Update Integration Tests**:
   - In `tests/integration.rs`, properly panic with "BLOCKED" if Docker fails/is unavailable, as per constraints.
   - Add negative tests for invalid document roots, pool names, etc. (invariant bypass).
5. **Run tests**: Make sure all tests pass (or panic with BLOCKED properly for missing environment).
6. **Pre-commit checks**: Run `cargo fmt`, `cargo clippy`, `cargo test`.
7. **Submit**: Create PR against `master`.
