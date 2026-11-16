#!/usr/bin/env python3
"""Reuse a verified Sprint 2 CI gate for docs-only PR updates."""

import json
import os
from pathlib import PurePosixPath
import subprocess
import sys
from urllib.parse import quote, urlencode, urlparse
from urllib.request import Request, urlopen

CI_WORKFLOW = ".github/workflows/ci.yml"
FULL_GATE = "Full Sprint 2 release gate"
REUSED_GATE = "Sprint 2 docs-only gate reuse"
E2E_GATE = "Sprint 2 browser E2E (Docker)"


def git(*args):
    return subprocess.run(
        ["git", *args], check=True, text=True, capture_output=True
    ).stdout.strip()


def docs_only(paths):
    return all(
        path == "AGENTS.md"
        or (path.startswith("docs/") and PurePosixPath(path).suffix.lower() == ".md")
        for path in paths
    )


def repo_name(repo):
    if not isinstance(repo, dict):
        return None
    if repo.get("full_name"):
        return repo["full_name"].lower()
    url = repo.get("url")
    parts = [part for part in urlparse(url).path.split("/") if part] if url else []
    return "/".join(parts[-2:]).lower() if len(parts) >= 2 else None


def same_repo(expected, actual):
    if not isinstance(expected, dict) or not isinstance(actual, dict):
        return False
    if expected.get("id") is not None and actual.get("id") is not None:
        return expected["id"] == actual["id"]
    return repo_name(expected) is not None and repo_name(expected) == repo_name(actual)


def should_skip_ci(event, gate_state, parents_for_sha, changed_paths):
    action = event.get("action")
    pr = event.get("pull_request") or {}
    base = pr.get("base") or {}
    head = pr.get("head") or {}
    head_sha = head.get("sha")

    if (
        action not in {"reopened", "synchronize"}
        or not head_sha
        or base.get("ref") != "master"
        or head.get("ref") != "sprint-2"
        or not base.get("sha")
        or not pr.get("number")
    ):
        return False

    if action == "reopened":
        return gate_state(head_sha, event) == "passed"

    parents = parents_for_sha(head_sha)
    if len(parents) != 2:
        return False
    parent = parents[0]
    if not docs_only(changed_paths(parent, head_sha)):
        return False
    return gate_state(parent, event) == "passed"


class GitHubActionsEvidence:
    """Read prior Actions evidence; callers fail closed if the API is unavailable."""

    def __init__(self):
        self.api_url = os.environ.get("GITHUB_API_URL", "https://api.github.com").rstrip("/")
        self.repository = quote(os.environ["GITHUB_REPOSITORY"], safe="/")
        self.token = os.environ["GITHUB_TOKEN"]
        self.current_run_id = os.environ.get("GITHUB_RUN_ID")
        self.reused_gate = None

    def get_json(self, path, query=None):
        suffix = f"?{urlencode(query)}" if query else ""
        request = Request(
            f"{self.api_url}/repos/{self.repository}/{path}{suffix}",
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self.token}",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        with urlopen(request, timeout=20) as response:
            return json.load(response)

    def latest_run(self, sha, event):
        pr = event["pull_request"]
        base = pr["base"]
        head = pr["head"]
        response = self.get_json(
            "actions/runs",
            {"head_sha": sha, "event": "pull_request", "per_page": 100},
        )
        runs = response.get("workflow_runs")
        total = response.get("total_count")
        if not isinstance(runs, list) or len(runs) >= 100 or (total is not None and total > len(runs)):
            raise RuntimeError("workflow-run results are incomplete or paginated")

        matches = []
        for run in runs:
            if (
                str(run.get("id")) == self.current_run_id
                or run.get("head_sha") != sha
                or run.get("event") != "pull_request"
                or run.get("path") != CI_WORKFLOW
            ):
                continue
            associated = run.get("pull_requests")
            if not isinstance(associated, list):
                raise RuntimeError("workflow-run PR metadata is missing")
            for candidate in associated:
                candidate_head = candidate.get("head") or {}
                candidate_base = candidate.get("base") or {}
                if candidate.get("number") != pr["number"] or candidate_head.get("sha") != sha:
                    continue
                if (
                    candidate_base.get("ref") != base.get("ref")
                    or candidate_base.get("sha") != base.get("sha")
                    or candidate_head.get("ref") != head.get("ref")
                ):
                    continue
                if not same_repo(base.get("repo"), candidate_base.get("repo")) or not same_repo(
                    head.get("repo"), candidate_head.get("repo")
                ):
                    raise RuntimeError("workflow-run repository metadata does not match the PR")
                matches.append(run)
                break

        if not matches:
            return None
        return max(
            matches,
            key=lambda run: (
                run.get("run_started_at") or run.get("created_at") or "",
                run.get("run_attempt", 0),
                run.get("id", 0),
            ),
        )

    def gate_state(self, sha, event):
        run = self.latest_run(sha, event)
        if not run:
            return "absent"
        if run.get("status") != "completed" or run.get("conclusion") != "success":
            return "failed"

        attempt = run.get("run_attempt")
        response = self.get_json(
            f"actions/runs/{run['id']}/attempts/{attempt}/jobs", {"per_page": 100}
        )
        jobs = response.get("jobs")
        total = response.get("total_count")
        if not isinstance(jobs, list) or len(jobs) >= 100 or (total is not None and total > len(jobs)):
            raise RuntimeError("workflow-job results are incomplete or paginated")
        by_name = {job.get("name"): job for job in jobs}

        def succeeded(name):
            job = by_name.get(name) or {}
            return job.get("status") == "completed" and job.get("conclusion") == "success"

        if succeeded(REUSED_GATE):
            if not succeeded("discovery"):
                return "failed"
        elif succeeded(FULL_GATE):
            required = {"discovery", "validation", "dependency-lane", E2E_GATE}
            if not all(succeeded(name) for name in required):
                return "failed"
            for job in jobs:
                name = job.get("name") or ""
                if name.startswith(("compile-and-unit-cargo (", "compile-and-unit-npm (")):
                    if job.get("status") != "completed" or job.get("conclusion") != "success":
                        return "failed"
        else:
            return "absent"

        self.reused_gate = {"sha": sha, "base": event["pull_request"]["base"]["sha"], "url": run.get("html_url", "")}
        return "passed"


def main():
    with open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8") as event_file:
        event = json.load(event_file)
    evidence = GitHubActionsEvidence()

    try:
        skip = should_skip_ci(
            event,
            evidence.gate_state,
            lambda sha: git("rev-list", "--parents", "-n", "1", sha).split()[1:],
            lambda base, head: git(
                "diff", "--name-only", "--no-renames", base, head
            ).splitlines(),
        )
    except Exception as error:
        print(f"Gate-reuse check unavailable; running full gates: {error}")
        skip = False

    if skip and evidence.reused_gate:
        item = evidence.reused_gate
        print(f"Reusing validated gate {item['sha']} against base {item['base']}: {item['url']}")
    elif not skip:
        print("No reusable exact-base gate; running the full Sprint 2 CI gate.")
    value = "false" if skip else "true"
    print(f"run_release_gate={value}")
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(f"run_release_gate={value}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
