#!/usr/bin/env python3
"""
First-push smoke test for Stage.

Creates one synthetic run with a handful of events, polls until the
server acknowledges them, then prints the run URL.

Usage:
  STAGE_API_KEY=<key> STAGE_BASE_URL=http://localhost:3000 \\
    uv run python integration/scripts/smoke_test.py

Prerequisites:
  1. A running Stage server (cargo run from the repo root).
  2. A project that already exists on that server (create it via the UI
     or via `POST /v1/projects/<org>/<project>`).
  3. A push-scoped API key from /me.

Set STAGE_PROJECT to override the default project slug.
"""

import json
import os
import sys
import time
import uuid

import requests

BASE_URL = os.environ.get("STAGE_BASE_URL", "http://localhost:3000").rstrip("/")
API_KEY  = os.environ.get("STAGE_API_KEY", "")
PROJECT  = os.environ.get("STAGE_PROJECT", "demo/smoke-test")

if not API_KEY:
    print("error: set STAGE_API_KEY", file=sys.stderr)
    sys.exit(1)

SESSION = requests.Session()
SESSION.headers.update({
    "Authorization": f"Bearer {API_KEY}",
    "Content-Type":  "application/json",
    "User-Agent":    "smoke-test/0.1",
})


def post(path, body):
    r = SESSION.post(f"{BASE_URL}{path}", json=body)
    r.raise_for_status()
    return r.json()


def get(path):
    r = SESSION.get(f"{BASE_URL}{path}")
    r.raise_for_status()
    return r.json()


def main():
    if "/" not in PROJECT:
        print(f"error: STAGE_PROJECT must be 'org_slug/project_slug', got {PROJECT!r}", file=sys.stderr)
        sys.exit(1)

    org, proj = PROJECT.split("/", 1)

    print(f"Creating run on {BASE_URL} in project {PROJECT} …")
    run = post(f"/v1/projects/{org}/{proj}/runs", {
        "scenario": "smoke.hello",
        "world":    "smoke",
        "backend":  "test-backend",
        "metadata": {"smoke_test": True, "ts": int(time.time())},
    })
    run_id = run["id"]
    run_url = run["url"]
    print(f"  run id:  {run_id}")
    print(f"  run url: {run_url}")

    post(f"/v1/runs/{run_id}/status", {"status": "running"})
    print("  status -> running")

    events = [
        {
            "sequence_number": 1,
            "kind": "system",
            "payload": {"note": "run started", "actor": "system"},
            "event_id": str(uuid.uuid4()),
            "wall_time_ms": 0,
        },
        {
            "sequence_number": 2,
            "kind": "user_message",
            "payload": {"actor": "user", "kind": "user_message", "text": "Hello from the smoke test."},
            "event_id": str(uuid.uuid4()),
            "wall_time_ms": 50,
        },
        {
            "sequence_number": 3,
            "kind": "agent_message",
            "payload": {"actor": "agent:0", "kind": "agent_message", "text": "Hello! The smoke test is passing."},
            "event_id": str(uuid.uuid4()),
            "wall_time_ms": 400,
        },
        {
            "sequence_number": 4,
            "kind": "tool_call",
            "payload": {
                "actor": "agent:0",
                "kind":  "tool_call",
                "name":  "echo",
                "args":  {"message": "smoke test complete"},
            },
            "event_id": str(uuid.uuid4()),
            "wall_time_ms": 420,
        },
        {
            "sequence_number": 5,
            "kind": "tool_result",
            "payload": {
                "actor":  "agent:0",
                "kind":   "tool_result",
                "name":   "echo",
                "result": {"summary": "smoke test complete"},
            },
            "event_id": str(uuid.uuid4()),
            "wall_time_ms": 430,
        },
        {
            "sequence_number": 6,
            "kind": "cost",
            "payload": {
                "actor":         "agent:0",
                "kind":          "cost",
                "unit":          "usd",
                "amount":        0.0001,
                "running_total": 0.0001,
            },
            "event_id": str(uuid.uuid4()),
            "wall_time_ms": 435,
        },
        {
            "sequence_number": 7,
            "kind": "system",
            "payload": {
                "note":  'grader: ' + json.dumps({
                    "scenario": "smoke.hello",
                    "scores": {"correctness": 1.0},
                }),
                "actor": "system",
            },
            "event_id": str(uuid.uuid4()),
            "wall_time_ms": 440,
        },
    ]

    result = post(f"/v1/runs/{run_id}/events", {"events": events})
    print(f"  pushed {result.get('accepted', '?')} events")

    post(f"/v1/runs/{run_id}/status", {
        "status":       "completed",
        "outcome":      {"scores": {"correctness": 1.0}},
        "total_cost":   {"usd": 0.0001},
        "wall_time_ms": 440,
    })
    print("  status -> completed")

    time.sleep(0.5)
    final = get(f"/v1/runs/{run_id}")
    print(f"\nFinal status: {final.get('status')}")
    print(f"Outcome:      {json.dumps(final.get('outcome'))}")
    print(f"\nOpen the trace viewer:\n  {run_url}")
    print("\nSmoke test passed.")


if __name__ == "__main__":
    main()
