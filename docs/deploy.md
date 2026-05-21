# Deployment

Stage runs on [Fly.io](https://fly.io) with a Fly Postgres database. This document covers first-time setup, day-to-day deploy operations, secret rotation, migration execution, backups, and access management.

## Prerequisites

Install `flyctl` and authenticate:

```
brew install flyctl
fly auth login
```

The GitHub Actions deploy workflow uses a `FLY_API_TOKEN` secret. Generate one with `fly tokens create deploy -x 999999h` and store it in the GitHub repository's secrets under `Settings > Secrets and variables > Actions`.

## First-time setup

Run these commands once when provisioning the app. After that, deploys happen automatically via GitHub Actions on push to `main`.

**Create the Fly app:**

```
fly apps create ensemble-stage
```

**Attach a Postgres cluster:**

```
fly postgres create \
  --name ensemble-stage-db \
  --region ord \
  --initial-cluster-size 1 \
  --vm-size shared-cpu-1x \
  --volume-size 10

fly postgres attach --app ensemble-stage ensemble-stage-db
```

`attach` injects `DATABASE_URL` as a secret on the app automatically.

**Set the remaining secrets:**

```
fly secrets set \
  GITHUB_CLIENT_ID=<your_github_app_client_id> \
  GITHUB_CLIENT_SECRET=<your_github_app_client_secret> \
  JWT_SECRET=$(openssl rand -hex 32) \
  BASE_URL=https://ensemble-stage.fly.dev
```

`JWT_SECRET` signs session cookies. Generate it fresh; it does not need to match anything external. If you rotate it, all existing sessions are invalidated.

**Run migrations:**

The runtime image is minimal (no `sqlx` binary), so migrations are applied by proxying the Fly Postgres port locally and running the SQL with a local `psql`:

```bash
fly proxy 5454:5432 --app ensemble-stage-db &
PGPASSWORD=<db_password> psql -h localhost -p 5454 \
  -U ensemble_stage -d ensemble_stage \
  -f ops/migrations/001_initial_schema.sql
kill %1
```

The database password is in the `DATABASE_URL` secret (`fly secrets list --app ensemble-stage`). For subsequent migrations, increment the file number and run the new file only.

The CI workflow intentionally does not run migrations automatically; schema changes are applied by a human before the new binary starts serving traffic.

**First deploy:**

```
fly deploy --config ops/fly.toml --remote-only
```

After this, GitHub Actions handles subsequent deploys.

## Routine deploys

Push to `main`. The GitHub Actions workflow at `.github/workflows/deploy.yml` runs after CI passes and calls `flyctl deploy`. Monitor progress in the Actions tab.

If CI is broken and you need to force a deploy:

```
fly deploy --config ops/fly.toml --remote-only
```

## Adding a migration

Put the new SQL file in `ops/migrations/` with the next sequence number (e.g. `002_add_foo.sql`). The migration runs via sqlx's migration framework.

After deploying the new binary, run:

```
fly ssh console --app ensemble-stage \
  -C "sqlx migrate run --source ops/migrations"
```

Run migrations before restarting traffic on the new binary. If the migration is backward-compatible (adding a column with a default, adding an index), you can deploy the binary first and migrate after. If it is not backward-compatible, use a maintenance window.

## Secret rotation

### GitHub OAuth credentials

1. Go to your GitHub OAuth App settings and generate a new client secret.
2. Update the Fly secret:
   ```
   fly secrets set GITHUB_CLIENT_ID=<id> GITHUB_CLIENT_SECRET=<new_secret>
   ```
3. Fly restarts the app automatically. The old secret is invalidated immediately.

### JWT session secret

Rotating `JWT_SECRET` invalidates all existing user sessions; everyone will need to sign in again.

```
fly secrets set JWT_SECRET=$(openssl rand -hex 32)
```

If you want a grace period (old sessions remain valid temporarily), you would need to support two secrets in the JWT verification code. The current implementation does not do this.

### API keys

API key values are not recoverable from the server (only the SHA-256 hash is stored). Users revoke and re-create keys from `/me`. There is no server-side bulk revocation endpoint in v1; if a key is compromised, have the user revoke it from the UI.

## Postgres backups

Fly Postgres includes daily automated snapshots retained for 7 days. For a cross-region replica that also serves as a warm backup:

**Create a read replica in a different region:**

```
fly postgres create \
  --name ensemble-stage-db-replica \
  --region iad \
  --fork-from ensemble-stage-db \
  --initial-cluster-size 1 \
  --vm-size shared-cpu-1x \
  --volume-size 10
```

The replica streams WAL from the primary. If the primary fails, promote the replica:

```
fly postgres failover --app ensemble-stage-db
```

Fly's Postgres UI in the dashboard shows backup status. You can also list backups manually:

```
fly postgres backups list --app ensemble-stage-db
```

**Restore from a backup:**

```
# List available backups to get the backup name.
fly postgres backups list --app ensemble-stage-db

# Restore into a new Postgres cluster (not in place, to avoid data loss).
fly postgres restore <backup_name> \
  --app ensemble-stage-db-restore \
  --region ord

# Verify the restore, then repoint the app.
fly secrets set DATABASE_URL=<connection_string_of_restored_cluster> \
  --app ensemble-stage
```

Test the restored database before cutting over. After cutover, delete the old cluster if the restore is permanent.

## Adding a team member as a deployer

Grant Fly access:

```
fly orgs invite <email>
```

The invited member can then run `fly auth login` and will have access to the `ensemble-stage` app. They do not automatically get the `FLY_API_TOKEN` used by CI; that token lives in GitHub Secrets and is scoped to deployments.

To let them trigger GitHub Actions deploys from their fork or to create their own deploy tokens, share the deploy token generation command:

```
fly tokens create deploy --app ensemble-stage -x 999999h
```

Store the token in GitHub as `FLY_API_TOKEN`.

## Environment variables reference

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | yes | Postgres connection string. Set automatically by `fly postgres attach`. |
| `GITHUB_CLIENT_ID` | yes | GitHub OAuth App client ID. |
| `GITHUB_CLIENT_SECRET` | yes | GitHub OAuth App client secret. |
| `JWT_SECRET` | yes | 32+ bytes of random hex. Signs session cookies. |
| `BASE_URL` | yes | Public URL of the app, e.g. `https://ensemble-stage.fly.dev`. Used to construct the OAuth callback URL. |
| `PORT` | no | Listening port. Defaults to 3000 in development, 8080 in the Docker image. |
| `RUST_LOG` | no | Log filter. Default `info,stage_server=info`. Use `debug` for verbose output. |

## Path to Azure Postgres

Fly Postgres is a self-managed Postgres cluster running on Fly VMs, not a managed cloud database service. When the project outgrows it (larger storage, read replicas in multiple regions, point-in-time recovery), migrate to Azure Database for PostgreSQL Flexible Server.

The migration path: export data with `pg_dump`, provision the Azure instance, import with `psql`, test against a staging environment, then update `DATABASE_URL`. The application code has no Fly-specific dependencies; the only change is the connection string.
