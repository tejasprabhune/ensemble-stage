# Integrating Stage with ensemble

The `ensemble_stage` Python package lives in `integration/` in this repository. It provides a `Stage` client and a `RunContext` context manager that handle run creation, event streaming, and status finalization. The ensemble runner calls these during a scenario execution; Stage records the trace and makes it visible in the web UI.

## Installing the package

The package is not yet published to PyPI. Install it directly from the repository:

```bash
uv add git+https://github.com/your-org/ensemble-stage.git#subdirectory=integration
```

For development, an editable install from a local checkout works the same way:

```bash
uv add --editable /path/to/ensemble-stage/integration
```

## Configuration

The client reads two environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `ENSEMBLE_STAGE_API_KEY` | (required) | A push-scoped API key from `/me`. |
| `ENSEMBLE_STAGE_BASE_URL` | `https://stage.ensemble.sh` | The Stage server URL. |

You can also pass them as constructor arguments, which take priority over the environment:

```python
stage = Stage(api_key="stage_sk_...", base_url="http://localhost:3000")
```

To configure per-project with a `stage.toml` file in the ensemble project root:

```toml
[stage]
base_url = "https://stage.ensemble.sh"
project  = "myorg/popcornbench"
enabled  = true
```

The `api_key` field in `stage.toml` is supported but storing credentials in files is not recommended. `ENSEMBLE_STAGE_API_KEY` in the environment is the right place.

## Pushing a run

The main entry point is `Stage.run()`, which returns a context manager. The run is created on the server when the `with` block is entered and finalized when it exits. If the block exits normally the status is set to "completed"; if an exception propagates out, the status is set to "failed".

```python
from ensemble_stage import Stage

stage = Stage()

with stage.run(
    project="myorg/popcornbench",
    scenario="popcorn.single_problem",
    world="popcorn",
    backend="claude-sonnet-4-5",
    metadata={"seed": 42, "git_sha": "a1b2c3d"},
) as run:
    print(f"Run started: {run.url}")

    run.append_event(
        sequence_number=1,
        kind="system",
        payload={"note": "run started", "actor": "system"},
        wall_time_ms=0,
    )
    # ... more events as the scenario runs ...
```

`run.id` and `run.url` are set after the context is entered. `run.url` points directly to the trace viewer page for this run.

Events are buffered in memory and flushed to the server every second, or immediately when the buffer reaches 100 events. This keeps network overhead low for fast scenarios while keeping the trace viewer reasonably up to date for slow ones.

If a flush fails, the client retries up to three times with exponential backoff. Events that still cannot be delivered after three attempts go into a pending list (`run._pending`) for later replay. In normal operation you will never need to interact with this directly.

## Event kinds

Stage understands eight event kinds. Other kind strings are accepted and stored as-is, but the trace viewer only renders the eight it knows about.

**`system`** records lifecycle notes and grader output. Use it at the start and end of a run and for structured grader results:

```python
# Run lifecycle
run.append_event(1, "system", {"note": "run started", "actor": "system"})

# Grader output at run end
import json
run.append_event(n, "system", {
    "note": "grader: " + json.dumps({"scores": {"correctness": 0.92}}),
    "actor": "system",
})
```

**`user_message`** and **`agent_message`** record the dialogue between human and agent turns:

```python
run.append_event(2, "user_message", {
    "actor": "user",
    "kind": "user_message",
    "text": "Solve this problem.",
})
run.append_event(3, "agent_message", {
    "actor": "agent:0",
    "kind": "agent_message",
    "text": "I will begin by...",
})
```

**`tool_call`** and **`tool_result`** capture each tool invocation and its outcome:

```python
run.append_event(4, "tool_call", {
    "actor": "agent:0",
    "kind": "tool_call",
    "name": "bash",
    "args": {"command": "ls /tmp"},
    "seed": False,
})
run.append_event(5, "tool_result", {
    "actor": "agent:0",
    "kind": "tool_result",
    "name": "bash",
    "result": {"summary": "file1.txt\nfile2.txt"},
})
```

**`state_diff`** records a change to world state:

```python
run.append_event(6, "state_diff", {
    "actor": "agent:0",
    "kind": "state_diff",
    "diff": {"table": "inventory", "field": "count", "old": 0, "new": 10},
})
```

**`cost`** accumulates token spend. Call it after each model invocation:

```python
run.append_event(7, "cost", {
    "actor": "agent:0",
    "kind": "cost",
    "unit": "usd",
    "amount": 0.0012,
    "running_total": 0.0183,
})
```

**`progress`** reports completion of a long-running tool call:

```python
run.append_event(8, "progress", {
    "actor": "agent:0",
    "kind": "progress",
    "tool": "compile",
    "fraction": 0.45,
    "message": "compiling module 9 of 20",
})
```

## Sequence numbers

Sequence numbers are 1-indexed integers you assign. They must be unique within a run; gaps are allowed. The simplest correct implementation is a counter you increment for each event:

```python
seq = 0

def emit(kind, payload, wall_time_ms=None):
    global seq
    seq += 1
    run.append_event(sequence_number=seq, kind=kind, payload=payload, wall_time_ms=wall_time_ms)
```

The server uses sequence numbers to order events in the trace viewer. Out-of-order delivery is fine because the viewer sorts by sequence number, not by arrival time.

## Finalizing with outcome and cost

The `RunContext` sends a bare `status=completed` (or `status=failed`) on context exit. To include outcome scores and total cost in the run's permanent record, post the final status explicitly before exiting the context:

```python
import requests

with stage.run(...) as run:
    # ... run the scenario ...

    # Post the final status with outcome before the context exits.
    run._session.post(
        f"{run._base_url}/v1/runs/{run.id}/status",
        json={
            "status": "completed",
            "outcome": {"scores": {"correctness": 0.92, "efficiency": 0.78}},
            "total_cost": {"input_tokens": 12400, "output_tokens": 3200, "usd": 0.0183},
            "wall_time_ms": 47000,
        },
    )
    # The context exit will post status=completed again; the server accepts
    # the second update but outcome and cost are already set.
```

A cleaner pattern, once the ensemble runner integrates Stage directly, is for the runner to call the status endpoint after collecting the grader result, then let the context exit naturally.

## Sweeps

A sweep groups multiple runs that share a configuration grid. Create one before starting the child runs, then register each run as a child as it starts.

```python
sweep = stage.sweep(
    project="myorg/popcornbench",
    config={
        "scenarios": ["popcorn.single_problem"],
        "backends": ["claude-sonnet-4-5", "claude-opus-4-7"],
        "n_trials": 5,
    },
)
sweep_id = sweep["id"]
print(f"Sweep: {sweep['url']}")

for backend in ["claude-sonnet-4-5", "claude-opus-4-7"]:
    for trial in range(5):
        with stage.run(
            project="myorg/popcornbench",
            scenario="popcorn.single_problem",
            world="popcorn",
            backend=backend,
            sweep_id=sweep_id,
            metadata={"trial": trial},
        ) as run:
            # ... run the scenario ...
            pass
```

Passing `sweep_id` to `stage.run()` sets the FK on the run record directly. The sweep page on Stage will show all runs in the sweep once that view is built.

## Training runs

```python
tr = stage.training_run(
    project="myorg/popcornbench",
    persona_name="popcorn-v2",
    base_model="claude-haiku-4-5",
    hyperparameters={"learning_rate": 1e-4, "batch_size": 32, "max_steps": 10000},
)
training_run_id = tr["id"]

# Report metrics periodically during training.
stage._session.post(
    f"{stage.base_url}/v1/training_runs/{training_run_id}/metrics",
    json={"metrics": [
        {"step": 100, "metric_name": "train_loss", "value": 1.42},
        {"step": 100, "metric_name": "eval_loss",  "value": 1.57},
    ]},
)

# Finalize when training is done.
stage._session.post(
    f"{stage.base_url}/v1/training_runs/{training_run_id}/status",
    json={
        "status": "completed",
        "final_metrics": {"train_loss": 0.32, "eval_loss": 0.41},
        "artifact_uri": "gs://my-bucket/adapters/popcorn-v2.safetensors",
    },
)
```

## Error handling

The client should not interrupt ensemble runs if Stage is unavailable. The `RunContext` handles retries internally for event pushes. For the initial run creation, wrap the `with stage.run(...)` block and fall back to running without Stage:

```python
try:
    ctx = stage.run(project=..., scenario=..., world=..., backend=...)
except Exception as e:
    logging.warning(f"Stage unavailable, running without tracing: {e}")
    ctx = nullcontext()  # or a no-op RunContext

with ctx as run:
    # run is None if Stage is unavailable; check before calling run.append_event
    ...
```

If `ENSEMBLE_STAGE_API_KEY` is not set, `stage.api_key` is an empty string and the first request will get a 401. Treat a missing key the same as an unavailable server: log a warning and proceed.

## Where the ensemble hook goes

The hook belongs in the `Runner` class in `ensemble/runner.py`, which orchestrates a single scenario execution. The hook is additive: if `ENSEMBLE_STAGE_API_KEY` is not set, import of `ensemble_stage` is skipped and ensemble behaves exactly as before.

The ensemble event stream maps to Stage event kinds as follows:

| Ensemble event | Stage `kind` | Notes |
|---------------|-------------|-------|
| Actor message (agent) | `agent_message` | `actor` is the agent name |
| Actor message (user) | `user_message` | `actor` is `"user"` |
| Tool call | `tool_call` | Include `name`, `args`, `seed` |
| Tool result | `tool_result` | Include `name`, `result` |
| State change | `state_diff` | Include `table`, `field`, `old`, `new` |
| Cost record | `cost` | Include `unit`, `amount`, `running_total` |
| Progress report | `progress` | Include `tool`, `fraction`, `message` |
| Run lifecycle | `system` | `note` is "run started", "completed", or "failed" |
| Grader output | `system` | `note` is `"grader: " + json.dumps(grader_payload)` |

The sequence number counter resets to 1 for each new run and increments for every event.
