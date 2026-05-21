# Getting started with Stage

Stage records runs from ensemble scenarios and lets you browse traces, compare outcomes, and monitor training jobs through a web UI. This guide walks you from a fresh account to having a run visible in the trace viewer.

By the end you will have signed in, created a project, created an API key, pushed a run with a handful of events, and confirmed that the run detail page shows the trace and the outcome scores.

## Sign in

Navigate to the Stage server in your browser. The landing page has a "Sign in with GitHub" button. After you authorize, GitHub redirects back to Stage, which creates your account using your GitHub login as your personal org slug. You land on your personal org page.

## Create a project

Your personal org page shows an inline form. Fill in a slug (lowercase letters, digits, and hyphens; for example `experiments`), a display name, and optionally a description. Check "Public" if you want the project visible without sign-in. Click "Create project."

Stage redirects you to the new project page at `/{your-login}/experiments`.

## Create an API key

Go to `/me` (the link is in the top-right corner of every page). Enter a name for the key (for example "laptop"), leave the scope as "push", and click "Create key."

The raw key value is shown exactly once in a banner. Copy it now. The server stores only a SHA-256 hash; the value cannot be recovered after you dismiss the banner.

## Configure ensemble

Set these variables in your shell:

```bash
export ENSEMBLE_STAGE_API_KEY="stage_sk_..."  # the key you just copied
export ENSEMBLE_STAGE_PROJECT="your-login/experiments"
```

If you are pushing to a self-hosted server instead of the production instance, also set:

```bash
export ENSEMBLE_STAGE_BASE_URL="https://your-server.example.com"
```

The project page shows a pre-filled snippet with these variables. If you skipped creating a key, the snippet links you back to `/me` first.

## Push a first run

With the environment variables set, run any ensemble scenario:

```bash
ensemble run plank.refund_storm
```

The ensemble integration prints a run URL before the scenario starts:

```
Stage:  https://ensemble-stage.fly.dev/your-login/experiments/runs/019542a3-...
```

Open the URL. The trace viewer shows events streaming in as the scenario runs. After the run completes, the page shows the outcome scores, cost breakdown, and full event timeline.

## Verify in the UI

Refresh the project page. The run appears in the table. Click the ID to open the run detail page. Click another run, select both with the checkboxes in the table, and press `c` to open the comparison view.

## Smoke test without ensemble

If you want to verify Stage independently before wiring ensemble, use the smoke-test script:

```bash
cd integration && uv sync && cd ..
ENSEMBLE_STAGE_API_KEY="stage_sk_..." ENSEMBLE_STAGE_PROJECT="your-login/experiments" \
  uv run python integration/scripts/smoke_test.py
```

This pushes a synthetic run with seven events and prints the run URL.
