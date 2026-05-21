"""ensemble-stage command-line tool.

Commands:

  ensemble-stage push <trace.jsonl> --project myorg/bench --scenario ... \\
      --world ... --backend ...

      Read a local JSONL trace file and push it as a new run. Useful for
      retrying failed pushes or uploading traces from offline runs.

  ensemble-stage status <run-id>

      Print the current status and outcome of a run.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import click

from .stage import Stage


@click.group()
def main() -> None:
    """ensemble-stage: push and inspect Stage runs."""
    pass


@main.command()
@click.argument("trace", type=click.Path(exists=True, dir_okay=False, path_type=Path))
@click.option("--project", required=True, help="org_slug/project_slug")
@click.option("--scenario", required=True)
@click.option("--world", required=True)
@click.option("--backend", required=True)
@click.option("--sweep-id", default=None)
def push(
    trace: Path,
    project: str,
    scenario: str,
    world: str,
    backend: str,
    sweep_id: str | None,
) -> None:
    """Push a local JSONL trace file to Stage as a new run."""
    stage = Stage()
    events = []
    with trace.open() as f:
        for i, line in enumerate(f):
            line = line.strip()
            if not line:
                continue
            try:
                payload = json.loads(line)
            except json.JSONDecodeError as err:
                click.echo(f"warning: line {i+1} is not valid JSON: {err}", err=True)
                continue
            events.append(
                {
                    "sequence_number": i + 1,
                    "kind": payload.get("kind", "raw"),
                    "payload": payload,
                    "event_id": payload.get("event_id") or _new_uuid(),
                    "wall_time_ms": payload.get("wall_time_ms"),
                }
            )

    click.echo(f"pushing {len(events)} events to {stage.base_url} ...")

    with stage.run(
        project=project,
        scenario=scenario,
        world=world,
        backend=backend,
        sweep_id=sweep_id,
    ) as run:
        click.echo(f"run created: {run.url}")
        _BATCH = 200
        for i in range(0, len(events), _BATCH):
            batch = events[i : i + _BATCH]
            import requests as _req

            resp = run._session.post(
                f"{run._base_url}/v1/runs/{run.id}/events",
                json={"events": batch},
            )
            if not resp.ok:
                click.echo(f"error pushing batch: {resp.status_code} {resp.text}", err=True)
                sys.exit(1)

    click.echo("done.")


@main.command()
@click.argument("run_id")
def status(run_id: str) -> None:
    """Print the current status of a run."""
    import os
    import requests as _req

    base = os.environ.get("ENSEMBLE_STAGE_BASE_URL", "https://stage.ensemble.sh").rstrip("/")
    api_key = os.environ.get("ENSEMBLE_STAGE_API_KEY", "")
    resp = _req.get(
        f"{base}/v1/runs/{run_id}",
        headers={"Authorization": f"Bearer {api_key}"},
    )
    if not resp.ok:
        click.echo(f"error: {resp.status_code} {resp.text}", err=True)
        sys.exit(1)
    data = resp.json()
    click.echo(f"status:  {data.get('status')}")
    click.echo(f"url:     {base}/runs/{run_id}")
    if data.get("outcome"):
        click.echo(f"outcome: {json.dumps(data['outcome'], indent=2)}")


def _new_uuid() -> str:
    import uuid
    return str(uuid.uuid4())
