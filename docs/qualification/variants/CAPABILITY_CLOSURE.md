# Variant Compatibility Qualification

## Objective

Test each selectable user workflow and incompatible combination against the feature registry to ensure explicit compatibility rules and safe transitions.

## Capability Evidence

| Capability ID | Description | Acceptance Criteria | Tests | Status |
|---|---|---|---|---|
| S58-001 | Feature registry queries | Returns valid default, scope and prerequisites | `tests/e2e/variants/tests/qualification.rs`<br>`tests/e2e/browser/tests/variants.spec.js` | BLOCKED |
| S58-002 | Valid transitions | Transitions respect dependency rules and locks | `tests/e2e/variants/tests/qualification.rs`<br>`tests/e2e/browser/tests/variants.spec.js` | BLOCKED |
| S58-003 | Invalid transitions | Unmet prerequisites fail safely without changes | `tests/e2e/variants/tests/qualification.rs`<br>`tests/e2e/browser/tests/variants.spec.js` | BLOCKED |
| S58-004 | Data persistence | Backup boundary spans transition artifacts | `tests/e2e/variants/tests/qualification.rs`<br>`tests/e2e/browser/tests/variants.spec.js` | BLOCKED |

These live API and browser lanes require an explicitly owned disposable loopback target, token, and test-site ID. The current frontend does not expose the variant registry or site-variant routes, and the API has no verified variant-transition route. They remain BLOCKED until those capabilities and fixtures are available; a page reload is not evidence of persistence across a service restart.
