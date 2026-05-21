# Stage API v1

Stage exposes a single versioned API under `/v1`. All endpoints return JSON. Successful responses use `200 OK` for reads, `201 Created` for resource creation, and `200 OK` for mutations (status updates, appending events). Errors always return a JSON object with an `error` key.

**Base URL:** `https://stage.ensemble.sh` (or your self-hosted instance).

**Versioning:** The `/v1` prefix is stable. Breaking changes will appear under `/v2`.

## Authentication

Two authentication mechanisms are supported.

**Session cookies** authenticate web-browser requests. The session cookie (`stage_session`) is set by the GitHub OAuth flow and is a signed JWT. All web-page routes and read endpoints accept it.

**API keys** authenticate push requests from the ensemble integration. Pass the key as a Bearer token in the `Authorization` header:

```
Authorization: Bearer stage_sk_...
```

API keys have a `scope` field:
- `push`: create runs, append events, update run status. Sufficient for the integration.
- `admin`: all push operations plus project and key management.

Keys are created through the web UI at `/me` or via `POST /v1/me/api_keys`.

## Error shape

All errors return JSON with this structure:

```json
{
  "error": {
    "code": "not_found",
    "message": "not found"
  }
}
```

Error codes:

| Code | HTTP status | Meaning |
|------|-------------|---------|
| `unauthorized` | 401 | No credentials or invalid credentials. |
| `forbidden` | 403 | Credentials are valid but insufficient for this operation. |
| `not_found` | 404 | Resource does not exist or is not visible to the caller. |
| `bad_request` | 400 | Malformed request body or invalid field values. |
| `conflict` | 409 | Duplicate resource (e.g., duplicate `event_id`). |
| `internal_error` | 500 | Server-side failure. |

## Projects

### `GET /v1/projects/{org_slug}/{project_slug}`

Returns metadata for a project.

**Auth:** Public projects require no credentials. Private projects require a session cookie, an API key, or a share token passed as `?token=<share_token>`.

**Response `200`:**

```json
{
  "org_slug": "myorg",
  "project_slug": "popcornbench",
  "name": "Popcornbench",
  "public": true,
  "description": "Evaluation harness for the popcorn scenario family.",
  "created_at": "2025-01-15T09:23:00Z"
}
```

### `GET /v1/projects/{org_slug}/{project_slug}/runs`

List runs for a project, newest first by default. Supports filtering and sorting.

**Auth:** Same as `GET /v1/projects/...` above.

**Query parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `filter` | string | Substring match against scenario, world, backend, or status. |
| `sort` | string | `created_at:desc` (default), `created_at:asc`, `wall_time_ms:asc`, `wall_time_ms:desc`. |
| `limit` | integer | Maximum number of runs to return. Default 50, max 200. |
| `cursor` | string | Opaque pagination cursor returned in the previous response. |

**Response `200`:**

```json
{
  "runs": [
    {
      "id": "019542a3-...",
      "scenario": "popcorn.single_problem",
      "world": "popcorn",
      "backend": "claude-sonnet-4-5",
      "status": "completed",
      "started_at": "2025-01-15T09:30:00Z",
      "ended_at": "2025-01-15T09:30:47Z",
      "wall_time_ms": 47000,
      "sweep_id": null
    }
  ],
  "next_cursor": "eyJjcmVhdGVkX2F0IjoiMjAyNS0wMS0xNVQwOTozMDowMFoifQ"
}
```

Pass `next_cursor` as the `cursor` parameter to retrieve the next page. When `next_cursor` is null, there are no more results.

### `POST /v1/projects/{org_slug}/{project_slug}/runs`

Create a run. The org and project must already exist. Called by the ensemble integration at scenario start.

**Auth:** API key with `push` scope.

**Request body:**

```json
{
  "scenario": "popcorn.single_problem",
  "world": "popcorn",
  "backend": "claude-sonnet-4-5",
  "sweep_id": null,
  "metadata": {
    "seed": 42,
    "git_sha": "a1b2c3d"
  }
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `scenario` | yes | Scenario identifier, e.g. `popcorn.single_problem`. |
| `world` | yes | World identifier, e.g. `popcorn`. |
| `backend` | yes | Model or backend string, e.g. `claude-sonnet-4-5`. |
| `sweep_id` | no | UUID of the parent sweep, if this run is part of one. |
| `metadata` | no | Arbitrary JSON object. |

**Response `201`:**

```json
{
  "id": "019542a3-4e7b-7000-8e1d-3f9a1c2d5e6f",
  "url": "https://stage.ensemble.sh/myorg/popcornbench/runs/019542a3-..."
}
```

### `POST /v1/projects/{org_slug}/{project_slug}/sweeps`

Create a sweep.

**Auth:** API key with `push` scope.

**Request body:**

```json
{
  "config": {
    "scenarios": ["popcorn.single_problem"],
    "backends": ["claude-sonnet-4-5", "claude-opus-4-7"],
    "n_trials": 5
  }
}
```

**Response `201`:**

```json
{
  "id": "019542b0-...",
  "url": "https://stage.ensemble.sh/myorg/popcornbench/sweeps/019542b0-..."
}
```

### `POST /v1/projects/{org_slug}/{project_slug}/training_runs`

Create a training run.

**Auth:** API key with `push` scope.

**Request body:**

```json
{
  "persona_name": "popcorn-v2",
  "base_model": "claude-haiku-4-5",
  "hyperparameters": {
    "learning_rate": 1e-4,
    "batch_size": 32,
    "max_steps": 10000
  }
}
```

**Response `201`:**

```json
{
  "id": "019542c1-...",
  "url": "https://stage.ensemble.sh/myorg/popcornbench/training_runs/019542c1-..."
}
```

## Runs

### `GET /v1/runs/{run_id}`

Returns full run metadata.

**Auth:** Public-project runs require no credentials. Private-project runs require session cookie, API key, or share token.

**Response `200`:**

```json
{
  "id": "019542a3-4e7b-7000-8e1d-3f9a1c2d5e6f",
  "project_id": 17,
  "scenario": "popcorn.single_problem",
  "world": "popcorn",
  "backend": "claude-sonnet-4-5",
  "status": "completed",
  "outcome": {
    "scores": {
      "correctness": 0.92,
      "efficiency": 0.78
    }
  },
  "wall_time_ms": 47000,
  "total_cost": {
    "input_tokens": 12400,
    "output_tokens": 3200,
    "usd": 0.0183
  },
  "started_at": "2025-01-15T09:30:00Z",
  "ended_at": "2025-01-15T09:30:47Z",
  "sweep_id": null,
  "metadata": { "seed": 42 },
  "created_at": "2025-01-15T09:29:58Z"
}
```

### `POST /v1/runs/{run_id}/events`

Append events to a run. Idempotent on `event_id`: re-sending an event that was already accepted is a no-op; the response still returns the accepted count.

**Auth:** API key with `push` scope.

**Request body:**

```json
{
  "events": [
    {
      "sequence_number": 1,
      "kind": "system",
      "payload": {
        "note": "run started",
        "actor": "system"
      },
      "event_id": "4f9a1b2c-...",
      "wall_time_ms": 0
    },
    {
      "sequence_number": 2,
      "kind": "agent_message",
      "payload": {
        "actor": "agent:0",
        "kind": "agent_message",
        "text": "I see a popcorn kernel. Let me begin."
      },
      "event_id": "7d3e2a1b-...",
      "wall_time_ms": 214
    }
  ]
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `sequence_number` | yes | 1-indexed, must be unique per run. The client is responsible for assigning sequence numbers; gaps are allowed. |
| `kind` | yes | Event kind string. The viewer understands `system`, `user_message`, `agent_message`, `tool_call`, `tool_result`, `state_diff`, `cost`, `progress`. Other strings are stored and returned as-is. |
| `payload` | yes | Arbitrary JSON object. For kinds the viewer understands, see the payload schemas below. |
| `event_id` | yes | Client-generated UUID for idempotency. Two events submitted with the same `event_id` within the same run are deduplicated; the second is silently dropped. |
| `wall_time_ms` | no | Milliseconds since run start, used for timeline display. |

Up to 500 events may be sent in a single request. The integration sends batches of up to 100.

**Response `200`:**

```json
{ "accepted": 2 }
```

#### Event payload schemas

**`system`** — lifecycle or grader notes:

```json
{ "note": "run started", "actor": "system" }
```

Grader output uses a structured prefix:

```json
{ "note": "grader: {\"scenario\": \"popcorn\", \"scores\": {\"correctness\": 0.92}}" }
```

**`user_message`** — a message from a human or user-role actor:

```json
{ "actor": "user", "kind": "user_message", "text": "Solve this problem." }
```

**`agent_message`** — a message from an agent:

```json
{ "actor": "agent:0", "kind": "agent_message", "text": "I will begin by..." }
```

**`tool_call`** — an agent invoking a tool:

```json
{
  "actor": "agent:0",
  "kind": "tool_call",
  "name": "bash",
  "args": { "command": "ls /tmp" },
  "seed": false
}
```

**`tool_result`** — the result of a tool invocation:

```json
{
  "actor": "agent:0",
  "kind": "tool_result",
  "name": "bash",
  "result": { "summary": "file1.txt\nfile2.txt" }
}
```

**`state_diff`** — a change to world state:

```json
{
  "actor": "agent:0",
  "kind": "state_diff",
  "diff": {
    "table": "inventory",
    "field": "popcorn_count",
    "old": 0,
    "new": 10
  }
}
```

**`cost`** — a cost accumulation record:

```json
{
  "actor": "agent:0",
  "kind": "cost",
  "unit": "usd",
  "amount": 0.0012,
  "running_total": 0.0183
}
```

**`progress`** — a progress report from a long tool call:

```json
{
  "actor": "agent:0",
  "kind": "progress",
  "tool": "compile",
  "fraction": 0.45,
  "message": "compiling module 9 of 20"
}
```

### `GET /v1/runs/{run_id}/events`

Return events for a run. Used by the trace viewer for polling during live runs.

**Auth:** Same as `GET /v1/runs/{run_id}`.

**Query parameters:**

| Parameter | Description |
|-----------|-------------|
| `since` | Return only events with `sequence_number` greater than this value. Defaults to -1 (return all events). |

**Response `200`:**

```json
[
  {
    "run_id": "019542a3-...",
    "sequence_number": 1,
    "kind": "system",
    "payload": { "note": "run started", "actor": "system" },
    "event_id": "4f9a1b2c-...",
    "wall_time_ms": 0
  }
]
```

Events are returned in ascending `sequence_number` order. The viewer polls every two seconds with `?since={last_seen_sequence_number}` while the run status is `running` or `queued`.

### `POST /v1/runs/{run_id}/status`

Update a run's status. Called by the integration at scenario completion or failure, and again when the run transitions to `running`.

**Auth:** API key with `push` scope.

**Request body:**

```json
{
  "status": "completed",
  "outcome": {
    "scores": { "correctness": 0.92, "efficiency": 0.78 }
  },
  "total_cost": {
    "input_tokens": 12400,
    "output_tokens": 3200,
    "usd": 0.0183
  },
  "wall_time_ms": 47000
}
```

| Field | Required when | Description |
|-------|---------------|-------------|
| `status` | always | One of `running`, `completed`, `failed`, `cancelled`. |
| `outcome` | completing | Grader scores and any structured outcome data. |
| `total_cost` | completing | Final cost breakdown. |
| `wall_time_ms` | completing | Total wall-clock duration in milliseconds. |

`outcome`, `total_cost`, and `wall_time_ms` are ignored for transitions to `running` or `cancelled`.

**Response `200`:**

```json
{ "ok": true }
```

## Sweeps

### `GET /v1/sweeps/{sweep_id}`

Returns a sweep and its status.

**Auth:** Same as project read access.

**Response `200`:**

```json
{
  "id": "019542b0-...",
  "project_id": 17,
  "config": {
    "scenarios": ["popcorn.single_problem"],
    "backends": ["claude-sonnet-4-5"],
    "n_trials": 5
  },
  "status": "running",
  "started_at": "2025-01-15T09:00:00Z",
  "ended_at": null,
  "created_at": "2025-01-15T08:59:58Z"
}
```

### `POST /v1/sweeps/{sweep_id}/runs`

Register an existing run as a child of a sweep. The run must already exist and must belong to the same project.

**Auth:** API key with `push` scope.

**Request body:**

```json
{ "run_id": "019542a3-..." }
```

**Response `200`:**

```json
{ "ok": true }
```

### `POST /v1/sweeps/{sweep_id}/status`

Update sweep status.

**Auth:** API key with `push` scope.

**Request body:**

```json
{ "status": "completed" }
```

Valid values: `running`, `completed`, `failed`, `cancelled`.

**Response `200`:**

```json
{ "ok": true }
```

## Training runs

### `GET /v1/training_runs/{id}`

Returns a training run.

**Auth:** Same as project read access.

**Response `200`:**

```json
{
  "id": "019542c1-...",
  "project_id": 17,
  "persona_name": "popcorn-v2",
  "base_model": "claude-haiku-4-5",
  "status": "completed",
  "hyperparameters": {
    "learning_rate": 1e-4,
    "batch_size": 32,
    "max_steps": 10000
  },
  "final_metrics": {
    "train_loss": 0.32,
    "eval_loss": 0.41
  },
  "artifact_uri": "gs://ensemble-artifacts/adapters/popcorn-v2.safetensors",
  "started_at": "2025-01-14T14:00:00Z",
  "ended_at": "2025-01-14T18:22:00Z",
  "created_at": "2025-01-14T13:59:45Z"
}
```

### `POST /v1/training_runs/{id}/metrics`

Append metric measurements. Called periodically during training.

**Auth:** API key with `push` scope.

**Request body:**

```json
{
  "metrics": [
    { "step": 100,  "metric_name": "train_loss", "value": 1.42 },
    { "step": 100,  "metric_name": "eval_loss",  "value": 1.57 },
    { "step": 200,  "metric_name": "train_loss", "value": 1.18 },
    { "step": 200,  "metric_name": "eval_loss",  "value": 1.31 }
  ]
}
```

Duplicate `(step, metric_name)` pairs within the same training run are ignored. Up to 1000 metric points may be sent per request.

**Response `200`:**

```json
{ "accepted": 4 }
```

### `POST /v1/training_runs/{id}/status`

Update training run status.

**Auth:** API key with `push` scope.

**Request body:**

```json
{
  "status": "completed",
  "final_metrics": {
    "train_loss": 0.32,
    "eval_loss": 0.41
  },
  "artifact_uri": "gs://ensemble-artifacts/adapters/popcorn-v2.safetensors"
}
```

`final_metrics` and `artifact_uri` are optional and ignored when transitioning to `running` or `cancelled`.

**Response `200`:**

```json
{ "ok": true }
```

## Account and API keys

### `GET /v1/me`

Returns the authenticated user's profile and default org.

**Auth:** Session cookie or API key with `admin` scope.

**Response `200`:**

```json
{
  "id": 3,
  "github_login": "tprabhune",
  "email": "tejas@example.com",
  "default_org_slug": "tprabhune"
}
```

### `GET /v1/me/api_keys`

List the caller's API keys. Key values are not returned; only metadata.

**Auth:** Session cookie or API key with `admin` scope.

**Response `200`:**

```json
[
  {
    "id": 7,
    "name": "workstation",
    "scope": "push",
    "last_used_at": "2025-01-15T09:30:00Z",
    "created_at": "2025-01-10T11:00:00Z",
    "expires_at": null
  }
]
```

### `POST /v1/me/api_keys`

Create a new API key. The raw key value is returned exactly once; store it immediately.

**Auth:** Session cookie.

**Request body:**

```json
{
  "name": "workstation",
  "scope": "push",
  "expires_at": null
}
```

`expires_at` is an ISO 8601 timestamp or null for a non-expiring key.

**Response `201`:**

```json
{
  "id": 7,
  "name": "workstation",
  "scope": "push",
  "key": "stage_sk_3f9a1b2c..."
}
```

Keys are prefixed `stage_sk_` for push scope and `stage_ak_` for admin scope. The server stores only the SHA-256 hash; the raw key cannot be recovered.

### `DELETE /v1/me/api_keys/{id}`

Revoke an API key. Idempotent: revoking an already-revoked key returns 200.

**Auth:** Session cookie.

**Response `200`:**

```json
{ "ok": true }
```

## Auth endpoints

These are not JSON API endpoints; they handle the GitHub OAuth browser flow.

### `GET /auth/github/login`

Redirects the browser to GitHub's OAuth authorization page. No body or credentials required.

### `GET /auth/github/callback`

GitHub redirects here after the user authorizes. The server exchanges the code for an access token, fetches the user's GitHub profile, upserts the user and personal org in the database, and sets a signed `stage_session` cookie. Redirects to `/` on success.

On first login the personal org slug is set to the user's GitHub login. Subsequent logins update the stored `github_login` and `email` if they changed.

### `POST /auth/logout`

Clears the `stage_session` cookie and redirects to `/`.

## Push workflow example

This is the call sequence the Python integration makes for a single run. The `Authorization` header contains the API key.

```bash
# 1. Create the run.
curl -X POST https://stage.ensemble.sh/v1/projects/myorg/popcornbench/runs \
  -H "Authorization: Bearer stage_sk_3f9a..." \
  -H "Content-Type: application/json" \
  -d '{
    "scenario": "popcorn.single_problem",
    "world": "popcorn",
    "backend": "claude-sonnet-4-5",
    "metadata": {"seed": 42}
  }'
# Response: {"id": "019542a3-...", "url": "https://..."}

# 2. Transition to running.
curl -X POST https://stage.ensemble.sh/v1/runs/019542a3-.../status \
  -H "Authorization: Bearer stage_sk_3f9a..." \
  -H "Content-Type: application/json" \
  -d '{"status": "running"}'

# 3. Stream events as the scenario runs (batched, every ~1 second).
curl -X POST https://stage.ensemble.sh/v1/runs/019542a3-.../events \
  -H "Authorization: Bearer stage_sk_3f9a..." \
  -H "Content-Type: application/json" \
  -d '{
    "events": [
      {
        "sequence_number": 1,
        "kind": "system",
        "payload": {"note": "run started", "actor": "system"},
        "event_id": "4f9a1b2c-3d4e-5f6a-7b8c-9d0e1f2a3b4c",
        "wall_time_ms": 0
      }
    ]
  }'

# 4. Finalize with outcome and cost.
curl -X POST https://stage.ensemble.sh/v1/runs/019542a3-.../status \
  -H "Authorization: Bearer stage_sk_3f9a..." \
  -H "Content-Type: application/json" \
  -d '{
    "status": "completed",
    "outcome": {"scores": {"correctness": 0.92}},
    "total_cost": {"input_tokens": 12400, "output_tokens": 3200, "usd": 0.0183},
    "wall_time_ms": 47000
  }'
```

The Python integration wraps steps 1-4 in the `Stage.run()` context manager and handles buffering and retry internally.

## ID format

Run, sweep, and training run IDs are UUID version 7 (time-ordered). They sort lexicographically in creation order, which the database exploits for efficient list queries. Other entity IDs (orgs, users, projects, API keys) use auto-incrementing integers.
