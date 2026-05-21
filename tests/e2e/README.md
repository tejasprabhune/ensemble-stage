# End-to-end tests

Playwright tests for ensemble-stage. Each test function gets its own
isolated postgres database, a running Stage server on a random port, and
a browser context with a test user already signed in.

## Requirements

- Rust release binary at `../../target/release/stage-server`. Run
  `cargo build --release` from the repo root if it does not exist.
- `sqlx` CLI available: `cargo install sqlx-cli --no-default-features --features postgres`
- Local postgres accepting connections at `postgres://localhost/postgres`.
  Override with `TEST_DATABASE_URL`.
- Python 3.11+ with `pytest`, `pytest-playwright`, `psycopg2-binary`, and `requests`.
  Install with `uv pip install pytest pytest-playwright psycopg2-binary requests`.
- Chromium browser for Playwright:
  `playwright install chromium`

## Run locally

```
# From the repo root
cargo build --release
cd tests/e2e
python3 -m pytest -v
```

Run a specific file:
```
python3 -m pytest test_run_detail.py -v
```

Run a single test:
```
python3 -m pytest test_run_detail.py::test_run_detail_populates_all_sections -v
```

## Debug a failing test

Run headed (opens a real browser window):
```
PWDEBUG=1 python3 -m pytest test_compare.py::test_compare_view_with_two_runs --headed -v
```

Pause at a specific point by adding `page.pause()` in the test body.

Take a screenshot inside a test:
```python
page.screenshot(path="/tmp/debug.png")
```

## How the fixtures work

`test_db` (function-scoped): creates a fresh postgres database named
`stage_e2e_{pid}_{timestamp}`, runs migrations against it, and drops it
after the test. Each test function gets an independent database with no
shared state.

`live_server` (function-scoped): starts the release server binary against
`test_db` on a random port, waits until it accepts HTTP connections, yields
the base URL, then terminates the process after the test.

`test_user` (function-scoped): inserts a user, org, org membership, and push
API key directly into `test_db`. Returns a dict with `user_id`,
`github_login`, `org_slug`, `api_key` (raw), and `jwt` (minted directly
from the JWT secret without going through OAuth).

`authed_page` (function-scoped): takes the built-in `page` fixture from
pytest-playwright, adds the test user's session cookie to the browser
context, and yields the page. The cookie is a real JWT signed with the
same secret the server was started with.

`test_project` (function-scoped): inserts a project in the test user's org
directly in the database.

`pushed_run`, `pushed_sweep` (function-scoped): call the Stage HTTP API to
push a complete run or sweep with events, then return the entity ID.

## CI

The `e2e.yml` workflow runs on every push to main and any `stage-*` branch,
and on pull requests. It builds the release binary, sets up the Python
environment, and runs `pytest tests/e2e/`. Screenshots from failing tests
are uploaded as artifacts under the "e2e-screenshots" name.

To run the same workflow locally:
```
# Must have a postgres listening on localhost:5432
export TEST_DATABASE_URL=postgres://localhost/postgres
cargo build --release
python3 -m pytest tests/e2e/ -v
```
