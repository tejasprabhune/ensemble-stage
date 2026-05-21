from __future__ import annotations

import threading
import time
from typing import Any

import requests


_FLUSH_INTERVAL = 1.0
_FLUSH_BATCH = 100
_MAX_RETRIES = 3
_BACKOFF_BASE = 1.5


class RunContext:
    """Context manager for a single run.

    Created by Stage.run(); do not instantiate directly.

    Within the ``with`` block, call ``append_event`` to stream events. Events
    are buffered and flushed every second or when the buffer reaches 100 events,
    whichever comes first. On context exit the buffer is flushed and the run
    status is set to "completed" (or "failed" if the block raised an exception).

    ``run.id`` and ``run.url`` are available after the context is entered.
    """

    def __init__(
        self,
        session: requests.Session,
        base_url: str,
        org_slug: str,
        project_slug: str,
        scenario: str,
        world: str,
        backend: str,
        sweep_id: str | None,
        metadata: dict[str, Any],
    ) -> None:
        self._session = session
        self._base_url = base_url
        self._org_slug = org_slug
        self._project_slug = project_slug
        self._scenario = scenario
        self._world = world
        self._backend = backend
        self._sweep_id = sweep_id
        self._metadata = metadata

        self.id: str | None = None
        self.url: str | None = None

        self._buffer: list[dict[str, Any]] = []
        self._lock = threading.Lock()
        self._flush_thread: threading.Thread | None = None
        self._stop_flush = threading.Event()
        self._pending: list[dict[str, Any]] = []

    def __enter__(self) -> RunContext:
        resp = self._session.post(
            f"{self._base_url}/v1/projects/{self._org_slug}/{self._project_slug}/runs",
            json={
                "scenario": self._scenario,
                "world": self._world,
                "backend": self._backend,
                "sweep_id": self._sweep_id,
                "metadata": self._metadata,
            },
        )
        resp.raise_for_status()
        data = resp.json()
        self.id = data["id"]
        self.url = data["url"]

        self._session.post(
            f"{self._base_url}/v1/runs/{self.id}/status",
            json={"status": "running"},
        )

        self._stop_flush.clear()
        self._flush_thread = threading.Thread(target=self._flush_loop, daemon=True)
        self._flush_thread.start()

        return self

    def __exit__(
        self,
        exc_type: type | None,
        exc_val: BaseException | None,
        exc_tb: object,
    ) -> None:
        self._stop_flush.set()
        if self._flush_thread:
            self._flush_thread.join(timeout=10)
        self._flush_once()

        status = "failed" if exc_type is not None else "completed"
        try:
            self._session.post(
                f"{self._base_url}/v1/runs/{self.id}/status",
                json={"status": status},
            )
        except Exception:
            pass

    def append_event(
        self,
        sequence_number: int,
        kind: str,
        payload: dict[str, Any],
        wall_time_ms: int | None = None,
    ) -> None:
        """Buffer one event for streaming to Stage."""
        import uuid as _uuid

        event = {
            "sequence_number": sequence_number,
            "kind": kind,
            "payload": payload,
            "event_id": str(_uuid.uuid4()),
            "wall_time_ms": wall_time_ms,
        }
        with self._lock:
            self._buffer.append(event)
            if len(self._buffer) >= _FLUSH_BATCH:
                self._drain_buffer()

    def _drain_buffer(self) -> list[dict[str, Any]]:
        """Move buffered events out under the lock. Caller must hold the lock."""
        batch = self._buffer[:]
        self._buffer.clear()
        return batch

    def _flush_loop(self) -> None:
        while not self._stop_flush.wait(timeout=_FLUSH_INTERVAL):
            self._flush_once()

    def _flush_once(self) -> None:
        with self._lock:
            batch = self._drain_buffer()
        if not batch:
            return
        self._push_with_retry(batch)

    def _push_with_retry(self, events: list[dict[str, Any]]) -> None:
        for attempt in range(_MAX_RETRIES):
            try:
                resp = self._session.post(
                    f"{self._base_url}/v1/runs/{self.id}/events",
                    json={"events": events},
                )
                if resp.status_code < 500:
                    return
            except requests.RequestException:
                pass

            if attempt < _MAX_RETRIES - 1:
                delay = _BACKOFF_BASE ** attempt
                time.sleep(delay)

        # Push failed after retries; mark events as pending for later replay.
        with self._lock:
            self._pending.extend(events)
