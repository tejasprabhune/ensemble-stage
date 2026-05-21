"""
Tests for the compare view.
"""
from __future__ import annotations

import uuid

import pytest
import requests
from playwright.sync_api import Page, expect


@pytest.fixture
def two_runs(test_project, test_user, live_server):
    """Push two completed runs and return their IDs."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    run_ids = []

    for i, score in enumerate([0.8, 0.6]):
        run_id = str(uuid.uuid4())
        requests.post(
            f"{live_server}/v1/projects/{org}/{proj}/runs",
            headers=headers,
            json={"id": run_id, "scenario": "plank.smoke", "world": "plank", "backend": "mock"},
        )
        requests.post(
            f"{live_server}/v1/runs/{run_id}/events",
            headers=headers,
            json={"events": [
                {"sequence_number": 0, "kind": "user_message", "event_id": str(uuid.uuid4()), "wall_time_ms": 1000,
                 "payload": {"actor": "alice", "message_id": "m1", "payload": {"kind": "user_message", "text": f"Run {i}: help!"}, "tick": 0, "ts_ms": 1000}},
            ]},
        )
        requests.post(
            f"{live_server}/v1/runs/{run_id}/status",
            headers=headers,
            json={"status": "completed", "outcome": {"score": score}, "wall_time_ms": 1000},
        )
        run_ids.append(run_id)

    return run_ids


def test_compare_picker_renders(
    live_server, test_project, authed_page: Page
):
    """The compare page shows the picker form when no run IDs are in the URL."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/compare")

    expect(authed_page.get_by_text("Compare two runs")).to_be_visible()
    expect(authed_page.locator("input[name=a]")).to_be_visible()
    expect(authed_page.locator("input[name=b]")).to_be_visible()


def test_compare_view_with_two_runs(
    live_server, test_project, two_runs, authed_page: Page
):
    """Passing ?a=...&b=... loads the comparison view with metadata and timeline."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    run_a, run_b = two_runs

    authed_page.goto(f"{live_server}/{org}/{proj}/compare?a={run_a}&b={run_b}")
    authed_page.wait_for_timeout(3000)

    # Should show "Loading runs..." resolved to real content
    body_text = authed_page.locator("#compareBody").text_content() or ""
    assert "Loading" not in body_text, f"compare still loading: {body_text}"

    # Metadata table should appear
    meta_table = authed_page.locator(".compare-meta-table")
    expect(meta_table).to_be_visible()

    # Run IDs appear as links in the header row
    meta_text = meta_table.text_content() or ""
    assert run_a[:8] in meta_text
    assert run_b[:8] in meta_text


def test_compare_view_highlights_different_scores(
    live_server, test_project, two_runs, authed_page: Page
):
    """Rows with different values between the two runs get the diff CSS class."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    run_a, run_b = two_runs

    authed_page.goto(f"{live_server}/{org}/{proj}/compare?a={run_a}&b={run_b}")
    authed_page.wait_for_timeout(3000)

    # The 'score' row should be highlighted because 0.8 != 0.6
    diff_rows = authed_page.locator(".compare-row-diff")
    assert diff_rows.count() > 0, "expected at least one highlighted difference row"


def test_compare_timeline_shows_events(
    live_server, test_project, two_runs, authed_page: Page
):
    """The aligned timeline table appears with event rows."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    run_a, run_b = two_runs

    authed_page.goto(f"{live_server}/{org}/{proj}/compare?a={run_a}&b={run_b}")
    authed_page.wait_for_timeout(3000)

    timeline = authed_page.locator(".compare-timeline-table")
    expect(timeline).to_be_visible()

    rows = authed_page.locator(".compare-timeline-table tbody tr")
    assert rows.count() > 0


def test_compare_c_shortcut_with_two_checked(
    live_server, test_project, two_runs, authed_page: Page
):
    """Checking two rows on the project page and pressing c navigates to compare."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}")
    authed_page.wait_for_load_state("networkidle")

    # Check first two run rows
    checkboxes = authed_page.locator(".runs-table tbody input[type=checkbox]")
    expect(checkboxes.nth(0)).to_be_visible()
    checkboxes.nth(0).check()
    checkboxes.nth(1).check()

    # Blur any focused input so the app.js inInput guard doesn't block the shortcut
    authed_page.evaluate("document.activeElement && document.activeElement.blur()")
    authed_page.keyboard.press("c")
    authed_page.wait_for_url("**/compare**", timeout=5000)

    assert "/compare" in authed_page.url
    assert "?a=" in authed_page.url
