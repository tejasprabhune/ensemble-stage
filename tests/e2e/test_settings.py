"""
Tests for the project settings page.
"""
from __future__ import annotations

import pytest
from playwright.sync_api import Page, expect


def test_settings_page_renders(
    live_server, test_project, authed_page: Page
):
    """Settings page loads and shows project fields."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/settings")

    expect(authed_page.locator("input[name=name]")).to_be_visible()
    expect(authed_page.locator("input[name=public]")).to_be_visible()
    expect(authed_page.get_by_text("Delete project")).to_be_visible()


def test_settings_edit_name(
    live_server, test_project, authed_page: Page
):
    """Submitting the settings form updates the project name."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/settings")
    authed_page.wait_for_load_state("networkidle")

    name_input = authed_page.locator("input[name=name]")
    expect(name_input).to_be_visible()
    name_input.click()
    name_input.fill("Updated Project Name")
    authed_page.get_by_role("button", name="Save changes").click()

    # Should redirect back to settings
    authed_page.wait_for_url(f"**/{proj}/settings**", timeout=5000)

    # Name input should reflect new value
    expect(authed_page.locator("input[name=name]")).to_have_value("Updated Project Name")


def test_settings_delete_project_requires_slug(
    live_server, test_project, authed_page: Page
):
    """Delete form with wrong slug does not delete the project."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/settings")
    authed_page.wait_for_load_state("networkidle")

    # Expand the delete section by clicking the summary
    authed_page.locator("details.delete-confirm-details > summary").click()
    authed_page.wait_for_timeout(400)

    confirm_input = authed_page.locator("input[name=confirm_slug]")
    expect(confirm_input).to_be_visible()
    confirm_input.click()
    confirm_input.fill("wrong-slug")
    authed_page.locator("button.btn-danger").click()

    authed_page.wait_for_timeout(500)
    assert proj in authed_page.url or "settings" in authed_page.url


def test_settings_delete_project_with_correct_slug(
    live_server, test_project, authed_page: Page
):
    """Delete form with correct slug deletes the project and redirects to org."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}/settings")
    authed_page.wait_for_load_state("networkidle")

    authed_page.locator("details.delete-confirm-details > summary").click()
    authed_page.wait_for_timeout(400)

    confirm_input = authed_page.locator("input[name=confirm_slug]")
    expect(confirm_input).to_be_visible()
    confirm_input.click()
    confirm_input.fill(proj)
    authed_page.locator("button.btn-danger").click()

    authed_page.wait_for_url(f"**/{org}**", timeout=5000)
    assert authed_page.url.rstrip("/").endswith(f"/{org}")


def test_settings_nav_link_present(
    live_server, test_project, authed_page: Page
):
    """The Settings link in the nav is a real URL, not href='#'."""
    org = test_project["org_slug"]
    proj = test_project["project_slug"]

    authed_page.goto(f"{live_server}/{org}/{proj}")
    settings_link = authed_page.locator(f"a[href='/{org}/{proj}/settings']")
    expect(settings_link).to_be_visible()
