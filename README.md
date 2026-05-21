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

- GitHub OAuth login and session cookies
- API key creation and verification (push and admin scope)
- All push endpoints: create run, append events, update run status, create sweep, register sweep child runs, update sweep status, create training run, append training metrics, update training run status
- All read endpoints: project metadata, runs list with filter and sort, run detail, events with polling cursor, sweep, training run
- Project home page server-renders the runs table with live data; filter and sort update the table via HTMX without a full page reload; pagination loads additional rows
- Run detail page populates metadata and streams events via the trace viewer
- Account page lists API keys and supports creation and revocation

What ships in the next session:

- Project creation web form (currently requires a direct database insert)
- Comparison view (selecting multiple runs for side-by-side trace comparison)
- Sweep dashboard with aggregated statistics across sweep runs
- Training run detail page with loss curves
- Share tokens for read-only project access without a login
