# Observability Module

This module implements contracts, Prometheus integration, probes, alert delivery and incident states for the FusionPanel observability domain.

## Shared Workspace Modifications

To integrate this module correctly, the following `.gitignore` modifications should be applied in the shared workspace root (do not commit them locally in this batch):

```
# Exclude observability module build artifacts
modules/observability/target/
```

## E2E Case Definitions

- Target: E2E outage notification (BLOCKED: pending scheduler, collector, and incident persistence integration)
  - Trigger: Kill disposable fixture mock application PID (`kill <mock_pid>`)
  - Verify: Metric `site_requests_total` drops; HTTP probe fails
  - Expect: New `Incident` triggered and `Alert` JSON payload POSTed to the webhook endpoint.
  - Recovery: Restart mock application, probe recovers, incident auto-resolves.

## Capabilities

The following capabilities track the existing independent module primitives. Live integration is not yet complete:
- OBS-01: CPU, RAM and disk metrics (Primitive implemented)
- OBS-02: Metrics history (Blocked: Time-series database dependency missing)
- OBS-03: Live network traffic (Primitive implemented: Network RX/TX counters)
- OBS-04: Per-site metrics (Primitive implemented: Site requests counter)
- OBS-05: Per-container metrics (Primitive implemented: Container CPU gauge)
- OBS-06: HTTP uptime checks (Primitive implemented)
- OBS-07: TCP and ping checks (Primitive implemented TCP, Blocked ping: Missing root privileges/ICMP raw socket permissions)
- OBS-08: Prometheus endpoint (Primitive implemented)
- OBS-09: Configurable alerts (Primitive implemented)
- OBS-10: PagerDuty notifications (Blocked: Missing PagerDuty integration details/credentials)
- OBS-11: Slack notifications (Blocked: Missing Slack integration details/credentials)
- OBS-12: Automatic recovery actions (Blocked: Missing remediation engine)
- OBS-13: System diagnostics (Blocked: Missing diagnostic tools)
- OBS-14: Public status page (Blocked: Missing frontend rendering component)
- OBS-15: Incident management (Primitive implemented)

## Integration Needs
- Need to expose Prometheus endpoint in the main HTTP server.
- Need to run periodic probes for configured endpoints.
- Require configuration structure for webhooks/alerts.
