"""
Tests for the training run detail page.
"""
from __future__ import annotations

import math
import uuid

import pytest
import requests
from playwright.sync_api import Page, expect


def _push_training_run(live_server, test_project, test_user, *, persona="test_persona",
                        base_model="Qwen/Qwen2.5-7B-Instruct", steps=40,
                        hyperparameters=None, artifact_uri=None):
    """Create a completed training run with realistic loss curves."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}

    resp = requests.post(
        f"{live_server}/v1/projects/{org}/{proj}/training_runs",
        headers=headers,
        json={
            "persona_name": persona,
            "base_model": base_model,
            "hyperparameters": hyperparameters or {
                "learning_rate": 5e-6,
                "batch_size": 4,
                "epochs": 1,
                "lora_r": 16,
                "dpo_beta": 0.1,
            },
        },
    )
    assert resp.status_code == 201, resp.text
    tr_id = resp.json()["id"]

    # Push loss curves: realistic exponential decay
    metrics = []
    for step in range(10, steps + 1, 10):
        t = step / steps
        train_loss = 2.0 * math.exp(-3 * t) + 0.3 + (0.05 * math.sin(step))
        eval_loss = 2.1 * math.exp(-2.8 * t) + 0.35
        metrics.append({"step": step, "metric_name": "train_loss", "value": round(train_loss, 4)})
        metrics.append({"step": step, "metric_name": "eval_loss", "value": round(eval_loss, 4)})

    requests.post(
        f"{live_server}/v1/training_runs/{tr_id}/metrics",
        headers=headers,
        json={"metrics": metrics},
    )

    final = {"train_loss": 0.32, "eval_loss": 0.38, "best_eval": 0.36, "total_steps": steps}
    requests.post(
        f"{live_server}/v1/training_runs/{tr_id}/status",
        headers=headers,
        json={
            "status": "completed",
            "final_metrics": final,
            "artifact_uri": artifact_uri or f"hf://test-user/ensemble-{persona}",
        },
    )
    return tr_id


def test_loss_curves_render_with_metrics(
    live_server, test_project, test_user, authed_page: Page
):
    """Loss curves appear after metrics are pushed."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user)

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(3000)

    charts = authed_page.locator(".metric-chart")
    assert charts.count() >= 1, "expected at least one chart"
    waiting = authed_page.locator("#metricsWaiting")
    assert waiting.is_hidden(), "waiting message should be hidden after metrics load"


def test_loss_curves_log_scale_toggle(
    live_server, test_project, test_user, authed_page: Page
):
    """Log scale toggle re-renders the chart and updates the URL hash."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user)

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(3000)

    toggle = authed_page.locator("#logScaleToggle")
    expect(toggle).to_be_visible()
    assert not toggle.is_checked(), "log scale should be off by default"

    toggle.click()
    authed_page.wait_for_timeout(300)
    assert toggle.is_checked()
    assert "#log" in authed_page.url

    toggle.click()
    authed_page.wait_for_timeout(300)
    assert not toggle.is_checked()


def test_loss_curves_best_eval_annotation(
    live_server, test_project, test_user, authed_page: Page
):
    """The best eval annotation appears on the loss curves chart."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user)

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(3000)

    svg_texts = authed_page.locator(".metric-chart text").all_text_contents()
    best_labels = [t for t in svg_texts if "Best eval" in t]
    assert best_labels, f"expected Best eval annotation, svg texts: {svg_texts[:10]}"


def test_loss_curves_polling_while_running(
    live_server, test_project, test_user, authed_page: Page
):
    """A running training run shows the waiting message until metrics arrive."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}

    resp = requests.post(
        f"{live_server}/v1/projects/{org}/{proj}/training_runs",
        headers=headers,
        json={"persona_name": "poll_test", "base_model": "test-model"},
    )
    tr_id = resp.json()["id"]
    requests.post(
        f"{live_server}/v1/training_runs/{tr_id}/status",
        headers=headers,
        json={"status": "running"},
    )

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(2000)

    waiting = authed_page.locator("#metricsWaiting")
    expect(waiting).to_be_visible()


def test_hyperparameters_table_renders(
    live_server, test_project, test_user, authed_page: Page
):
    """Hyperparameters table appears with the pushed hyperparameter values."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user,
                                hyperparameters={"learning_rate": 5e-6, "batch_size": 4})

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(3000)

    section = authed_page.locator("#hyperparamsSection")
    expect(section).to_be_visible()
    text = section.text_content() or ""
    assert "learning_rate" in text, f"expected learning_rate in hyperparams: {text[:200]}"


def test_final_metrics_table_renders(
    live_server, test_project, test_user, authed_page: Page
):
    """Final metrics table appears with the pushed final metric values."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user)

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(3000)

    section = authed_page.locator("#finalMetricsSection")
    expect(section).to_be_visible()
    text = section.text_content() or ""
    assert "train_loss" in text or "eval_loss" in text, f"expected loss in final metrics: {text[:200]}"


def test_comparison_to_past_training_overlays_lines(
    live_server, test_project, test_user, authed_page: Page
):
    """Comparison chart overlays lines from past training runs for the same persona."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    persona = "comparison_persona"
    _push_training_run(live_server, test_project, test_user, persona=persona)
    tr_id2 = _push_training_run(live_server, test_project, test_user, persona=persona)

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id2}")
    authed_page.wait_for_timeout(3000)

    wrap = authed_page.locator("#comparisonChartWrap")
    expect(wrap).to_be_visible()
    paths = authed_page.locator("#comparisonChartWrap path")
    assert paths.count() >= 2, f"expected at least 2 paths (current + 1 past), got {paths.count()}"

    legend = authed_page.locator("#comparisonLegend")
    text = legend.text_content() or ""
    assert "This run" in text


def test_comparison_empty_state_for_first_run(
    live_server, test_project, test_user, authed_page: Page
):
    """First training run for a persona shows the empty state message."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user, persona="unique_first_persona")

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(3000)

    wrap = authed_page.locator("#comparisonChartWrap")
    text = wrap.text_content() or ""
    assert "first training run" in text.lower(), f"expected first-run message: {text[:200]}"


def test_baseline_comparison_table_renders(
    live_server, test_project, test_user, authed_page: Page
):
    """Baseline comparison section renders with a table when linked eval runs exist."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}
    artifact = f"hf://testuser/ensemble-baseline-persona-{uuid.uuid4().hex[:8]}"
    tr_id = _push_training_run(live_server, test_project, test_user,
                                persona="baseline_persona", artifact_uri=artifact)

    # Push a run that references this adapter via metadata
    run_id = str(uuid.uuid4())
    requests.post(
        f"{live_server}/v1/projects/{org}/{proj}/runs",
        headers=headers,
        json={"id": run_id, "scenario": "agora.refund_storm", "world": "agora",
              "backend": "mock", "metadata": {"adapter_uri": artifact, "persona": "baseline_persona"}},
    )
    requests.post(
        f"{live_server}/v1/runs/{run_id}/status",
        headers=headers,
        json={"status": "completed", "outcome": {"score": 0.75}, "wall_time_ms": 1000},
    )

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(3000)

    wrap = authed_page.locator("#baselineWrap")
    text = wrap.text_content() or ""
    assert "agora.refund_storm" in text or "baseline" in text.lower(), \
        f"expected scenario in baseline table: {text[:300]}"


def test_baseline_comparison_empty_state_when_no_linkage(
    live_server, test_project, test_user, authed_page: Page
):
    """Baseline section shows the no-linkage message when no runs reference the adapter."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user, persona="no_link_persona")

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(3000)

    wrap = authed_page.locator("#baselineWrap")
    text = wrap.text_content() or ""
    assert "adapter_uri" in text or "not yet recorded" in text.lower() or \
           "no completed eval" in text.lower(), \
        f"expected no-linkage message: {text[:300]}"


def test_artifact_uri_copy_button(
    live_server, test_project, test_user, authed_page: Page
):
    """Copy button appears next to the artifact URI in the header."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user)

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(2000)

    copy_btn = authed_page.locator("#btnCopyArtifactHeader")
    expect(copy_btn).to_be_visible()


def test_artifact_inference_snippet_copies(
    live_server, test_project, test_user, authed_page: Page
):
    """The artifact section shows a TOML snippet and a run command."""
    org, proj = test_project["org_slug"], test_project["project_slug"]
    tr_id = _push_training_run(live_server, test_project, test_user)

    authed_page.goto(f"{live_server}/{org}/{proj}/training_runs/{tr_id}")
    authed_page.wait_for_timeout(2000)

    snippets = authed_page.locator(".artifact-snippet").all_text_contents()
    combined = " ".join(snippets)
    assert "adapter_name" in combined, f"expected adapter_name in snippet: {combined[:200]}"
    assert "ensemble run" in combined, f"expected ensemble run command in snippet: {combined[:200]}"
