Stage is a hosted observability and tracking service for [ensemble](https://ensemble.sh). It accepts run, sweep, and training-run data pushed from ensemble installations, and renders a web UI for browsing traces, comparing runs, and monitoring training progress. See [docs/api.md](docs/api.md) for the API specification and [docs/deploy.md](docs/deploy.md) for deployment instructions.

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

- Comparison view (selecting multiple runs for side-by-side trace comparison)
- Sweep dashboard with aggregated statistics across sweep runs
- Training run detail page with loss curves
- Share tokens for read-only project access without a login
- Mobile-responsive layout polish

## Development

See [docs/development.md](docs/development.md) for local setup instructions.
