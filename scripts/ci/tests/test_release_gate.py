import unittest
from unittest.mock import Mock

from scripts.ci.release_gate import (
    CI_WORKFLOW,
    E2E_GATE,
    FULL_GATE,
    REUSED_GATE,
    GitHubActionsEvidence,
    docs_only,
    should_skip_ci,
)


REPO = {"id": 7, "full_name": "scherenhaenden/FusionPanel"}


def event(
    action="synchronize",
    head_ref="sprint-2",
    base_ref="master",
    *,
    head_sha="head",
    base_sha="base",
    pr_number=590,
):
    return {
        "action": action,
        "pull_request": {
            "number": pr_number,
            "head": {"ref": head_ref, "sha": head_sha, "repo": REPO},
            "base": {"ref": base_ref, "sha": base_sha, "repo": REPO},
        },
    }


def run_record(
    run_id=1,
    *,
    head_sha="head",
    base_sha="base",
    base_ref="master",
    head_ref="sprint-2",
    pr_number=590,
    started="2026-11-16T10:00:00Z",
    attempt=1,
    conclusion="success",
):
    return {
        "id": run_id,
        "path": CI_WORKFLOW,
        "head_sha": head_sha,
        "event": "pull_request",
        "run_started_at": started,
        "run_attempt": attempt,
        "status": "completed",
        "conclusion": conclusion,
        "html_url": f"https://github.com/scherenhaenden/FusionPanel/actions/runs/{run_id}",
        "pull_requests": [
            {
                "number": pr_number,
                "head": {"ref": head_ref, "sha": head_sha, "repo": REPO},
                "base": {"ref": base_ref, "sha": base_sha, "repo": REPO},
            }
        ],
    }


def job(name, conclusion="success"):
    return {"name": name, "status": "completed", "conclusion": conclusion}


def full_gate_jobs(matrix_conclusion="success"):
    return [
        job("discovery"),
        job("validation"),
        job("dependency-lane"),
        job(E2E_GATE),
        job(FULL_GATE),
        job("compile-and-unit-cargo (Cargo.toml)", matrix_conclusion),
        job("compile-and-unit-npm (frontend/package.json)"),
    ]


def evidence_for(run, jobs):
    evidence = object.__new__(GitHubActionsEvidence)
    evidence.current_run_id = None
    evidence.reused_gate = None
    evidence.get_json = Mock(side_effect=[
        {"total_count": 1, "workflow_runs": [run]},
        {"total_count": len(jobs), "jobs": jobs},
    ])
    return evidence


class ReleaseGateTests(unittest.TestCase):
    def test_docs_allowlist_is_strict(self):
        self.assertTrue(docs_only(["AGENTS.md", "docs/execution/SPRINTS.md"]))
        self.assertFalse(docs_only(["README.md"]))
        self.assertFalse(docs_only(["docs/ci-settings.yml"]))
        self.assertFalse(docs_only(["docs/guide.md", "crates/api/src/lib.rs"]))

    def test_only_targeted_reopen_or_sync_can_skip(self):
        passed = lambda _sha, _event: "passed"
        parents = lambda _sha: ["parent", "topic"]
        changed = lambda _base, _head: ["docs/only.md"]
        self.assertFalse(should_skip_ci(event("opened"), passed, parents, changed))
        self.assertFalse(should_skip_ci(event(head_ref="feat/sprint-2-api"), passed, parents, changed))
        self.assertFalse(should_skip_ci(event(base_ref="sprint-3"), passed, parents, changed))

    def test_reopen_reuses_only_exact_head_evidence(self):
        state = lambda sha, _event: "passed" if sha == "head" else "absent"
        self.assertTrue(should_skip_ci(event("reopened"), state, Mock(), Mock()))
        self.assertFalse(
            should_skip_ci(event("reopened"), lambda _sha, _event: "absent", Mock(), Mock())
        )

    def test_sync_reuses_two_parent_docs_only_change(self):
        state = lambda sha, _event: "passed" if sha == "parent" else "absent"
        parents = lambda sha: ["parent", "topic"] if sha == "head" else []
        paths = lambda _base, _head: ["AGENTS.md", "docs/plan.md"]
        self.assertTrue(should_skip_ci(event(), state, parents, paths))

    def test_sync_code_or_non_merge_commit_runs_full_gate(self):
        state = lambda _sha, _event: "passed"
        self.assertFalse(
            should_skip_ci(event(), state, lambda _sha: ["parent"], Mock())
        )
        self.assertFalse(
            should_skip_ci(
                event(),
                state,
                lambda _sha: ["parent", "topic"],
                lambda _base, _head: ["crates/api/src/lib.rs"],
            )
        )

    def test_absent_failed_or_unknown_parent_evidence_runs_full_gate(self):
        parents = lambda sha: ["parent", "topic"] if sha == "head" else []
        paths = lambda _base, _head: ["docs/plan.md"]
        for state in ("absent", "failed", "unknown"):
            with self.subTest(state=state):
                self.assertFalse(
                    should_skip_ci(event(), lambda _sha, _event: state, parents, paths)
                )

    def test_latest_run_matches_pr_head_base_and_ignores_current_run(self):
        evidence = object.__new__(GitHubActionsEvidence)
        evidence.current_run_id = "3"
        wrong_base = run_record(1, base_sha="old-base")
        valid = run_record(2, started="2026-11-16T11:00:00Z")
        current = run_record(3, started="2026-11-16T12:00:00Z")
        evidence.get_json = Mock(
            return_value={"total_count": 3, "workflow_runs": [wrong_base, valid, current]}
        )
        self.assertEqual(evidence.latest_run("head", event())["id"], 2)

    def test_latest_run_prefers_newer_attempt(self):
        evidence = object.__new__(GitHubActionsEvidence)
        evidence.current_run_id = None
        earlier = run_record(1, started="2026-11-16T10:00:00Z", attempt=1)
        latest = run_record(2, started="2026-11-16T11:00:00Z", attempt=2)
        evidence.get_json = Mock(
            return_value={"total_count": 2, "workflow_runs": [earlier, latest]}
        )
        self.assertEqual(evidence.latest_run("head", event())["id"], 2)

    def test_workflow_list_pagination_is_uncertain(self):
        evidence = object.__new__(GitHubActionsEvidence)
        evidence.current_run_id = None
        evidence.get_json = Mock(return_value={"total_count": 101, "workflow_runs": []})
        with self.assertRaises(RuntimeError):
            evidence.latest_run("head", event())

    def test_full_gate_requires_e2e_sentinel_and_successful_matrix(self):
        evidence = evidence_for(run_record(), full_gate_jobs())
        self.assertEqual(evidence.gate_state("head", event()), "passed")

        failed_matrix = evidence_for(run_record(), full_gate_jobs("failure"))
        self.assertEqual(failed_matrix.gate_state("head", event()), "failed")

        missing_e2e = evidence_for(
            run_record(), [job(name) for name in ("discovery", "validation", "dependency-lane", FULL_GATE)]
        )
        self.assertEqual(missing_e2e.gate_state("head", event()), "failed")

    def test_reuse_marker_is_a_valid_continuation_of_the_full_gate(self):
        evidence = evidence_for(
            run_record(), [job("discovery"), job(REUSED_GATE)]
        )
        self.assertEqual(evidence.gate_state("head", event()), "passed")

        missing_discovery = evidence_for(run_record(), [job(REUSED_GATE)])
        self.assertEqual(missing_discovery.gate_state("head", event()), "failed")

    def test_two_consecutive_docs_updates_reuse_exact_gate_markers(self):
        first_parent_gate = evidence_for(
            run_record(head_sha="qualified"), full_gate_jobs()
        )
        first_update = event(head_sha="first-docs-update")
        first_skip = should_skip_ci(
            first_update,
            first_parent_gate.gate_state,
            lambda _sha: ["qualified", "topic"],
            lambda _base, _head: ["docs/first.md"],
        )
        self.assertTrue(first_skip)

        first_update_marker = evidence_for(
            run_record(head_sha="first-docs-update"),
            [job("discovery"), job(REUSED_GATE)],
        )
        second_update = event(head_sha="second-docs-update")
        self.assertTrue(
            should_skip_ci(
                second_update,
                first_update_marker.gate_state,
                lambda _sha: ["first-docs-update", "topic"],
                lambda _base, _head: ["docs/second.md"],
            )
        )

    def test_docs_update_does_not_reuse_wrong_pr_base_or_failed_marker(self):
        docs_event = event(head_sha="next-docs-update", base_sha="new-base")
        parents = lambda _sha: ["prior-docs-update", "topic"]
        paths = lambda _base, _head: ["docs/next.md"]

        wrong_base = evidence_for(
            run_record(head_sha="prior-docs-update", base_sha="old-base"),
            [job("discovery"), job(REUSED_GATE)],
        )
        self.assertFalse(
            should_skip_ci(docs_event, wrong_base.gate_state, parents, paths)
        )

        wrong_pr_run = run_record(head_sha="prior-docs-update", base_sha="new-base", pr_number=591)
        wrong_pr = evidence_for(wrong_pr_run, [job("discovery"), job(REUSED_GATE)])
        self.assertFalse(should_skip_ci(docs_event, wrong_pr.gate_state, parents, paths))

        failed_discovery = evidence_for(
            run_record(head_sha="prior-docs-update", base_sha="new-base"),
            [job("discovery", "failure"), job(REUSED_GATE)],
        )
        self.assertFalse(
            should_skip_ci(docs_event, failed_discovery.gate_state, parents, paths)
        )

        missing_discovery = evidence_for(
            run_record(head_sha="prior-docs-update", base_sha="new-base"),
            [job(REUSED_GATE)],
        )
        self.assertFalse(
            should_skip_ci(docs_event, missing_discovery.gate_state, parents, paths)
        )

    def test_missing_marker_or_failed_workflow_is_not_reusable(self):
        no_marker = evidence_for(run_record(), [job("discovery")])
        self.assertEqual(no_marker.gate_state("head", event()), "absent")

        failed_run = evidence_for(run_record(conclusion="failure"), [])
        self.assertEqual(failed_run.gate_state("head", event()), "failed")

    def test_jobs_are_read_from_the_selected_attempt(self):
        evidence = object.__new__(GitHubActionsEvidence)
        evidence.reused_gate = None
        evidence.latest_run = Mock(return_value=run_record(42, attempt=2))
        evidence.get_json = Mock(return_value={"total_count": 7, "jobs": full_gate_jobs()})
        self.assertEqual(evidence.gate_state("head", event()), "passed")
        evidence.get_json.assert_called_once_with(
            "actions/runs/42/attempts/2/jobs", {"per_page": 100}
        )

    def test_api_errors_propagate_for_full_gate_fallback(self):
        evidence = object.__new__(GitHubActionsEvidence)
        evidence.latest_run = Mock(return_value=run_record())
        evidence.get_json = Mock(side_effect=OSError("API unavailable"))
        with self.assertRaises(OSError):
            evidence.gate_state("head", event())

    def test_job_pagination_is_uncertain(self):
        evidence = object.__new__(GitHubActionsEvidence)
        evidence.latest_run = Mock(return_value=run_record())
        evidence.get_json = Mock(return_value={"total_count": 101, "jobs": []})
        with self.assertRaises(RuntimeError):
            evidence.gate_state("head", event())


if __name__ == "__main__":
    unittest.main()
