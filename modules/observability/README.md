# Observability Module

This module implements contracts, Prometheus integration, probes, alert delivery and incident states for the FusionPanel observability domain.

## Shared Workspace Modifications

To integrate this module correctly, the following `.gitignore` modifications should be applied in the shared workspace root (do not commit them locally in this batch):

```
# Exclude observability module build artifacts
modules/observability/target/
```

## E2E Case Definitions

- Target: E2E outage notification (BLOCKED until a scheduled probe/alert runtime is integrated).
  - Dependency: a disposable fixture process, probe scheduler, rule evaluator, durable incident store and webhook receiver.
  - Trigger: terminate only the PID launched by the isolated test fixture; never signal host services.
  - Verify: the real integrated collector records probe failure, creates an incident and POSTs its alert; then restart the fixture PID, observe recovery and resolve the incident.
  - Current evidence: this module provides probe, threshold, payload and incident primitives; it does not run a scheduler, collector, alert dispatcher or persistence. No end-to-end outage notification has passed.

## Capabilities

The following capabilities are tracked at module-slice level; “Implemented” means a module primitive exists, not that FusionPanel has integrated or accepted the live service. Remaining acceptance is assigned to the runtime/E2E follow-up tasks:
- OBS-01: CPU, RAM and disk metric types (Implemented as primitives; host collection/integration Blocked)
- OBS-02: Metrics history (Blocked: no durable time-series storage)
- OBS-03: Network RX/TX counter types (Implemented as primitives; live collection Blocked)
- OBS-04: Per-site metrics (Partial: counter exists without site labels or request instrumentation)
- OBS-05: Per-container metrics (Partial: gauge exists without container labels or collection)
- OBS-06: HTTP uptime checks (Implemented probe primitive; scheduling/history Blocked)
- OBS-07: TCP check (Implemented probe primitive; scheduling Blocked); ping (Blocked: no ICMP implementation)
- OBS-08: Prometheus exposition registry (Implemented; HTTP endpoint integration Blocked)
- OBS-09: Alert rule evaluation (Implemented; configuration, scheduling and delivery integration Blocked)
- OBS-10: PagerDuty notifications (Blocked: Missing PagerDuty integration details/credentials)
- OBS-11: Slack notifications (Blocked: Missing Slack integration details/credentials)
- OBS-12: Automatic recovery actions (Blocked: Missing remediation engine)
- OBS-13: System diagnostics (Blocked: Missing diagnostic tools)
- OBS-14: Public status page (Blocked: Missing frontend rendering component)
- OBS-15: Incident state primitives (Implemented; persistent service and UI integration Blocked)

## Integration Needs
- Need to expose Prometheus endpoint in the main HTTP server.
- Need to run periodic probes for configured endpoints.
- Require configuration structure for webhooks/alerts.
