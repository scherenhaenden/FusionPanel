# Observability Module

This module implements contracts, Prometheus integration, probes, alert delivery and incident states for the FusionPanel observability domain.

## Shared Workspace Modifications

To integrate this module correctly, the following `.gitignore` modifications should be applied in the shared workspace root (do not commit them locally in this batch):

```
# Exclude observability module build artifacts
modules/observability/target/
```

## E2E Case Definitions

- Target: E2E outage notification
  - Trigger: Kill mock application instance (`killall nginx` on target host)
  - Verify: Metric `site_requests_total` drops; HTTP probe fails
  - Expect: New `Incident` triggered and `Alert` JSON payload POSTed to the webhook endpoint.
  - Recovery: Restart mock application, probe recovers, incident auto-resolves.

## Capabilities

The following capabilities are tracked:
- OBS-01: CPU, RAM and disk metrics (Implemented)
- OBS-02: Metrics history (Blocked: Time-series database dependency missing)
- OBS-03: Live network traffic (Implemented: Network RX/TX counters)
- OBS-04: Per-site metrics (Implemented: Site requests counter)
- OBS-05: Per-container metrics (Implemented: Container CPU gauge)
- OBS-06: HTTP uptime checks (Implemented)
- OBS-07: TCP and ping checks (Implemented TCP, Blocked ping: Missing root privileges/ICMP raw socket permissions)
- OBS-08: Prometheus endpoint (Implemented)
- OBS-09: Configurable alerts (Implemented)
- OBS-10: PagerDuty notifications (Blocked: Missing PagerDuty integration details/credentials)
- OBS-11: Slack notifications (Blocked: Missing Slack integration details/credentials)
- OBS-12: Automatic recovery actions (Blocked: Missing remediation engine)
- OBS-13: System diagnostics (Blocked: Missing diagnostic tools)
- OBS-14: Public status page (Blocked: Missing frontend rendering component)
- OBS-15: Incident management (Implemented)

## Integration Needs
- Need to expose Prometheus endpoint in the main HTTP server.
- Need to run periodic probes for configured endpoints.
- Require configuration structure for webhooks/alerts.
