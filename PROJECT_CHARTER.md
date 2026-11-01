# Project Charter

## Mission

Build FusionPanel as a coherent server administration product covering traditional hosting, containers, automated deployments, security, backups, databases, WordPress operations, monitoring, multi-node management, DNS, email, administration and migration.

The owner sets priorities, approves release scope and authorizes production changes. Implementation is staged; scheduling a capability later does not remove it from the final product scope.

## Deliverables

A unified UI/API/CLI, control plane, node agent, durable operations, installation/update and recovery tooling, capability acceptance evidence, operator documentation and qualified migration tooling.

## Initial implementation milestone

The first five parallel work packages establish domain types, a control-plane API, a node protocol, a persistent job store and an administrative CLI. A subsequent integration task connects these components, owns the shared lockfile and validates the workspace. No remote agent may merge its own pull request.

## Governance

Track work in GitHub issues and pull requests. Each task has exclusive write paths, acceptance criteria and tests. Record architecture decisions and limitations in English. Preserve applicable license and copyright notices for any introduced material. Production infrastructure changes require their own approved execution plan.

## Delivery window and preservation

The Jules window ends on 11 November 2026. The detailed execution plan assigns all 162 capabilities and plans 63 work packages with integration and real-system qualification gates. Full product scope remains committed as a goal; unproven features stay blocked/partial rather than being removed or claimed complete. Preserve alternative user workflows through explicitly validated selectable modes.
