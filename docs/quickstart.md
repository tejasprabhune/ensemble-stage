# Getting started with Stage

Stage records runs from ensemble scenarios and lets you browse traces, compare outcomes, and monitor training jobs through a web UI. This guide walks you from a fresh Stage server to having a real run visible in the trace viewer.

By the end you will have signed in, created an API key, pushed a run with a handful of events, and confirmed that the run detail page shows the trace and the outcome scores.

## Before you begin

You need a Stage server. If you are running one locally for development, see [docs/development.md](development.md) for setup instructions. If someone has already deployed Stage for your team, get the base URL from them.

The examples below use `http://localhost:3000`. Replace that with your server's URL wherever it appears.

## Sign in

Navigate to the server in your browser. The landing page has a "Sign in with GitHub" button. Clicking it redirects you to GitHub's OAuth authorization page. After you approve, GitHub redirects back to Stage, which creates your account (using your GitHub login as your personal org slug) and sets a session cookie. You are now signed in.

Your personal org is named after your GitHub login. All projects you create live under that org by default.

## Create an API key

The ensemble integration authenticates with a push-scoped API key, not with your browser session. Go to `/me` (there is a link in the top-right corner of every page) and use the key creation form. Give the key a name you will recognize later -- "laptop" or "workstation" works -- and leave the scope as "push".

After you click "Create key", Stage shows the raw key value once. Copy it now; it cannot be recovered after you close the page. The server stores only a SHA-256 hash.

The key looks like `stage_sk_` followed by 64 hex characters. Set it in your environment:

```bash
export STAGE_API_KEY=stage_sk_...
export STAGE_BASE_URL=http://localhost:3000
```

## Create a project

Stage does not yet have a web form for project creation. Until that ships, create one directly in the database. Connect to the database your Stage server is using and run:

```sql
-- Find your org's ID first.
SELECT id FROM orgs WHERE slug = 'your-github-login';

-- Create the project under that org.
INSERT INTO projects (org_id, slug, name, public)
VALUES (<org_id>, 'my-project', 'My Project', true);
```

Replace `your-github-login` with your GitHub username (the personal org slug from sign-in) and choose a slug for the project. The slug appears in the URL and in the `STAGE_PROJECT` environment variable.

```bash
export STAGE_PROJECT=your-github-login/my-project
```

## Push your first run

The repository includes a smoke-test script that pushes one synthetic run with seven events and prints the run URL.

Install the integration package (requires `uv`):

```bash
cd integration
uv sync
cd ..
```

Run the test:

```bash
uv run python integration/scripts/smoke_test.py
```

If the environment variables above are set, the script uses them automatically. Expected output:

```
Creating run on http://localhost:3000 in project your-github-login/my-project …
  run id:  019542a3-4e7b-7000-8e1d-3f9a1c2d5e6f
  run url: http://localhost:3000/your-github-login/my-project/runs/019542a3-...
  status -> running
  pushed 7 events
  status -> completed

Final status: completed
Outcome:      {"scores": {"correctness": 1.0}}

Open the trace viewer:
  http://localhost:3000/your-github-login/my-project/runs/019542a3-...

Smoke test passed.
```

Open the run URL in your browser.

## What you see

The run detail page has three areas.

The header bar shows the run's scenario name, world, backend (model), and status. While a run is live its status reads "running" and the page polls the server every two seconds. When the run completes, the status updates automatically and polling stops.

The center pane is the trace viewer. It shows each event as a tile on a timeline. The viewer distinguishes actors (agents and users), tool calls and their results, state changes, and cost records. The chat tab shows the same events as a conversation thread, which is often easier to read when you are looking at a dialogue-heavy scenario.

The right sidebar shows cost, outcome scores, metadata, and -- once the next session's work is complete -- actors with their hidden state and predicates.

## Where to go next

Once the smoke test passes, the natural next step is to route real ensemble runs through Stage. The [ensemble integration guide](ensemble-integration.md) covers installing the `ensemble_stage` package, configuring it for your project, and mapping ensemble events to Stage's event kinds.

The [API reference](api.md) documents every endpoint in detail, including the full event payload schemas and the error shape. It is the authoritative contract for anything the integration or a custom client needs to do.

For self-hosting or deploying to a team environment, see [docs/deploy.md](deploy.md).
