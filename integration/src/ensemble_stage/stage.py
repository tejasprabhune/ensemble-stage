from __future__ import annotations

import os
import uuid
from typing import Any

import requests

from .run import RunContext


class Stage:
    """Client for a Stage server.

    Configuration is read from environment variables:

      ENSEMBLE_STAGE_API_KEY   required for push operations
      ENSEMBLE_STAGE_BASE_URL  defaults to https://stage.ensemble.sh

    Both can also be passed as constructor arguments, which take priority.
    """

    def __init__(
        self,
        api_key: str | None = None,
        base_url: str | None = None,
    ) -> None:
        self.api_key = api_key or os.environ.get("ENSEMBLE_STAGE_API_KEY", "")
        self.base_url = (
            base_url
            or os.environ.get("ENSEMBLE_STAGE_BASE_URL", "https://stage.ensemble.sh")
        ).rstrip("/")

        self._session = requests.Session()
        self._session.headers.update(
            {
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
                "User-Agent": "ensemble-stage/0.1.0",
            }
        )

    def run(
        self,
        project: str,
        scenario: str,
        world: str,
        backend: str,
        sweep_id: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> RunContext:
        """Return a context manager that represents one run.

        The run is created on the Stage server when the context is entered
        and finalized (completed or failed) when the context exits.

        ``project`` is ``org_slug/project_slug``.
        """
        if "/" not in project:
            raise ValueError("project must be 'org_slug/project_slug'")
        org_slug, project_slug = project.split("/", 1)

        return RunContext(
            session=self._session,
            base_url=self.base_url,
            org_slug=org_slug,
            project_slug=project_slug,
            scenario=scenario,
            world=world,
            backend=backend,
            sweep_id=sweep_id,
            metadata=metadata or {},
        )

    def sweep(
        self,
        project: str,
        config: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Create a sweep. Returns the server response with ``id`` and ``url``."""
        if "/" not in project:
            raise ValueError("project must be 'org_slug/project_slug'")
        org_slug, project_slug = project.split("/", 1)
        resp = self._session.post(
            f"{self.base_url}/v1/projects/{org_slug}/{project_slug}/sweeps",
            json={"config": config or {}},
        )
        resp.raise_for_status()
        return resp.json()

    def training_run(
        self,
        project: str,
        persona_name: str,
        base_model: str,
        hyperparameters: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Create a training run. Returns ``id`` and ``url``."""
        if "/" not in project:
            raise ValueError("project must be 'org_slug/project_slug'")
        org_slug, project_slug = project.split("/", 1)
        resp = self._session.post(
            f"{self.base_url}/v1/projects/{org_slug}/{project_slug}/training_runs",
            json={
                "persona_name": persona_name,
                "base_model": base_model,
                "hyperparameters": hyperparameters or {},
            },
        )
        resp.raise_for_status()
        return resp.json()
