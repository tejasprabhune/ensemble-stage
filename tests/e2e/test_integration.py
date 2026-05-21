"""
Cross-repo integration tests: ensemble CLI pushing to a live Stage server.

These tests run a real ensemble scenario against the local test Stage
server and verify that events, metadata, and scores reach Stage correctly.
This is the test that would have caught the wire format divergence fixed
in the preceding session.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest
import requests
from playwright.sync_api import Page, expect

ENSEMBLE_REPO = Path(__file__).resolve().parents[3] / "ensemble"
ENSEMBLE_CLI = Path(os.path.expanduser("~/.cargo/bin/ensemble"))


def _ensemble_available() -> bool:
    return ENSEMBLE_CLI.exists() and ENSEMBLE_REPO.is_dir()


pytestmark = pytest.mark.skipif(
    not _ensemble_available(),
    reason="ensemble CLI or repo not found",
)


def test_run_pushes_events_to_stage(live_server, test_project, test_user):
    """ensemble run plank.refund_storm with mock backend pushes events and
    scores to Stage; the events endpoint returns them and the status
    endpoint shows the correct outcome dict."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    env = {
        **os.environ,
        "ENSEMBLE_STAGE_PROJECT": f"{org}/{proj}",
        "ENSEMBLE_STAGE_API_KEY": test_user["api_key"],
        "ENSEMBLE_STAGE_BASE_URL": live_server,
        "ENSEMBLE_QUIET": "1",
    }

    result = subprocess.run(
        [str(ENSEMBLE_CLI), "run", "plank.refund_storm"],
        env=env,
        cwd=str(ENSEMBLE_REPO),
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert result.returncode == 0, f"ensemble run failed:\n{result.stderr}"

    # Parse run_id from CLI output
    output = result.stdout.strip()
    last_line = output.splitlines()[-1]
    summary = json.loads(last_line)
    run_id = summary["run_id"]
    scores = summary["scores"]

    # Verify events reached Stage
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    events_resp = requests.get(
        f"{live_server}/v1/runs/{run_id}/events",
        headers=headers,
    )
    assert events_resp.status_code == 200
    events = events_resp.json()
    assert len(events) > 5, f"expected >5 events, got {len(events)}"

    # Verify run status and outcome on Stage
    run_resp = requests.get(
        f"{live_server}/v1/runs/{run_id}",
        headers=headers,
    )
    assert run_resp.status_code == 200
    run = run_resp.json()
    assert run["status"] == "completed"
    assert run["outcome"] is not None, "outcome should not be null"
    assert isinstance(run["outcome"], dict), "outcome should be the scores dict"

    # Each score key from the CLI must appear in Stage's stored outcome
    for key in scores:
        assert key in run["outcome"], f"score key {key!r} missing from Stage outcome"

    # wall_time_ms should be set (not zero)
    assert run["wall_time_ms"] is not None
    assert run["wall_time_ms"] > 0

    # started_at should be set
    assert run["started_at"] is not None


def test_run_detail_page_shows_events(
    live_server, test_project, test_user, authed_page: Page
):
    """After pushing a run, the run detail page renders timeline events,
    actor list, and outcome scores -- not empty placeholders."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    env = {
        **os.environ,
        "ENSEMBLE_STAGE_PROJECT": f"{org}/{proj}",
        "ENSEMBLE_STAGE_API_KEY": test_user["api_key"],
        "ENSEMBLE_STAGE_BASE_URL": live_server,
        "ENSEMBLE_QUIET": "1",
    }

    result = subprocess.run(
        [str(ENSEMBLE_CLI), "run", "plank.refund_storm"],
        env=env,
        cwd=str(ENSEMBLE_REPO),
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert result.returncode == 0
    last_line = result.stdout.strip().splitlines()[-1]
    run_id = json.loads(last_line)["run_id"]

    authed_page.goto(f"{live_server}/{org}/{proj}/runs/{run_id}")
    authed_page.wait_for_timeout(3000)

    # Timeline shows events (not 0/0)
    tick_label = authed_page.locator("#tickLabel").text_content()
    assert tick_label and not tick_label.startswith("0 / 0"), (
        f"expected events in timeline, got: {tick_label}"
    )

    # Actors panel has actor names
    actors_text = authed_page.locator("#messagesPanel").text_content()
    assert "alice" in (actors_text or "").lower() or "bob" in (actors_text or "").lower()

    # Outcome sidebar shows scores dict keys
    outcome_text = authed_page.locator("#metaOutcome").text_content() or ""
    assert "alice_refund_resolved" in outcome_text or "bob_no_unsolicited_upgrade" in outcome_text
