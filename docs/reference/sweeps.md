# Sweeps reference

A sweep is a set of runs sharing a configuration grid. Stage records all child runs under a single sweep entity and provides a detail page that lets a researcher answer five questions without navigating away: did it complete, which configurations won, where is the variance, what did it cost, and what should I look at next.

## Creating a sweep

Sweeps are created by the ensemble CLI during `ensemble sweep run sweep.toml`. The CLI posts to `POST /v1/projects/{org}/{project}/sweeps` before launching cells, then registers each child run with `POST /v1/sweeps/{id}/runs`. Sweeps can also be created directly via the API for custom orchestration.

```bash
ensemble sweep run sweep.toml
# Sweep: https://stage.ensemble.sh/org/proj/sweeps/019a1b2c-...
```

## Reading the sweep detail page

The sweep detail page at `/{org}/{project}/sweeps/{id}` has six sections.

### KPI strip

Six cells across the top of the page:

| Cell | What it shows |
|------|---------------|
| Total runs | Count of all child runs registered to this sweep |
| Completed | Count and percentage of runs with status `completed` |
| Failed | Count and percentage; rendered in red when nonzero |
| Running | Count and percentage of runs still in progress |
| Total cost | Sum of `total_cost.usd` across completed runs |
| Mean outcome | Mean of the primary outcome score across completed runs, with standard error |

The strip polls every 2 seconds while the sweep status is `running` and stops when the sweep reaches a terminal state. A high standard error relative to the mean indicates noisy results; consider more trials per cell.

### Matrix view

The matrix is a two-dimensional table whose rows and columns correspond to two sweep axes. The axis dropdowns above the matrix let you choose which dimensions appear. Any axes not shown appear in the per-axis breakdown section.

Each cell shows three pieces of information:

- A row of small colored squares, one per run: green for completed, red for failed, amber (pulsing) for running, gray for not yet started. If a cell has more than six runs, the first six are shown with a count of the rest.
- The mean primary outcome score in monospace type.
- A box plot glyph when the "Show variance" toggle is on and the cell has two or more runs. The box marks the interquartile range, the center line marks the median, and the caps mark the minimum and maximum. A wide box means the result varies significantly across trials of the same configuration.

Cell background color shifts toward green for cells where all runs completed and toward red for cells where all runs failed.

Hovering a cell shows a tooltip with the per-run breakdown: ID prefix, status, score, and cost. Clicking a single-run cell opens that run's detail page. Clicking a multi-run cell scrolls the page to the flat runs list and applies a filter.

### Cost over time chart

A cumulative line chart of USD spend over wall-clock time. Each point is one completed run; the dot is green for a successful run and red for a failed one. The slope shows the burn rate. A flat segment indicates cheap or free runs; a steep jump indicates an expensive single run.

When no runs recorded cost data, the section shows a brief explanation of why.

### Per-axis breakdown

A table for each axis not currently shown in the matrix rows or columns. Each table lists distinct values with the number of runs, mean outcome score, and total cost for that value. Clicking a row filters the flat runs list to just runs with that axis value.

Use this section to discover which axes matter. If "backend" shows identical mean scores across all values, the backend choice is not a significant factor for this scenario.

### Flat runs list

All runs in the sweep as a table with ID, scenario, world, model, status, outcome score, cost, and duration. Each ID links to the run's detail page. When a filter is active (from a matrix cell click or axis breakdown row click), a banner above the table identifies the filter and offers a "Clear filter" button.

### Actions

Three buttons appear above the KPI strip:

- **Share**: copies the current URL to the clipboard.
- **Compare two runs**: opens a picker where you select two runs from this sweep and opens the Stage comparison view.
- **Cancel sweep**: visible only when status is `running`. Asks for confirmation before posting `POST /v1/sweeps/{id}/cancel`. In-flight runs are not stopped; Stage only observes them.

## Polling behavior

Pages for sweeps in `running` status poll every 2 seconds for updated run data and stop when the sweep reaches `completed`, `failed`, or `cancelled`. Polling pauses automatically when the browser tab is backgrounded (Page Visibility API) and resumes when the tab comes back to the foreground.

## API endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/projects/{org}/{proj}/sweeps` | Create sweep. Body: `{config: object}`. |
| `GET` | `/v1/projects/{org}/{proj}/sweeps` | List sweeps, newest first. |
| `GET` | `/v1/sweeps/{id}` | Get sweep metadata and status. |
| `GET` | `/v1/sweeps/{id}/runs` | All child runs with outcomes and costs. |
| `POST` | `/v1/sweeps/{id}/runs` | Register an existing run as a child. Body: `{run_id}`. |
| `POST` | `/v1/sweeps/{id}/status` | Update sweep status. Body: `{status}`. |
| `POST` | `/v1/sweeps/{id}/cancel` | Cancel a running sweep. No body required. |
