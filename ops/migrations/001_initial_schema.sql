-- Initial schema for Stage.
-- Run via: sqlx migrate run --source ops/migrations

-- Enum types

CREATE TYPE org_role AS ENUM ('owner', 'admin', 'member');
CREATE TYPE api_key_scope AS ENUM ('push', 'admin');
CREATE TYPE run_status AS ENUM ('queued', 'running', 'completed', 'failed', 'cancelled');
CREATE TYPE sweep_status AS ENUM ('running', 'completed', 'failed', 'cancelled');
CREATE TYPE training_run_status AS ENUM ('running', 'completed', 'failed', 'cancelled');
CREATE TYPE artifact_kind AS ENUM ('adapter', 'dataset', 'trace', 'baseline');

-- Orgs

CREATE TABLE orgs (
    id             BIGSERIAL PRIMARY KEY,
    slug           TEXT NOT NULL UNIQUE,
    name           TEXT NOT NULL,
    github_org_id  BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Users

CREATE TABLE users (
    id             BIGSERIAL PRIMARY KEY,
    github_id      BIGINT NOT NULL UNIQUE,
    github_login   TEXT NOT NULL,
    email          TEXT,
    default_org_id BIGINT NOT NULL DEFAULT 0,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE users ADD CONSTRAINT fk_users_default_org
    FOREIGN KEY (default_org_id) REFERENCES orgs(id)
    DEFERRABLE INITIALLY DEFERRED;

-- Org members

CREATE TABLE org_members (
    org_id   BIGINT NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    user_id  BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role     org_role NOT NULL DEFAULT 'member',
    PRIMARY KEY (org_id, user_id)
);

-- Projects

CREATE TABLE projects (
    id          BIGSERIAL PRIMARY KEY,
    org_id      BIGINT NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    slug        TEXT NOT NULL,
    name        TEXT NOT NULL,
    public      BOOLEAN NOT NULL DEFAULT TRUE,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, slug)
);

-- API keys

CREATE TABLE api_keys (
    id           BIGSERIAL PRIMARY KEY,
    user_id      BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope        api_key_scope NOT NULL DEFAULT 'push',
    name         TEXT NOT NULL,
    key_hash     TEXT NOT NULL UNIQUE,
    last_used_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at   TIMESTAMPTZ
);

-- Share tokens

CREATE TABLE share_tokens (
    id                 BIGSERIAL PRIMARY KEY,
    project_id         BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    token_hash         TEXT NOT NULL UNIQUE,
    name               TEXT NOT NULL,
    expires_at         TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by_user_id BIGINT NOT NULL REFERENCES users(id)
);

-- Sweeps (before runs because runs references sweeps)

CREATE TABLE sweeps (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id         BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    config             JSONB NOT NULL DEFAULT '{}',
    status             sweep_status NOT NULL DEFAULT 'running',
    started_at         TIMESTAMPTZ,
    ended_at           TIMESTAMPTZ,
    created_by_user_id BIGINT NOT NULL REFERENCES users(id),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Runs

CREATE TABLE runs (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id         BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sweep_id           UUID REFERENCES sweeps(id) ON DELETE SET NULL,
    scenario           TEXT NOT NULL,
    world              TEXT NOT NULL,
    backend            TEXT NOT NULL,
    status             run_status NOT NULL DEFAULT 'queued',
    outcome            JSONB,
    wall_time_ms       BIGINT,
    total_cost         JSONB,
    started_at         TIMESTAMPTZ,
    ended_at           TIMESTAMPTZ,
    created_by_user_id BIGINT NOT NULL REFERENCES users(id),
    metadata           JSONB,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Run events

CREATE TABLE run_events (
    run_id          UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    sequence_number BIGINT NOT NULL,
    kind            TEXT NOT NULL,
    payload         JSONB NOT NULL DEFAULT '{}',
    event_id        UUID NOT NULL,
    wall_time_ms    BIGINT,
    PRIMARY KEY (run_id, sequence_number)
);

CREATE UNIQUE INDEX run_events_event_id ON run_events (run_id, event_id);

-- Training runs

CREATE TABLE training_runs (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id         BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    persona_name       TEXT NOT NULL,
    base_model         TEXT NOT NULL,
    status             training_run_status NOT NULL DEFAULT 'running',
    hyperparameters    JSONB,
    final_metrics      JSONB,
    artifact_uri       TEXT,
    started_at         TIMESTAMPTZ,
    ended_at           TIMESTAMPTZ,
    created_by_user_id BIGINT NOT NULL REFERENCES users(id),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Training metrics

CREATE TABLE training_metrics (
    training_run_id UUID NOT NULL REFERENCES training_runs(id) ON DELETE CASCADE,
    step            BIGINT NOT NULL,
    metric_name     TEXT NOT NULL,
    value           DOUBLE PRECISION NOT NULL,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (training_run_id, step, metric_name)
);

-- Artifacts

CREATE TABLE artifacts (
    id                BIGSERIAL PRIMARY KEY,
    project_id        BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind              artifact_kind NOT NULL,
    uri               TEXT NOT NULL,
    size_bytes        BIGINT,
    created_by_run_id UUID,
    metadata          JSONB,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Comparisons

CREATE TABLE comparisons (
    id                 BIGSERIAL PRIMARY KEY,
    project_id         BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name               TEXT NOT NULL,
    run_ids            UUID[] NOT NULL DEFAULT '{}',
    view_state         JSONB,
    created_by_user_id BIGINT NOT NULL REFERENCES users(id),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes: list queries, foreign keys, run events lookup

CREATE INDEX idx_projects_org           ON projects (org_id, created_at DESC);
CREATE INDEX idx_runs_project           ON runs (project_id, created_at DESC);
CREATE INDEX idx_runs_sweep             ON runs (sweep_id) WHERE sweep_id IS NOT NULL;
CREATE INDEX idx_run_events_run         ON run_events (run_id, sequence_number);
CREATE INDEX idx_sweeps_project         ON sweeps (project_id, created_at DESC);
CREATE INDEX idx_training_runs_project  ON training_runs (project_id, created_at DESC);
CREATE INDEX idx_training_metrics_run   ON training_metrics (training_run_id, step);
CREATE INDEX idx_artifacts_project      ON artifacts (project_id, created_at DESC);
CREATE INDEX idx_comparisons_project    ON comparisons (project_id, created_at DESC);
CREATE INDEX idx_api_keys_user          ON api_keys (user_id);
CREATE INDEX idx_share_tokens_project   ON share_tokens (project_id);
