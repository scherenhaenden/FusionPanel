# ADOPT-01: Reconciling Effective Nginx and Application Behavior Before Migration

## Overview

During the migration and adoption process, FusionPanel must ensure that existing web server configurations, application configurations, and live upstream behaviors are consistent and accurately modeled before taking ownership of the system. This reconciliation process is critical to preventing regressions and ensuring a seamless transition.

The ADOPT-01 qualification suite defines the requirements for this reconciliation phase.

## Reconciliation Process

The reconciliation process consists of evaluating the following data sources:

1.  **Panel Metadata**: The intended configuration as specified within FusionPanel.
2.  **Effective Web Server Configuration**: The parsed state of the existing Nginx configuration (e.g., active locations, rewrites, headers, aliases).
3.  **Application Configuration**: Configuration derived from the application itself (e.g., framework settings).
4.  **Live Upstream Observations**: Real-time behavior observed from the active application (e.g., upstream scheme, port).

## Conflict Detection

The reconciliation engine must detect and report conflicts across these sources in the following areas:

*   **Site Type**: E.g., static vs. proxy vs. PHP-FPM.
*   **Root Directory**: The primary document root for the application.
*   **Upstream Scheme & Port**: The protocol (HTTP/HTTPS) and port used for upstream communication.
*   **Aliases**: Alternate domain names or paths.
*   **Route Maps**:
    *   Ordered locations
    *   Custom headers
    *   Rewrite rules
    *   WebSocket and streaming paths

## Failure Mode (Fail Closed)

In the event that the reconciliation process detects unresolved conflicts between the data sources, the system **must fail closed**.

This means:
1.  The migration apply operation is refused.
2.  The existing service behavior is left unmodified.
3.  A detailed report of the conflicts is generated for manual review and resolution by an administrator.
