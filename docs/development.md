# Development guide

This document covers running Stage locally and verifying the full push-to-viewer path with the "first push" smoke test.

## Prerequisites

- Rust stable (1.80 or later). Install with `rustup`.
- PostgreSQL 15 or 16 running locally.
- `sqlx-cli` for running migrations: `cargo install sqlx-cli --no-default-features --features postgres`
- `uv` for the Python integration package.

## Setting up the database

Create a local database and run the migrations:

```bash
createdb stage_dev
export DATABASE_URL=postgres://localhost/stage_dev

sqlx migrate run --source ops/migrations
```

## Configuring the server

Copy the example env file and edit it:

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

`GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` can be left empty for local development if you create users and API keys directly in the database (see below). Set them if you want GitHub OAuth to work locally; the callback URL to register in the GitHub OAuth App settings is `http://localhost:3000/auth/github/callback`.

## Running the server

```bash
cargo run --bin stage-server
```

The server listens on `http://localhost:3000`. Template changes do not require a rebuild since askama compiles templates at build time; you need to `cargo build` after changing any template file.

Static assets (`web/static/`) are served from disk on every request; you can edit CSS and JS without rebuilding.

## Creating a test user and project (without GitHub OAuth)

Insert directly while developing:

```sql
-- In psql:
INSERT INTO orgs (slug, name) VALUES ('demo', 'Demo Org') RETURNING id;
-- Note the org id; assume it's 1 below.

INSERT INTO users (github_id, github_login, default_org_id)
VALUES (1, 'demo', 1) RETURNING id;
-- Assume user id is 1.

INSERT INTO org_members (org_id, user_id, role) VALUES (1, 1, 'owner');

INSERT INTO projects (org_id, slug, name, public)
VALUES (1, 'smoke-test', 'Smoke Test', true) RETURNING id;
-- Assume project id is 1.
```

Create an API key for the user. The key value is SHA-256-hashed in the database. Generate one with the helper in the integration package:

```python
# uv run python -c "..."
import hashlib, secrets
raw = "stage_sk_" + secrets.token_hex(24)
hashed = hashlib.sha256(raw.encode()).hexdigest()
print(f"raw key (copy this): {raw}")
print(f"hash to insert:      {hashed}")
```

Then insert it:

```sql
INSERT INTO api_keys (user_id, scope, name, key_hash)
VALUES (1, 'push', 'local dev key', '<hash from above>');
```

## First-push smoke test

This test creates a synthetic run with seven events and verifies the server accepts them.

**Install the integration package:**

```bash
cd integration
uv sync
cd ..
```

**Set environment variables:**

```bash
export STAGE_API_KEY=stage_sk_...   # the raw key from above
export STAGE_BASE_URL=http://localhost:3000
export STAGE_PROJECT=demo/smoke-test
```

**Run the smoke test:**

```bash
uv run python integration/scripts/smoke_test.py
```

Expected output:

```
Creating run on http://localhost:3000 in project demo/smoke-test …
  run id:  019542a3-...
  run url: http://localhost:3000/demo/smoke-test/runs/019542a3-...
  status -> running
  pushed 7 events
  status -> completed

Final status: completed
Outcome:      {"scores": {"correctness": 1.0}}

Open the trace viewer:
  http://localhost:3000/demo/smoke-test/runs/019542a3-...

Smoke test passed.
```

**Verify in the browser:**

Open the run URL. The trace viewer should load and show:
- A "completed" status badge in the sidebar.
- Seven events in the timeline view (actor message from user, actor message from agent, tool call, tool result, cost record, and two system notes).
- The chat view showing the user/agent message exchange.
- Outcome `{"scores": {"correctness": 1.0}}` in the sidebar.

If any step fails, check the server log (`RUST_LOG=debug cargo run`) for details. Common issues:

- `401 Unauthorized`: API key hash does not match. Recheck the SHA-256 computation.
- `404 Not Found` on run creation: The project `demo/smoke-test` does not exist. Check the insert above.
- Template render errors: Run `cargo build` to recompile templates and see any compile-time errors.

## Running tests

```bash
# Rust tests (requires DATABASE_URL to be set).
cargo test --all

# Python integration tests.
cd integration && uv run pytest
```

## Hot-reloading during development

The server does not hot-reload. For rapid iteration on templates or static assets, use `cargo-watch`:

```bash
cargo install cargo-watch
cargo watch -x 'run --bin stage-server'
```

This rebuilds and restarts the server on any file change under `server/` or `web/`. CSS and JS changes under `web/static/` do not require a restart since they are served from disk.
