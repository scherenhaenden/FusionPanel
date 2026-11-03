# Deployments Integration

Connects the `modules/deployments` functionality into a web service using `axum`.

## API Routes

- `POST /webhook` - Receives GitHub push events, validating HMAC-SHA256 signatures and replay protection.
- `POST /deployments` - Creates a new deployment (initiator requested).
- `POST /deployments/:id/approve` - Approves a deployment by a different user and executes the directory switch.
- `POST /deployments/:id/rollback` - Manually rolls back a deployment to the previous version.
- `POST /deployments/:id/health_failure` - Simulates a health check failure resulting in a rollback.
- `GET /deployments/:id` - Gets the deployment record status.

## Missing Features / Capabilities blocked

- DEP-01: Needs actual git fetch mechanism attached to git push daemon in real usage (mocked locally using simple fs mapping in integration).
- DEP-03: Real container isolated build runner caching environments.
- DEP-04, DEP-06: Reverse proxy configuration/real orchestrator mappings.
- DEP-05: Real DNS zone automation integration.
- DEP-08: Full cron scheduled worker.

These are explicitly missing and currently documented as blocked pending additional component deliveries.
