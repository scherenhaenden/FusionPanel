# Contributing to FusionPanel

FusionPanel is open-source software licensed under AGPL-3.0-only and is in early development. Use English for documentation, code comments, issues and pull requests.

Start from master and a scoped issue. Read AGENTS.md, the architecture, feature-preservation policy, variant design and real-system test strategy. Reserve exclusive write paths; propose shared contract changes before implementation. Preserve existing capabilities and required license/copyright notices. Use appropriately licensed dependencies and original or permitted material.

A PR must state the user outcome, capability/mode IDs, behavior changes, actual unit/integration/E2E commands and results, supported environment, limitations and recovery impact. Do not claim mock-only coverage as real-service acceptance. Never commit credentials, private keys, personal data, production dumps or unsanitized logs.

Use disposable services and VMs for integration tests. Do not run host mutation tests on a contributor workstation or a production server. CI/unit success alone is not a production-readiness claim.
