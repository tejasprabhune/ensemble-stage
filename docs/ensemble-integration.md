# Integrating Stage with ensemble

This document is the handoff for the engineer adding Stage support to the ensemble runtime. The `ensemble-stage` package lives in `integration/` in this repository. It is pip-installable and provides the `Stage` class and the `ensemble-stage` CLI.

The ensemble-side hook is not in this repository. This document specifies exactly where it needs to go and what it needs to do.

## What the package provides

```python
from ensemble_stage import Stage

stage = Stage()  # reads STAGE_API_KEY and STAGE_BASE_URL from env

with stage.run(
    project="myorg/popcornbench",
    scenario="popcorn.single_problem",
    world="popcorn",
    backend="claude-sonnet-4-5",
    metadata={"seed": 42},
) as run:
    # run.id and run.url are set after entering the context.
    run.append_event(
        sequence_number=1,
        kind="system",
        payload={"note": "run started", "actor": "system"},
        wall_time_ms=0,
    )
    # ... more append_event calls ...
# On exit, status is set to "completed" or "failed" depending on whether
# an exception propagated out of the block.
```

Events are buffered in memory and flushed every second or when the buffer reaches 100 events. If a flush fails after three retries with exponential backoff, the events are moved to a pending list for later replay via `ensemble-stage push`.

The `ensemble-stage` CLI handles the retry case:

```
ensemble-stage push trace.jsonl \
  --project myorg/popcornbench \
  --scenario popcorn.single_problem \
  --world popcorn \
  --backend claude-sonnet-4-5
```

## Where the ensemble-side hook needs to go

The hook is a streaming sink in ensemble's runner. The right place is the `Runner` class (or equivalent) in `ensemble/runner.py`, which orchestrates a single scenario execution. The hook needs to:

1. Read `STAGE_API_KEY` and `STAGE_BASE_URL` from the environment (or a `stage.toml` config file in the project root; see the config section below).
2. If Stage is configured, call `stage.run(...)` at scenario start and hold the context open for the duration of the run.
3. For every event the runner emits to the local trace writer, also call `run.append_event(...)` with the same event data.
4. On scenario completion, allow the `with` block to exit naturally; the context manager finalizes the run status.

The hook should be additive: if `STAGE_API_KEY` is not set, ensemble runs exactly as before. No Stage dependency is imported unless Stage is configured.

### Event mapping

ensemble's internal event format should map to Stage's event format as follows:

| ensemble event | Stage `kind` | Payload notes |
|---------------|-------------|---------------|
| Actor message (agent) | `agent_message` | `{"actor": actor_name, "kind": "agent_message", "text": message_text}` |
| Actor message (user) | `user_message` | `{"actor": "user", "kind": "user_message", "text": message_text}` |
| Tool call | `tool_call` | `{"actor": actor_name, "kind": "tool_call", "name": tool_name, "args": args_dict}` |
| Tool result | `tool_result` | `{"actor": actor_name, "kind": "tool_result", "name": tool_name, "result": result}` |
| State change | `state_diff` | `{"actor": actor_name, "kind": "state_diff", "diff": {"table": ..., "field": ..., "old": ..., "new": ...}}` |
| Cost record | `cost` | `{"actor": actor_name, "kind": "cost", "unit": "usd", "amount": ..., "running_total": ...}` |
| Progress report | `progress` | `{"actor": actor_name, "kind": "progress", "tool": tool_name, "fraction": 0.0..1.0, "message": "..."}` |
| Run lifecycle | `system` | `{"note": "run started\|completed\|failed", "actor": "system"}` |
| Grader output | `system` | `{"note": "grader: " + json.dumps(grader_payload), "actor": "system"}` |

The `sequence_number` is the 1-indexed position of the event in the trace. It must be assigned by the ensemble-side hook, since Stage does not auto-increment it. Gaps are allowed but the sequence must be monotonically increasing.

The `event_id` in each `append_event` call is a freshly generated UUID4. The `RunContext.append_event` method generates one automatically; you do not need to pass it explicitly unless you are replaying from a trace file.

### Sequence number tracking

The simplest correct implementation: keep a counter that increments with each event. Reset it to 1 for each new run.

```python
seq = 0

def emit(kind, payload, wall_time_ms=None):
    nonlocal seq
    seq += 1
    run.append_event(
        sequence_number=seq,
        kind=kind,
        payload=payload,
        wall_time_ms=wall_time_ms,
    )
```

### Configuration via stage.toml

Stage can be configured per-project with a `stage.toml` in the ensemble project root (the directory containing `ensemble.toml` or `pyproject.toml`). The schema:

```toml
[stage]
api_key = ""        # overridden by STAGE_API_KEY env var
base_url = "https://stage.ensemble.sh"
project = "myorg/popcornbench"  # default project slug
enabled = true
```

The ensemble-side hook should look for this file with `tomllib.load` (stdlib in Python 3.11+) or `tomli` for earlier versions. Environment variables take priority over file values.

### Fallback behavior

If Stage is unreachable or returns errors during a run, the hook must not interrupt the run. The `RunContext` handles retries internally and moves failed events to a pending list. The local trace file is always written regardless of Stage's reachability.

If the initial `stage.run(...)` call fails (e.g., the project does not exist on the Stage server), log a warning and continue the run without Stage. Do not raise.

## Installing the package

The package is not yet published to PyPI. Install directly from the repository:

```
uv add git+https://github.com/your-org/ensemble-stage.git
```

Or in development, with an editable install from a local checkout:

```
uv add --editable /path/to/ensemble-stage/integration
```

## Running the tests

```
cd integration
uv sync --dev
uv run pytest
```

## Minimal end-to-end test

Once the ensemble-side hook is in place, verify the full path:

```bash
export STAGE_API_KEY=stage_sk_...
export STAGE_BASE_URL=http://localhost:3000

# Run a scenario with Stage enabled.
STAGE_API_KEY=$STAGE_API_KEY ensemble run popcorn.single_problem

# Check that the run appeared.
ensemble-stage status <run_id>
```

Open the Stage web UI and navigate to the run. Events should appear in the trace viewer in real time during the run.
