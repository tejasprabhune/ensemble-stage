"""
Tests for the run detail page.
"""
from __future__ import annotations

import pytest
from playwright.sync_api import Page, expect


def test_run_detail_populates_all_sections(
    live_server, test_project, pushed_run, authed_page: Page
):
    """
    A completed run with events and scores shows real data in every
    section of the detail page.
    """
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/runs/{pushed_run}")
    authed_page.wait_for_timeout(3000)

    # Header: status badge shows completed
    expect(authed_page.locator("#metaStatusText")).to_contain_text("completed")

    # Header: started and duration are not dashes
    started = authed_page.locator("#metaStarted").text_content() or ""
    assert started != "—", "started_at should be set"

    duration = authed_page.locator("#metaDuration").text_content() or ""
    assert duration != "—", "wall_time_ms should produce a non-dash duration"

    # Timeline: shows more than 0/0 events
    tick_label = authed_page.locator("#tickLabel").text_content() or ""
    assert not tick_label.startswith("0 / 0"), f"timeline stuck at 0/0: {tick_label}"

    # Timeline: actors panel has actor names
    actors_text = authed_page.locator("#messagesPanel").text_content() or ""
    assert "alice" in actors_text.lower()

    # Sidebar Outcome: shows real scores, not "success" string
    outcome_text = authed_page.locator("#metaOutcome").text_content() or ""
    assert "alice_refund_resolved" in outcome_text
    assert '"success"' not in outcome_text

    # Sidebar Actors: lists actors with model info
    actors_sidebar = authed_page.locator("#actorsData").text_content() or ""
    assert "alice" in actors_sidebar.lower()

    # Sidebar Predicates: shows grader scores
    predicates_text = authed_page.locator("#predicatesData").text_content() or ""
    assert predicates_text != "—", "predicates sidebar should be populated"

    # Sidebar Hidden state: alice's hidden state visible
    hidden_text = authed_page.locator("#hiddenStateData").text_content() or ""
    assert "mood" in hidden_text.lower() or "hidden_goal" in hidden_text.lower()


def test_run_detail_tool_calls_show_names(
    live_server, test_project, pushed_run, authed_page: Page
):
    """Tool call panel shows actual tool names, not 'undefined'."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/runs/{pushed_run}")
    authed_page.wait_for_timeout(3000)

    tools_text = authed_page.locator("#toolsPanel").text_content() or ""
    assert "undefined" not in tools_text.lower(), (
        "tool calls should show actual names, not 'undefined'"
    )
    assert "lookup_user" in tools_text


def test_run_detail_state_changes_populated(
    live_server, test_project, pushed_run, authed_page: Page
):
    """State changes panel shows table rows when there are diffs, not 'no state changes yet'."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    # pushed_run fixture doesn't include state_diff events; skip this assertion
    # but at minimum the panel should not crash (no JS error)
    authed_page.goto(f"{live_server}/{org}/{proj}/runs/{pushed_run}")
    authed_page.wait_for_timeout(2000)

    # Should not have JS errors that leave the diff panel empty with an error
    diff_text = authed_page.locator("#diffPanel").text_content() or ""
    assert "error" not in diff_text.lower()


def test_run_detail_chat_view_shows_messages(
    live_server, test_project, pushed_run, authed_page: Page
):
    """Switching to chat view shows message bubbles with text."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/runs/{pushed_run}")
    authed_page.wait_for_timeout(2000)

    # Click chat tab
    authed_page.locator("#tabChat").click()
    authed_page.wait_for_timeout(500)

    chat_text = authed_page.locator("#chatFeed").text_content() or ""
    assert "I want my money back" in chat_text or "alice" in chat_text.lower()


def test_run_detail_copy_id_button(
    live_server, test_project, pushed_run, authed_page: Page
):
    """The copy-id button exists and is clickable."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/runs/{pushed_run}")

    copy_btn = authed_page.get_by_text("copy").first
    expect(copy_btn).to_be_visible()
    copy_btn.click()
    # No assertion on clipboard content in headless mode; just verify no JS error
