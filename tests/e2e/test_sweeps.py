"""
Tests for sweep list and sweep detail pages.
"""
from __future__ import annotations

import pytest
from playwright.sync_api import Page, expect


def test_sweeps_list_empty_state(
    live_server, test_project, authed_page: Page
):
    """An empty project shows the 'no sweeps yet' message, not a loading spinner."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps")
    authed_page.wait_for_timeout(2000)

    body = authed_page.locator("#sweepsTable").text_content() or ""
    assert "Loading" not in body, f"still loading after 2s: {body}"
    assert "sweeps yet" in body.lower() or "No sweeps" in body


def test_sweeps_list_with_data(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """After pushing a sweep, the list shows the sweep with status and config."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps")
    authed_page.wait_for_timeout(2000)

    body = authed_page.locator("#sweepsTable").text_content() or ""
    assert "Loading" not in body
    # Should show the sweep (first 8 chars of sweep id or status)
    assert "completed" in body.lower() or pushed_sweep[:8] in body


def test_sweeps_list_row_navigates_to_detail(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """Clicking a sweep row navigates to the sweep detail page."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps")
    authed_page.wait_for_timeout(2000)

    # Click the first sweep link
    sweep_link = authed_page.locator(f"a[href*='/sweeps/{pushed_sweep}']").first
    expect(sweep_link).to_be_visible()
    sweep_link.click()

    authed_page.wait_for_url(f"**/{pushed_sweep}**")
    assert pushed_sweep[:8] in authed_page.url


def test_sweep_detail_kpi_strip(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """Sweep detail page shows the KPI strip with run counts."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(3000)

    # KPI: total runs should be 2 (from the fixture)
    total = authed_page.locator("#kpiTotal").text_content() or ""
    assert total == "2", f"expected 2 total runs, got {total!r}"

    completed = authed_page.locator("#kpiCompleted").text_content() or ""
    assert completed == "2", f"expected 2 completed, got {completed!r}"

    failed = authed_page.locator("#kpiFailed").text_content() or ""
    assert failed == "0"


def test_sweep_detail_flat_runs_table(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """Sweep detail page shows the flat runs table with child run rows."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(3000)

    # The runs table should be visible and have rows
    runs_section = authed_page.locator("#sweepRunsTable")
    expect(runs_section).to_be_visible()

    rows = authed_page.locator("#sweepRunRows tr")
    assert rows.count() == 2, f"expected 2 run rows, got {rows.count()}"


def test_sweep_detail_matrix_renders(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """Sweep detail shows the matrix when runs vary on backend axis."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(3000)

    matrix = authed_page.locator(".sweep-matrix")
    expect(matrix).to_be_visible()

    # The fixture uses backends: mock and anthropic -- should be in the matrix
    matrix_text = matrix.text_content() or ""
    assert "mock" in matrix_text or "anthropic" in matrix_text


def test_sweep_detail_not_stuck_loading(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """The 'Loading runs...' placeholder clears once data arrives."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(3000)

    matrix_text = authed_page.locator("#sweepMatrix").text_content() or ""
    assert "Loading" not in matrix_text, f"still loading: {matrix_text}"
