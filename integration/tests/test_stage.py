from unittest.mock import patch, MagicMock
import pytest
import responses as resp_lib

from ensemble_stage import Stage
from ensemble_stage.run import RunContext


def test_stage_reads_env():
    with patch.dict("os.environ", {"STAGE_API_KEY": "stage_sk_test", "STAGE_BASE_URL": "http://localhost:9999"}):
        s = Stage()
        assert s.api_key == "stage_sk_test"
        assert s.base_url == "http://localhost:9999"


def test_stage_constructor_overrides_env():
    with patch.dict("os.environ", {"STAGE_API_KEY": "stage_sk_env"}):
        s = Stage(api_key="stage_sk_arg", base_url="http://override:1234")
        assert s.api_key == "stage_sk_arg"
        assert s.base_url == "http://override:1234"


def test_run_returns_context():
    s = Stage(api_key="stage_sk_x", base_url="http://localhost:9999")
    ctx = s.run(
        project="org/proj",
        scenario="smoke.test",
        world="smoke",
        backend="test",
    )
    assert isinstance(ctx, RunContext)


def test_project_format_validation():
    s = Stage(api_key="stage_sk_x", base_url="http://localhost:9999")
    with pytest.raises(ValueError, match="org_slug/project_slug"):
        s.run(project="no-slash", scenario="x", world="x", backend="x")


@resp_lib.activate
def test_append_event_buffers():
    resp_lib.add(
        resp_lib.POST,
        "http://localhost:9999/v1/projects/org/proj/runs",
        json={"id": "019542a3-0000-7000-0000-000000000001", "url": "http://localhost:9999/org/proj/runs/019542a3-0000-7000-0000-000000000001"},
        status=201,
    )
    resp_lib.add(
        resp_lib.POST,
        "http://localhost:9999/v1/runs/019542a3-0000-7000-0000-000000000001/status",
        json={"ok": True},
        status=200,
    )
    resp_lib.add(
        resp_lib.POST,
        "http://localhost:9999/v1/runs/019542a3-0000-7000-0000-000000000001/events",
        json={"accepted": 1},
        status=200,
    )

    s = Stage(api_key="stage_sk_x", base_url="http://localhost:9999")
    with s.run(project="org/proj", scenario="s", world="w", backend="b") as run:
        assert run.id == "019542a3-0000-7000-0000-000000000001"
        run.append_event(1, "system", {"note": "test", "actor": "system"})
