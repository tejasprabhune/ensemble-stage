# Development guide

This document covers running Stage locally, verifying the server with the automated test suite, and confirming the full push-to-viewer path manually.

## Prerequisites

You need Rust (stable, 1.80 or later; install with `rustup`), PostgreSQL 14 or later running locally, and `uv` if you want to run the Python smoke test. No other build tools are required.

## Setting up the database

Create a local database and apply the schema migration:

```bash
createdb stage_dev
psql stage_dev -f ops/migrations/001_initial_schema.sql
```

## Configuring the server

Create a `.env` file in the repository root:

```bash
cat > .env <<'EOF'
DATABASE_URL=postgres://localhost/stage_dev
PORT=3000
BASE_URL=http://localhost:3000
GITHUB_CLIENT_ID=
GITHUB_CLIENT_SECRET=
JWT_SECRET=dev-jwt-secret-do-not-use-in-production
EOF
```

`GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` can be left empty during development if you bootstrap users and API keys directly in the database (see below). Set them if you want GitHub OAuth to work; the callback URL to register in your GitHub OAuth App is `http://localhost:3000/auth/github/callback`.

## Running the server

```bash
cargo run --bin stage-server
```

The server listens on `http://localhost:3000`.

Askama compiles templates at build time, so changing a template file in `web/templates/` requires `cargo build` to take effect. Static assets under `web/static/` are served from disk on every request and can be edited without rebuilding.

## Creating a test user and project without OAuth

When developing against a local server without a GitHub OAuth App configured, bootstrap the database directly:

```sql
-- Connect with: psql stage_dev

-- Create the personal org first.
INSERT INTO orgs (slug, name) VALUES ('demo', 'Demo') RETURNING id;
-- Note the returned id; we'll call it <org_id>.

-- Create the user referencing that org.
INSERT INTO users (github_id, github_login, default_org_id)
VALUES (1, 'demo', <org_id>) RETURNING id;
-- Note the returned id; we'll call it <user_id>.

-- Make the user an owner of the org.
INSERT INTO org_members (org_id, user_id, role) VALUES (<org_id>, <user_id>, 'owner');

-- Create a project.
INSERT INTO projects (org_id, slug, name, public)
VALUES (<org_id>, 'smoke-test', 'Smoke Test', true);
```

Create a push-scoped API key for that user. The server stores only the SHA-256 hash; you need to generate the raw value yourself and record it before inserting:

```python
# uv run python -c "..."
import hashlib, uuid
raw = f"stage_sk_{uuid.uuid4().hex}{uuid.uuid4().hex}"
hashed = hashlib.sha256(raw.encode()).hexdigest()
print(f"raw key (copy this): {raw}")
print(f"hash to insert:      {hashed}")
```

Then insert the hash:

```sql
INSERT INTO api_keys (user_id, scope, name, key_hash)
VALUES (<user_id>, 'push', 'local dev key', '<hash from above>');
```

Set the environment variables:

```bash
export STAGE_API_KEY=stage_sk_...   # the raw key printed above
export STAGE_BASE_URL=http://localhost:3000
export STAGE_PROJECT=demo/smoke-test
```

## Running the automated tests

The Rust end-to-end test in `server/tests/e2e.rs` exercises the full push-to-view path against an isolated test database. It creates a user and project directly, creates an API key, pushes a run with 20 events across all event kinds, finalizes the run, and verifies the read endpoints return what was written.

```bash
DATABASE_URL=postgres://localhost/stage_dev cargo test --test e2e
```

The test creates and tears down its own database (using sqlx's test infrastructure) so it does not affect `stage_dev`. On success the test prints a summary:

```
Push-to-view smoke test passed.
  Run ID:   019542a3-...
  Status:   completed
  Events:   21 accepted, idempotency verified
  Outcome:  correctness=0.92, efficiency=0.78
  Cost:     $0.0183
```

## Manual smoke test with Python

The `integration/scripts/smoke_test.py` script pushes a synthetic run through the HTTP API and prints the run URL, which you can open in the browser to verify the trace viewer.

Install the integration package and run it:

```bash
cd integration && uv sync && cd ..
uv run python integration/scripts/smoke_test.py
```

Expected output ends with the run URL and "Smoke test passed." Open the URL to confirm the trace viewer loads, the status is "completed", and the seven events appear in the timeline.

If you see a 401, the API key hash does not match -- recheck the SHA-256 computation. If you see a 404 on run creation, the project `demo/smoke-test` does not exist -- run the SQL inserts above first.

## Hot-reloading during development

The server does not hot-reload automatically. `cargo-watch` rebuilds and restarts on file changes:

```bash
cargo install cargo-watch
cargo watch -x 'run --bin stage-server'
```

This picks up changes under `server/src/` and `web/templates/`. CSS and JS under `web/static/` are served from disk and take effect on the next browser request without a restart.
