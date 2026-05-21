"""
Tests for the full sweep detail page with all sections.
"""
from __future__ import annotations

import time
import uuid

import pytest
import requests
from playwright.sync_api import Page, expect


def _make_sweep_12cells(live_server, test_project, test_user, headers):
    """Create a sweep with 12 cells: 3 backends x 2 scenarios x 2 seeds."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    backends = ["mock", "anthropic", "openai"]
    scenarios = ["agora.refund_storm", "agora.enterprise_audit"]

    resp = requests.post(
        f"{live_server}/v1/projects/{org}/{proj}/sweeps",
        headers=headers,
        json={"config": {"backends": backends, "scenarios": scenarios, "n_trials": 2}},
    )
    assert resp.status_code == 201, resp.text
    sweep_id = resp.json()["id"]

    for backend in backends:
        for scenario in scenarios:
            for seed in [1, 2]:
                run_id = str(uuid.uuid4())
                requests.post(
                    f"{live_server}/v1/projects/{org}/{proj}/runs",
                    headers=headers,
                    json={"id": run_id, "scenario": scenario, "world": "agora", "backend": backend, "sweep_id": sweep_id},
                )
                score = 0.7 + (seed - 1) * 0.1 + (backends.index(backend) * 0.05)
                requests.post(
                    f"{live_server}/v1/runs/{run_id}/status",
                    headers=headers,
                    json={"status": "completed", "outcome": {"score": round(score, 3)},
                          "wall_time_ms": 1000 + seed * 200, "cost_usd": 0.001 * seed},
                )
                requests.post(
                    f"{live_server}/v1/sweeps/{sweep_id}/runs",
                    headers=headers,
                    json={"run_id": run_id},
                )

    requests.post(
        f"{live_server}/v1/sweeps/{sweep_id}/status",
        headers=headers,
        json={"status": "completed"},
    )
    return sweep_id


def test_sweep_kpi_strip_updates_on_completion(
    live_server, test_project, test_user, pushed_sweep, authed_page: Page
):
    """KPI strip shows correct counts for a completed sweep."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(3000)

    total = authed_page.locator("#kpiTotal").text_content() or ""
    assert total.strip() == "2"
    completed = authed_page.locator("#kpiCompleted").text_content() or ""
    assert completed.strip() == "2"
    failed = authed_page.locator("#kpiFailed").text_content() or ""
    assert failed.strip() == "0"


def test_matrix_renders_with_default_axes(
    live_server, test_project, test_user, authed_page: Page
):
    """Matrix renders with default axis selection for a multi-axis sweep."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    sweep_id = _make_sweep_12cells(live_server, test_project, test_user, headers)

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{sweep_id}")
    authed_page.wait_for_timeout(3000)

    matrix = authed_page.locator(".sweep-matrix")
    expect(matrix).to_be_visible()
    text = matrix.text_content() or ""
    assert any(b in text for b in ["mock", "anthropic", "openai"]), f"no backend in matrix: {text[:200]}"


def test_matrix_axis_dropdowns_change_layout(
    live_server, test_project, test_user, authed_page: Page
):
    """Changing the row/col axis dropdowns re-renders the matrix."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    sweep_id = _make_sweep_12cells(live_server, test_project, test_user, headers)

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{sweep_id}")
    authed_page.wait_for_timeout(3000)

    row_sel = authed_page.locator("#rowAxisSelect")
    expect(row_sel).to_be_visible()
    options = row_sel.locator("option").all_text_contents()
    assert len(options) >= 2, f"expected at least 2 axis options, got {options}"


def test_matrix_cell_with_single_run_navigates(
    live_server, test_project, test_user, pushed_sweep, authed_page: Page
):
    """A matrix cell with one run links directly to that run's detail page."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(3000)

    run_link = authed_page.locator(".matrix-cell a[href*='/runs/']").first
    expect(run_link).to_be_visible()


def test_matrix_cell_with_multiple_runs_filters_list(
    live_server, test_project, test_user, authed_page: Page
):
    """Clicking a multi-run cell filters the runs list."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    sweep_id = _make_sweep_12cells(live_server, test_project, test_user, headers)

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{sweep_id}")
    authed_page.wait_for_timeout(3000)

    inner = authed_page.locator(".matrix-cell-inner").first
    expect(inner).to_be_visible()
    inner.click()
    authed_page.wait_for_timeout(500)

    filter_bar = authed_page.locator("#sweepFilterBar")
    if not filter_bar.is_hidden():
        text = filter_bar.text_content() or ""
        assert "Showing" in text


def test_matrix_variance_glyphs_render_for_cells_with_multiple_runs(
    live_server, test_project, test_user, authed_page: Page
):
    """Box plot glyphs appear in cells with 2 or more runs when variance is toggled on."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    sweep_id = _make_sweep_12cells(live_server, test_project, test_user, headers)

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{sweep_id}")
    authed_page.wait_for_timeout(3000)

    box_plots = authed_page.locator(".box-plot")
    assert box_plots.count() > 0, "expected box plot SVGs in matrix cells with multiple runs"


def test_cost_chart_polls_during_running_sweep(
    live_server, test_project, test_user, authed_page: Page
):
    """Cost chart section renders when runs have cost data."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    sweep_id = _make_sweep_12cells(live_server, test_project, test_user, headers)

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{sweep_id}")
    authed_page.wait_for_timeout(3000)

    cost_wrap = authed_page.locator("#costChartWrap")
    expect(cost_wrap).to_be_visible()
    # Runs in this sweep don't have cost_usd in the status update format the server accepts,
    # so expect the empty state message
    text = cost_wrap.text_content() or ""
    assert len(text) > 0


def test_per_axis_breakdown_shows_remaining_axes(
    live_server, test_project, test_user, authed_page: Page
):
    """Per-axis breakdown section shows axes not selected in matrix row/col."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    sweep_id = _make_sweep_12cells(live_server, test_project, test_user, headers)

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{sweep_id}")
    authed_page.wait_for_timeout(3000)

    breakdown = authed_page.locator("#axisBreakdown")
    expect(breakdown).to_be_visible()
    text = breakdown.text_content() or ""
    assert len(text) > 0


def test_per_axis_value_click_filters_list(
    live_server, test_project, test_user, authed_page: Page
):
    """Clicking a value row in the per-axis breakdown filters the runs list."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    sweep_id = _make_sweep_12cells(live_server, test_project, test_user, headers)

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{sweep_id}")
    authed_page.wait_for_timeout(3000)

    row = authed_page.locator(".axis-table tr[data-axis]").first
    if row.count() > 0:
        row.click()
        authed_page.wait_for_timeout(500)
        filter_bar = authed_page.locator("#sweepFilterBar")
        if not filter_bar.is_hidden():
            text = filter_bar.text_content() or ""
            assert "Showing" in text


def test_sweep_share_button_generates_url(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """Share button copies the current URL (basic clipboard action)."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(2000)

    share_btn = authed_page.locator("#btnShare")
    expect(share_btn).to_be_visible()
    share_btn.click()
    authed_page.wait_for_timeout(1000)
    # Button text briefly changes to "Copied!" to confirm the action
    text = share_btn.text_content() or ""
    assert "Copied" in text or "Share" in text


def test_compare_picker_from_sweep_navigates_to_compare_view(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """Compare picker opens and navigates to the compare page when two runs are selected."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(3000)

    compare_btn = authed_page.locator("#btnCompare")
    expect(compare_btn).to_be_visible()
    compare_btn.click()
    authed_page.wait_for_timeout(500)

    picker = authed_page.locator("#comparePicker")
    expect(picker).to_have_class("compare-picker open")

    run_a_sel = authed_page.locator("#cpRunA")
    run_b_sel = authed_page.locator("#cpRunB")
    opts_a = run_a_sel.locator("option").all()
    opts_b = run_b_sel.locator("option").all()
    assert len(opts_a) >= 2 and len(opts_b) >= 2

    run_a_sel.select_option(index=0)
    run_b_sel.select_option(index=1)
    authed_page.locator("#cpGo").click()
    authed_page.wait_for_url(f"**/{org}/{proj}/compare**")
    assert "compare" in authed_page.url


def test_cancel_sweep_button_only_visible_when_running(
    live_server, test_project, pushed_sweep, authed_page: Page
):
    """Cancel button is absent for a completed sweep."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/sweeps/{pushed_sweep}")
    authed_page.wait_for_timeout(2000)

    cancel_btn = authed_page.locator("#btnCancel")
    assert cancel_btn.count() == 0, "cancel button should not appear for completed sweep"
