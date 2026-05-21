"""
Pytest fixtures for ensemble-stage end-to-end tests.

Each test module gets an isolated postgres database, a running Stage
server on a random port, a test user with a valid session cookie, and
an API key for pushing runs. The ensemble CLI is also configured to
push to the test server.

Run locally:
  cd tests/e2e && uv run pytest -v
  PWDEBUG=1 uv run pytest -v --headed test_run_detail.py

Fixtures spin up their own postgres database via pg_tmp (or CREATE
DATABASE in the local postgres). The server binary is the release build
at ../../target/release/stage-server. If it doesn't exist, run
`cargo build --release` from the repo root first.
"""
from __future__ import annotations

import hashlib
import os
import re
import shutil
import signal
import socket
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Generator

import psycopg2
import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SERVER_BIN = REPO_ROOT / "target" / "release" / "stage-server"
MIGRATIONS_DIR = REPO_ROOT / "ops" / "migrations"
WEB_STATIC = REPO_ROOT / "web"
JWT_SECRET = "e2e-test-jwt-secret"
GITHUB_CLIENT_ID = "test-github-client-id"
GITHUB_CLIENT_SECRET = "test-github-client-secret"


def _free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _hash_api_key(raw: str) -> str:
    return hashlib.sha256(raw.encode()).hexdigest()


def _mint_jwt(user_id: int, login: str, secret: str = JWT_SECRET) -> str:
    import json
    import base64
    import hmac

    header = base64.urlsafe_b64encode(
        json.dumps({"alg": "HS256", "typ": "JWT"}).encode()
    ).rstrip(b"=").decode()

    exp = int(time.time()) + 86400 * 7
    payload = base64.urlsafe_b64encode(
        json.dumps({"sub": str(user_id), "login": login, "exp": exp}).encode()
    ).rstrip(b"=").decode()

    sig_input = f"{header}.{payload}".encode()
    sig = hmac.new(secret.encode(), sig_input, hashlib.sha256).digest()
    sig_b64 = base64.urlsafe_b64encode(sig).rstrip(b"=").decode()

    return f"{header}.{payload}.{sig_b64}"


def _run_migrations(db_url: str) -> None:
    result = subprocess.run(
        ["sqlx", "migrate", "run", "--source", str(MIGRATIONS_DIR)],
        env={**os.environ, "DATABASE_URL": db_url},
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Migration failed:\n{result.stderr}")


def _wait_for_server(base_url: str, timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            import urllib.request
            urllib.request.urlopen(base_url + "/", timeout=1)
            return
        except Exception:
            time.sleep(0.1)
    raise RuntimeError(f"Server at {base_url} did not start within {timeout}s")


@pytest.fixture(scope="session")
def server_bin() -> Path:
    if not SERVER_BIN.exists():
        pytest.skip(
            f"Release binary not found at {SERVER_BIN}. "
            "Run `cargo build --release` from the repo root first."
        )
    return SERVER_BIN


@pytest.fixture(scope="function")
def test_db():
    """Create and return an isolated postgres database for one test function."""
    db_name = f"stage_e2e_{os.getpid()}_{int(time.time() * 1000) % 100000}"
    admin_url = os.environ.get("TEST_DATABASE_URL", "postgres://localhost/postgres")

    conn = psycopg2.connect(admin_url)
    conn.autocommit = True
    cur = conn.cursor()
    cur.execute(f'CREATE DATABASE "{db_name}"')
    cur.close()
    conn.close()

    m = re.match(r"(postgres://[^/]*/)", admin_url)
    base = m.group(1) if m else "postgres://localhost/"
    db_url = f"{base}{db_name}"

    _run_migrations(db_url)

    yield db_url

    conn2 = psycopg2.connect(admin_url)
    conn2.autocommit = True
    c2 = conn2.cursor()
    c2.execute(f'DROP DATABASE "{db_name}" WITH (FORCE)')
    c2.close()
    conn2.close()


@pytest.fixture(scope="function")
def live_server(server_bin, test_db):
    """Start a Stage server against the test database and return its base URL."""
    port = _free_port()
    base_url = f"http://127.0.0.1:{port}"

    env = {
        **os.environ,
        "DATABASE_URL": test_db,
        "PORT": str(port),
        "BASE_URL": base_url,
        "JWT_SECRET": JWT_SECRET,
        "GITHUB_CLIENT_ID": GITHUB_CLIENT_ID,
        "GITHUB_CLIENT_SECRET": GITHUB_CLIENT_SECRET,
        "RUST_LOG": "error",
    }

    proc = subprocess.Popen(
        [str(server_bin)],
        env=env,
        cwd=str(REPO_ROOT),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    try:
        _wait_for_server(base_url)
    except RuntimeError:
        proc.terminate()
        proc.wait()
        raise

    yield base_url

    proc.terminate()
    proc.wait(timeout=5)


@pytest.fixture(scope="function")
def test_user(test_db):
    """
    Create a test user, org, project membership, and push API key directly
    in the database. Returns a dict with user_id, github_login, org_slug,
    api_key (raw), and jwt (for the session cookie).
    """
    conn = psycopg2.connect(test_db)
    conn.autocommit = True
    cur = conn.cursor()

    github_login = "testuser"
    org_slug = "testuser"
    org_name = "Test User"

    # Create org first (users.default_org_id references orgs)
    cur.execute(
        "INSERT INTO orgs (slug, name) VALUES (%s, %s) RETURNING id",
        (org_slug, org_name),
    )
    org_id = cur.fetchone()[0]

    # Create user
    cur.execute(
        "INSERT INTO users (github_id, github_login, default_org_id) VALUES (%s, %s, %s) RETURNING id",
        (99999, github_login, org_id),
    )
    user_id = cur.fetchone()[0]

    # Add to org as owner
    cur.execute(
        "INSERT INTO org_members (org_id, user_id, role) VALUES (%s, %s, 'owner')",
        (org_id, user_id),
    )

    # Create push API key
    raw_key = f"stage_sk_test_{user_id}_{'a' * 40}"
    key_hash = _hash_api_key(raw_key)
    cur.execute(
        "INSERT INTO api_keys (user_id, scope, name, key_hash) VALUES (%s, 'push', %s, %s)",
        (user_id, "e2e-push-key", key_hash),
    )

    cur.close()
    conn.close()

    jwt = _mint_jwt(user_id, github_login)

    return {
        "user_id": user_id,
        "github_login": github_login,
        "org_id": org_id,
        "org_slug": org_slug,
        "api_key": raw_key,
        "jwt": jwt,
    }


@pytest.fixture(scope="function")
def authed_page(live_server, test_user, page):
    """
    Return a Playwright page with the test user's session cookie set.
    The page is ready to navigate to any live_server URL as the test user.
    """
    page.context.add_cookies([{
        "name": "stage_session",
        "value": test_user["jwt"],
        "domain": "127.0.0.1",
        "path": "/",
    }])
    yield page


@pytest.fixture(scope="function")
def test_project(test_db, test_user, live_server):
    """
    Create a test project in the test user's org.
    Returns a dict with project_id, project_slug, org_slug.
    """
    import requests
    org_slug = test_user["org_slug"]
    project_slug = "testproject"

    resp = requests.post(
        f"{live_server}/v1/projects/{org_slug}/{project_slug}/runs",
        headers={"Authorization": f"Bearer {test_user['api_key']}"},
        json={
            "scenario": "plank.smoke",
            "world": "plank",
            "backend": "mock",
        },
    )
    # This will 404 since the project doesn't exist yet -- that's fine,
    # we create the project directly in the DB instead.
    conn = psycopg2.connect(test_db)
    conn.autocommit = True
    cur = conn.cursor()
    cur.execute(
        "INSERT INTO projects (org_id, slug, name, public) VALUES (%s, %s, %s, true) RETURNING id",
        (test_user["org_id"], project_slug, "Test Project"),
    )
    project_id = cur.fetchone()[0]
    cur.close()
    conn.close()

    return {
        "project_id": project_id,
        "project_slug": project_slug,
        "org_slug": org_slug,
    }


@pytest.fixture(scope="function")
def pushed_run(test_project, test_user, live_server):
    """
    Push a complete run to the test Stage server using the API.
    Returns the run_id.
    """
    import requests
    import uuid

    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    run_id = str(uuid.uuid4())

    headers = {"Authorization": f"Bearer {test_user['api_key']}"}

    # Create run
    resp = requests.post(
        f"{live_server}/v1/projects/{org}/{proj}/runs",
        headers=headers,
        json={
            "id": run_id,
            "scenario": "plank.refund_storm",
            "world": "plank",
            "backend": "mock",
        },
    )
    assert resp.status_code == 201, resp.text

    # Push some events
    events = [
        {
            "sequence_number": i,
            "kind": kind,
            "payload": payload,
            "event_id": str(uuid.uuid4()),
            "wall_time_ms": 1000 + i * 50,
        }
        for i, (kind, payload) in enumerate([
            ("system", {"actor": None, "message_id": None, "payload": {"kind": "system", "note": '{"kind":"user_spawned","actor_id":"alice","model":"user-model","persona":"frustrated_power_user","system_prompt":"You are alice.","hidden_state":{"mood":"annoyed","hidden_goal":"refund_3mo"}}'}, "tick": 0, "ts_ms": 1000}),
            ("user_message", {"actor": "alice", "message_id": "m1", "payload": {"kind": "user_message", "text": "I want my money back"}, "tick": 1, "ts_ms": 1100}),
            ("tool_call", {"actor": "rep1", "message_id": "m2", "payload": {"kind": "tool_call", "name": "lookup_user", "args": {"user_id": "u-alice"}}, "tick": 2, "ts_ms": 1200}),
            ("tool_result", {"actor": "rep1", "message_id": "m2", "payload": {"kind": "tool_result", "name": "lookup_user", "result": {"data": {"id": "u-alice", "plan": "team"}}}, "tick": 3, "ts_ms": 1300}),
            ("agent_message", {"actor": "rep1", "message_id": "m3", "payload": {"kind": "agent_message", "text": "I see your account. Let me check your billing."}, "tick": 4, "ts_ms": 1400}),
            ("system", {"actor": None, "message_id": None, "payload": {"kind": "system", "note": 'grader: {"kind":"grader","scenario":"plank.refund_storm","scores":{"alice_refund_resolved":0.0,"bob_no_unsolicited_upgrade":1.0}}'}, "tick": 5, "ts_ms": 1500}),
        ])
    ]
    resp = requests.post(
        f"{live_server}/v1/runs/{run_id}/events",
        headers=headers,
        json={"events": events},
    )
    assert resp.status_code == 200, resp.text

    # Complete the run with scores
    resp = requests.post(
        f"{live_server}/v1/runs/{run_id}/status",
        headers=headers,
        json={
            "status": "completed",
            "outcome": {"alice_refund_resolved": 0.0, "bob_no_unsolicited_upgrade": 1.0},
            "wall_time_ms": 2100,
        },
    )
    assert resp.status_code == 200, resp.text

    return run_id


@pytest.fixture(scope="function")
def pushed_sweep(test_project, test_user, live_server):
    """
    Push a completed sweep with two child runs to the test server.
    Returns the sweep_id.
    """
    import requests
    import uuid

    org = test_project["org_slug"]
    proj = test_project["project_slug"]
    headers = {"Authorization": f"Bearer {test_user['api_key']}"}

    # Create sweep
    resp = requests.post(
        f"{live_server}/v1/projects/{org}/{proj}/sweeps",
        headers=headers,
        json={"config": {"scenarios": ["plank.smoke"], "backends": ["mock", "anthropic"], "n_trials": 1}},
    )
    assert resp.status_code == 201, resp.text
    sweep_id = resp.json()["id"]

    # Create two child runs
    for backend in ["mock", "anthropic"]:
        run_id = str(uuid.uuid4())
        requests.post(
            f"{live_server}/v1/projects/{org}/{proj}/runs",
            headers=headers,
            json={"id": run_id, "scenario": "plank.smoke", "world": "plank", "backend": backend, "sweep_id": sweep_id},
        )
        requests.post(
            f"{live_server}/v1/runs/{run_id}/status",
            headers=headers,
            json={"status": "completed", "outcome": {"score": 0.8 if backend == "mock" else 0.9}, "wall_time_ms": 1000},
        )
        requests.post(
            f"{live_server}/v1/sweeps/{sweep_id}/runs",
            headers=headers,
            json={"run_id": run_id},
        )

    # Complete the sweep
    requests.post(
        f"{live_server}/v1/sweeps/{sweep_id}/status",
        headers=headers,
        json={"status": "completed"},
    )

    return sweep_id
