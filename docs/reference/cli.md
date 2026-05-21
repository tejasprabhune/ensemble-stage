# ensemble CLI reference

The `ensemble` binary is the primary interface to the framework. It is a Rust binary that dispatches to Python for scenario execution, sweep orchestration, runs inspection, and Stage integration. Install it with `cargo install --path crates/ensemble-cli` from the ensemble repo root.

## Global conventions

- Commands that invoke Python use `uv run python` by default so the host project's lockfile is respected. Pass `--no-sync` or set `ENSEMBLE_NO_SYNC=1` to skip uv and use whatever `python` is active in the current environment. The latter is useful when the host pyproject.toml has a stale lockfile.
- Arguments documented as `<positional>` are required. Arguments documented as `[--flag value]` are optional.
- Run IDs and sweep IDs are UUID version 7 strings. Any command that accepts a run ID also accepts a unique prefix of the full ID.

---

## ensemble init

Scaffold a new world project skeleton.

```
ensemble init <name> [--path <PATH>] [--world <NAME>] [--with-rust]
```

| Argument | Description |
|----------|-------------|
| `<name>` | Name of the new world, used as the directory name and manifest `name` field. |
| `--path <PATH>` | Create the project at this path instead of `./<name>`. |
| `--world <NAME>` | Scaffold a scenarios-only directory bound to an already-registered world. Useful when the world lives elsewhere and you only want a new scenario package pointing at it. |
| `--with-rust` | Scaffold the heavyweight shape: a Rust state crate in addition to the Python package. Use when the world needs typed state with snapshot/restore semantics. Default is the pure-Python layout. |

The pure-Python layout (default) produces:

```
my_world/
  world.toml          World manifest
  my_world/
    __init__.py
    scenarios/
      smoke.py        Minimal smoke scenario
  pyproject.toml
```

After scaffolding, register the world and run its smoke scenario:

```bash
ensemble worlds add my_world ./my_world
ensemble run my_world.smoke
```

---

## ensemble run

Run a registered scenario and write the trace to `./traces/`.

```
ensemble run <scenario> [--world <NAME>] [--manifest <PATH>] [--package-dir <PATH>]
                        [--backend <BACKEND>] [--traces-dir <PATH>] [--no-sync]
```

| Argument | Description |
|----------|-------------|
| `<scenario>` | Scenario identifier in `world.scenario` form, e.g. `agora.refund_storm`. Must match a `@scenario`-decorated function in the world's Python package or a `[scenario.name]` block in `scenarios.toml`. |
| `--world <NAME>` | World short name to resolve from the worlds registry. Usually inferred from the scenario prefix. |
| `--manifest <PATH>` | Path to a `scenarios.toml` to load before running. When omitted, ensemble looks for `scenarios.toml` in the world package directory. |
| `--package-dir <PATH>` | Directory containing the scenario Python package. Defaults to the path recorded in the worlds registry for the resolved world. |
| `--backend <BACKEND>` | LLM backend to use: `mock`, `anthropic`, `openai`, `vllm`, or `auto`. Default `auto` picks the first backend whose API key is set. |
| `--traces-dir <PATH>` | Where to write the per-run trace directory. Default `./traces`. |
| `--no-sync` | Skip `uv run` and use the active Python interpreter directly. |

**Stage integration.** When `ENSEMBLE_STAGE_API_KEY` and `ENSEMBLE_STAGE_PROJECT` are set, the run is created on Stage before the scenario starts. The run URL is printed to stdout. Events stream to Stage in parallel with the local trace. The run is finalized with outcome scores and cost once the scenario completes.

**Trace output.** Each run writes a directory under `--traces-dir`:

```
traces/
  agora_refund_storm_2025-01-15T09-30-00/
    trace.jsonl     Full event stream, one JSON object per line
    meta.json       Run ID, scenario, world, backend, outcome, cost, duration
```

---

## ensemble trace view

Open a local trace in the browser.

```
ensemble trace view <trace> [--port <PORT>] [--site <PATH>]
```

| Argument | Description |
|----------|-------------|
| `<trace>` | Path to a `trace.jsonl` file. |
| `--port <PORT>` | Port for the embedded HTTP server. Default `8765`. |
| `--site <PATH>` | Static site directory. Defaults to `./site` relative to cwd. The trace viewer JS and HTML live here. |

Opens `http://localhost:<PORT>` in the default browser. The viewer polls the JSONL file every two seconds while the run is in progress and stops when a grader note appears.

---

## ensemble trace compare

Open a side-by-side comparison of two traces in the browser.

```
ensemble trace compare <a> <b> [--port <PORT>] [--site <PATH>]
```

The two trace panels scroll together by tick, making it easy to see where two runs diverged.

---

## ensemble models list

Print the backends ensemble knows about, whether their API keys are set, and the model identifiers each backend exposes.

```
ensemble models list
```

Output columns: backend name, key status (set or missing), model IDs. Useful for verifying the environment before running a scenario.

---

## ensemble runs list

Print recent runs as a table, merging local traces and Stage results.

```
ensemble runs list [--scenario <SCENARIO>] [--limit <N>] [--traces-dir <PATH>]
```

| Argument | Description |
|----------|-------------|
| `--scenario <SCENARIO>` | Filter to runs matching this scenario name. |
| `--limit <N>` | Maximum number of runs to print. |
| `--traces-dir <PATH>` | Local runs index to read. Default `./traces`. |

When `ENSEMBLE_STAGE_API_KEY` and `ENSEMBLE_STAGE_PROJECT` are set, Stage results are merged with local results. Runs that exist in both places are deduplicated by run ID.

---

## ensemble runs show

Print one run's metadata as JSON.

```
ensemble runs show <run> [--traces-dir <PATH>]
```

`<run>` may be a full UUID or a unique prefix. Prints the full `meta.json` content including outcome scores, cost, and wall time.

---

## ensemble runs compare

Print a side-by-side diff of two runs' outcome scores.

```
ensemble runs compare <a> <b> [--traces-dir <PATH>]
```

Useful for quick numerical comparison before opening the trace viewer.

---

## ensemble runs export

Emit the full runs index.

```
ensemble runs export [--format <json|csv>] [--traces-dir <PATH>]
```

Default format is `json`. Pipe to a file for downstream analysis:

```bash
ensemble runs export --format csv > runs.csv
```

---

## ensemble sweep run

Run a sweep defined by a `sweep.toml` configuration file. The sweep is a cartesian product over the axes defined in the config; each cell is a separate `ensemble run` invocation.

```
ensemble sweep run <config> [--no-resume]
```

| Argument | Description |
|----------|-------------|
| `<config>` | Path to a `sweep.toml` file. |
| `--no-resume` | Re-run cells whose `meta.json` already exists. Default is to skip completed cells (resume-friendly). |

**`sweep.toml` format:**

```toml
[sweep]
scenario = "agora.refund_storm"   # required
world = "agora"                   # required

[[sweep.axis]]
name = "backend"
values = ["mock", "anthropic", "openai"]

[[sweep.axis]]
name = "seed"
values = [1, 2, 3, 4, 5]

[sweep.stage]
project = "myorg/experiments"     # optional; overrides ENSEMBLE_STAGE_PROJECT
```

Each axis defines a dimension. The sweep runs the full cartesian product: in this example, 15 runs (3 backends × 5 seeds). Results land in `./traces/<sweep-name>/` with one subdirectory per cell.

When `ENSEMBLE_STAGE_API_KEY` is set, the sweep is created on Stage before the first run and each child run is registered as a cell. The sweep detail page on Stage shows a live matrix as cells complete.

---

## ensemble worlds list

List worlds in the registry at `~/.ensemble/worlds.toml`.

```
ensemble worlds list
```

---

## ensemble worlds add

Register a world by local path.

```
ensemble worlds add <name> <path> [--git <URL>]
```

`<name>` must match the `name` field in the world's `world.toml`. The `--git` URL is recorded in the registry but is not used for cloning.

After registration, any scenario with the matching world prefix resolves automatically:

```bash
ensemble worlds add agora ./examples/agora
ensemble run agora.refund_storm    # resolves to ./examples/agora
```

---

## ensemble worlds remove

Unregister a world.

```
ensemble worlds remove <name>
```

---

## ensemble worlds show

Print a world's resolved manifest details: name, python package path, rust crate path, default personas, default tools.

```
ensemble worlds show <name>
```

---

## ensemble mcp serve

Run an MCP server that exposes a world's tools to an external MCP client (such as Claude or another LLM IDE integration). The server speaks stdio MCP protocol.

```
ensemble mcp serve --world <NAME> [--scenario <SCENARIO>] [--as-agent <AGENT_ID>]
                   [--package-dir <PATH>] [--backend <BACKEND>]
```

| Argument | Description |
|----------|-------------|
| `--world <NAME>` | World to expose (required). Must be registered. |
| `--scenario <SCENARIO>` | Run this scenario in the background while the server is up. Requires `--as-agent`. |
| `--as-agent <AGENT_ID>` | Agent slot the connected MCP client takes over. The other actors in the scenario are driven by the specified `--backend`. |
| `--package-dir <PATH>` | Directory holding the scenarios package. Defaults to the world's registered path. |
| `--backend <BACKEND>` | LLM backend for the non-external actors. Default `mock`. |

**Without `--scenario`**, the server exposes tools but does not run a scenario; the client can call tools directly.

**With `--scenario` and `--as-agent`**, the server sets up the scenario, lets all non-external actors run until it is the external agent's turn, then yields tool calls to the MCP client. This lets you drive an agent in a live scenario from an IDE.

---

## ensemble train

Hand off persona fine-tuning to the Python training pipeline.

```
ensemble train <persona.toml> [--backend <modal|skypilot|local>]
```

| Argument | Description |
|----------|-------------|
| `<persona.toml>` | Path to the persona TOML file. The `[persona.training]` block defines the base model, LoRA config, DPO hyperparameters, and self-play settings. |
| `--backend` | Compute backend. `modal` (default) runs on Modal cloud. `skypilot` uses SkyPilot multi-cloud. `local` runs on the current machine (requires GPU). |

The training pipeline:
1. Generates a preference dataset by running self-play rollouts of the scenario with the `breaker_model` acting as an adversary.
2. Runs DPO (or SFT, depending on config) using the preference dataset.
3. Uploads the adapter to HuggingFace or a cloud storage URI.
4. If `ENSEMBLE_STAGE_API_KEY` and `ENSEMBLE_STAGE_PROJECT` are set, reports training metrics and the final artifact URI to Stage.

See `docs/reference/personas.md` for the full `[persona.training]` schema.

---

## ensemble stage login

Authenticate with Stage via browser OAuth.

```
ensemble stage login [--base-url <URL>]
```

Opens a browser window. After authorizing, credentials are saved to `~/.ensemble/stage.toml`. The default base URL is `https://stage.ensemble.sh`.

---

## ensemble stage logout

Remove saved Stage credentials from `~/.ensemble/stage.toml`.

```
ensemble stage logout
```

---

## ensemble stage whoami

Print the authenticated user's GitHub login and default org.

```
ensemble stage whoami
```

---

## ensemble stage projects list

List the projects visible to the authenticated user.

```
ensemble stage projects list
```

---

## ensemble stage projects create

Create a new project on Stage and write a `.stage.toml` in the current directory.

```
ensemble stage projects create <org_slug/project_slug>
```

---

## ensemble stage push

Push local traces to Stage. Skips runs already present (checks by run ID).

```
ensemble stage push <path>
```

`<path>` may be a directory (scanned recursively for `trace.jsonl` files), a single `trace.jsonl` file, or a glob pattern. Useful for retroactively pushing runs that were recorded locally before Stage integration was configured.

---

## Environment variables

| Variable | Description |
|----------|-------------|
| `ENSEMBLE_STAGE_API_KEY` | Push-scoped Stage API key. When set, runs, sweeps, and training jobs stream to Stage automatically. |
| `ENSEMBLE_STAGE_PROJECT` | Stage project in `org_slug/project_slug` form. Required alongside `ENSEMBLE_STAGE_API_KEY`. |
| `ENSEMBLE_STAGE_BASE_URL` | Stage server URL. Default `https://stage.ensemble.sh`. Set for self-hosted instances. |
| `ENSEMBLE_NO_SYNC` | Set to any non-empty value to skip `uv run` and use the active Python interpreter. Equivalent to passing `--no-sync`. |
| `ANTHROPIC_API_KEY` | Anthropic API key. Required for `--backend anthropic`. |
| `OPENAI_API_KEY` | OpenAI API key. Required for `--backend openai`. |
| `VIRTUAL_ENV` | Path to the active virtualenv. Used by `--no-sync` to find the Python binary. |
| `HF_USERNAME` | HuggingFace username. Used during training to name the uploaded adapter namespace. |
