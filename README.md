Stage is a hosted observability and tracking service for [ensemble](https://ensemble.sh). It accepts run, sweep, and training-run data pushed from ensemble installations, and renders a web UI for browsing traces, comparing runs, and monitoring training progress.

## Documentation

- [Quickstart](docs/quickstart.md) -- push your first run and see it in the trace viewer
- [ensemble integration](docs/ensemble-integration.md) -- configure the `ensemble_stage` package and map ensemble events to Stage
- [API reference](docs/api.md) -- the full HTTP API contract
- [Deployment](docs/deploy.md) -- deploy to Fly.io or self-host
- [Development](docs/development.md) -- run the server locally and execute the test suite

## Status

The push-to-view path is fully operational. A researcher with the ensemble integration configured can push a run locally, see the run URL in terminal output, visit it in a browser, and watch events appear. The end-to-end path is verified by `server/tests/e2e.rs`.

What works:

- Sign in with GitHub; after OAuth the user lands on their personal org page
- Project creation form on the org page (no database access needed)
- API key creation with expiration date, post-creation banner, inline revoke confirmation
- All push endpoints: create run, append events, update run status, create sweep, register sweep child runs, update sweep status, create training run, append training metrics, update training run status
- All read endpoints: project metadata, runs list with filter and sort, run detail, events with polling cursor, sweep, training run, sweep child runs, training metrics
- Project home page server-renders the runs table; empty state shows a pre-filled install snippet
- Run detail page server-renders metadata and streams events via the trace viewer
- Sweep dashboard: KPI strip, matrix view with inferred axes, flat child runs table
- Training run page: SVG line charts per metric, polls while running, final metrics table
- Comparison view: metadata table with diff highlighting, side-by-side event timeline
- Keyboard shortcuts: j/k navigation, Enter to open, / to focus filter, c to compare two selected runs

What ships in the next session:

- Share tokens for read-only project access via URL
- Project settings (rename, change visibility)
- Multi-org and team membership management
