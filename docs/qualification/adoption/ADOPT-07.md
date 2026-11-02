# ADOPT-07: Upstream TLS Trust and SNI Verification

## Overview
This document outlines the FusionPanel policy and implementation for handling upstream TLS connections (e.g., from the Nginx reverse proxy to backend applications). It enforces strict security boundaries by ensuring proper Server Name Indication (SNI) and TLS certificate verification.

## Trust Modes
We support three explicit modes for verifying upstream TLS certificates:

1. **System CA (Default)**
   - The default mode. Nginx uses the system's root CA certificates (`/etc/ssl/certs/ca-certificates.crt`) to verify the upstream backend's certificate.
   - Requires `proxy_ssl_verify on` and validates the certificate against the expected SNI hostname.

2. **Internal CA**
   - For environments using private PKI or internal CAs (e.g., self-signed certificates distributed via configuration management).
   - Requires providing the path to the internal CA certificate.
   - Enables `proxy_ssl_verify on` pointing to the internal trust store.

3. **Insecure Compatibility**
   - Explicitly disables upstream TLS verification (`proxy_ssl_verify off`).
   - **Constraint**: This mode is never the default. It must be explicitly chosen.
   - **Requirement**: Selection of this mode requires providing an *Audit Trail* (why it is needed, such as a ticketing system ID) and a *Migration Path* (how and when the environment will be migrated to verified trust).
   - These justifications are injected as comments in the Nginx configuration to maintain accountability.

## SNI Preservation
- SNI (`proxy_ssl_server_name on`) is enabled by default to ensure modern TLS backends route correctly based on hostname.
- The `proxy_ssl_name` is explicitly set to the expected backend hostname to prevent mismatch errors during verification.
- Can be disabled if the upstream application strictly rejects SNI or expects a generic fallback (legacy systems).

## Nginx Configuration Generation

The Rust structure `UpstreamTlsPolicy` accurately models the configurations and translates them to the following standard Nginx directives:

```nginx
# When SNI is preserved (Default)
proxy_ssl_server_name on;
proxy_ssl_name <expected_hostname>;

# When Trust Mode is SystemCa (Default)
proxy_ssl_verify on;
proxy_ssl_trusted_certificate /etc/ssl/certs/ca-certificates.crt;
proxy_ssl_verify_depth 2;

# When Trust Mode is InternalCa
proxy_ssl_verify on;
proxy_ssl_trusted_certificate <path_to_ca_cert>;
proxy_ssl_verify_depth 2;

# When Trust Mode is InsecureCompatibility
# WARNING: Insecure compatibility mode enabled
# Audit Trail: <reason_for_insecure_mode>
# Migration Path: <plan_to_restore_verified_trust>
proxy_ssl_verify off;
```

## Evidence & Qualification
The tests located in `tests/qualification/adoption/adopt07/` verify that:
- The default trust mode remains `SystemCa`.
- All modes generate the correct configuration directives.
- Insecure mode explicitly models the requirement for an audit trail and migration path, translating those into the proxy configuration comments.
- Rust implementation correctly leverages `#[forbid(unsafe_code)]` and implements safe string formatting for generating proxy directives.
