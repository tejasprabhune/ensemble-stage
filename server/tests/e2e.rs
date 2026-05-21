// End-to-end smoke test for the push-to-view path.
//
// Starts a test instance of the server against an isolated postgres database,
// creates a user and org directly in the database, mints an API key, then
// exercises the full sequence that the ensemble integration performs: create
// project, create run, transition to running, stream events, finalize with
// outcome and cost, then verify the read side returns what was written.
//
// If this test passes, ensemble's integration can target this server with
// confidence.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use stage_server::{app, auth::middleware::hash_api_key, Config};

async fn setup(pool: PgPool) -> (axum::Router, String, String) {
    let config = Config {
        database_url: String::new(),
        port: 0,
        base_url: "http://localhost".into(),
        github_client_id: String::new(),
        github_client_secret: String::new(),
        jwt_secret: "test-secret".into(),
    };

    let router = app(config, pool.clone());

    // Create org and user directly (bypassing OAuth)
    let org_id: i64 = sqlx::query_scalar(
        "INSERT INTO orgs (slug, name) VALUES ('testuser', 'testuser') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (github_id, github_login, email, default_org_id) VALUES (999, 'testuser', 'test@example.com', $1) RETURNING id",
    )
    .bind(org_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO org_members (org_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(org_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    // Create a project
    sqlx::query("INSERT INTO projects (org_id, slug, name, public) VALUES ($1, 'testproject', 'Test Project', true)")
        .bind(org_id)
        .execute(&pool)
        .await
        .unwrap();

    // Create a push-scoped API key
    let raw_key = format!("stage_sk_{}{}", Uuid::new_v4().as_simple(), Uuid::new_v4().as_simple());
    let key_hash = hash_api_key(&raw_key);
    sqlx::query(
        "INSERT INTO api_keys (user_id, scope, name, key_hash) VALUES ($1, 'push', 'smoke-test', $2)",
    )
    .bind(user_id)
    .bind(&key_hash)
    .execute(&pool)
    .await
    .unwrap();

    (router, raw_key, format!("http://localhost/testuser/testproject"))
}

async fn post_json(
    router: &axum::Router,
    path: &str,
    key: &str,
    body: Value,
) -> (StatusCode, Value) {
    let body_bytes = serde_json::to_vec(&body).unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", key))
        .body(Body::from(body_bytes))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get_json(router: &axum::Router, path: &str, key: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {}", key))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[sqlx::test(migrations = "../ops/migrations")]
async fn push_to_view(pool: PgPool) {
    let (router, api_key, _base_url) = setup(pool).await;

    // Step 1: Create a run.
    let (status, body) = post_json(
        &router,
        "/v1/projects/testuser/testproject/runs",
        &api_key,
        json!({
            "scenario": "popcorn.single_problem",
            "world": "popcorn",
            "backend": "claude-sonnet-4-5",
            "metadata": { "seed": 42 }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "create run: {body}");
    let run_id = body["id"].as_str().expect("id in response");
    let run_url = body["url"].as_str().expect("url in response");
    assert!(run_url.contains(run_id), "url should contain run id");
    let run_id = run_id.to_string();

    // Step 2: Transition to running.
    let (status, body) = post_json(
        &router,
        &format!("/v1/runs/{}/status", run_id),
        &api_key,
        json!({ "status": "running" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "transition to running: {body}");
    assert_eq!(body["ok"], true);

    // Step 3: Append 20 events covering all event kinds.
    let events = json!({
        "events": [
            { "sequence_number": 1, "kind": "system",
              "payload": { "note": "run started", "actor": "system" },
              "event_id": Uuid::new_v4(), "wall_time_ms": 0 },
            { "sequence_number": 2, "kind": "user_message",
              "payload": { "actor": "user", "kind": "user_message", "text": "Solve this." },
              "event_id": Uuid::new_v4(), "wall_time_ms": 10 },
            { "sequence_number": 3, "kind": "agent_message",
              "payload": { "actor": "agent:0", "kind": "agent_message", "text": "I will begin." },
              "event_id": Uuid::new_v4(), "wall_time_ms": 214 },
            { "sequence_number": 4, "kind": "tool_call",
              "payload": { "actor": "agent:0", "kind": "tool_call", "name": "bash", "args": { "command": "ls /tmp" }, "seed": false },
              "event_id": Uuid::new_v4(), "wall_time_ms": 300 },
            { "sequence_number": 5, "kind": "tool_result",
              "payload": { "actor": "agent:0", "kind": "tool_result", "name": "bash", "result": { "summary": "file1.txt" } },
              "event_id": Uuid::new_v4(), "wall_time_ms": 450 },
            { "sequence_number": 6, "kind": "state_diff",
              "payload": { "actor": "agent:0", "kind": "state_diff", "diff": { "table": "inventory", "field": "count", "old": 0, "new": 10 } },
              "event_id": Uuid::new_v4(), "wall_time_ms": 500 },
            { "sequence_number": 7, "kind": "cost",
              "payload": { "actor": "agent:0", "kind": "cost", "unit": "usd", "amount": 0.0012, "running_total": 0.0012 },
              "event_id": Uuid::new_v4(), "wall_time_ms": 510 },
            { "sequence_number": 8, "kind": "progress",
              "payload": { "actor": "agent:0", "kind": "progress", "tool": "compile", "fraction": 0.5, "message": "halfway" },
              "event_id": Uuid::new_v4(), "wall_time_ms": 600 },
            { "sequence_number": 9, "kind": "agent_message",
              "payload": { "actor": "agent:0", "kind": "agent_message", "text": "Step 9." },
              "event_id": Uuid::new_v4(), "wall_time_ms": 700 },
            { "sequence_number": 10, "kind": "tool_call",
              "payload": { "actor": "agent:0", "kind": "tool_call", "name": "read_file", "args": {}, "seed": false },
              "event_id": Uuid::new_v4(), "wall_time_ms": 800 },
            { "sequence_number": 11, "kind": "tool_result",
              "payload": { "actor": "agent:0", "kind": "tool_result", "name": "read_file", "result": {} },
              "event_id": Uuid::new_v4(), "wall_time_ms": 900 },
            { "sequence_number": 12, "kind": "state_diff",
              "payload": { "actor": "agent:0", "kind": "state_diff", "diff": {} },
              "event_id": Uuid::new_v4(), "wall_time_ms": 950 },
            { "sequence_number": 13, "kind": "cost",
              "payload": { "actor": "agent:0", "kind": "cost", "unit": "usd", "amount": 0.0008, "running_total": 0.002 },
              "event_id": Uuid::new_v4(), "wall_time_ms": 960 },
            { "sequence_number": 14, "kind": "progress",
              "payload": { "actor": "agent:0", "kind": "progress", "tool": "compile", "fraction": 0.9, "message": "almost done" },
              "event_id": Uuid::new_v4(), "wall_time_ms": 1000 },
            { "sequence_number": 15, "kind": "agent_message",
              "payload": { "actor": "agent:0", "kind": "agent_message", "text": "Done with step 15." },
              "event_id": Uuid::new_v4(), "wall_time_ms": 1100 },
            { "sequence_number": 16, "kind": "user_message",
              "payload": { "actor": "user", "kind": "user_message", "text": "Continue." },
              "event_id": Uuid::new_v4(), "wall_time_ms": 1200 },
            { "sequence_number": 17, "kind": "tool_call",
              "payload": { "actor": "agent:0", "kind": "tool_call", "name": "finalize", "args": {}, "seed": false },
              "event_id": Uuid::new_v4(), "wall_time_ms": 1300 },
            { "sequence_number": 18, "kind": "tool_result",
              "payload": { "actor": "agent:0", "kind": "tool_result", "name": "finalize", "result": { "ok": true } },
              "event_id": Uuid::new_v4(), "wall_time_ms": 1350 },
            { "sequence_number": 19, "kind": "cost",
              "payload": { "actor": "agent:0", "kind": "cost", "unit": "usd", "amount": 0.0003, "running_total": 0.0183 },
              "event_id": Uuid::new_v4(), "wall_time_ms": 1360 },
            { "sequence_number": 20, "kind": "system",
              "payload": { "note": "grader: {\"scenario\": \"popcorn\", \"scores\": {\"correctness\": 0.92}}", "actor": "system" },
              "event_id": Uuid::new_v4(), "wall_time_ms": 1400 }
        ]
    });

    let (status, body) = post_json(
        &router,
        &format!("/v1/runs/{}/events", run_id),
        &api_key,
        events,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "append events: {body}");
    assert_eq!(body["accepted"], 20, "all 20 events should be accepted");

    // Step 3b: Re-send the same events. All should be idempotent (accepted=0).
    let idempotency_event_id = Uuid::new_v4();
    let (status, body) = post_json(
        &router,
        &format!("/v1/runs/{}/events", run_id),
        &api_key,
        json!({
            "events": [{
                "sequence_number": 21,
                "kind": "system",
                "payload": { "note": "duplicate test" },
                "event_id": idempotency_event_id,
                "wall_time_ms": 1500
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "first insert of event 21: {body}");
    assert_eq!(body["accepted"], 1);

    let (status, body) = post_json(
        &router,
        &format!("/v1/runs/{}/events", run_id),
        &api_key,
        json!({
            "events": [{
                "sequence_number": 22,
                "kind": "system",
                "payload": { "note": "duplicate test again" },
                "event_id": idempotency_event_id,
                "wall_time_ms": 1600
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "duplicate event_id should be accepted silently: {body}");
    assert_eq!(body["accepted"], 0, "duplicate event_id should not be counted");

    // Step 4: Finalize with outcome and cost.
    let (status, body) = post_json(
        &router,
        &format!("/v1/runs/{}/status", run_id),
        &api_key,
        json!({
            "status": "completed",
            "outcome": { "scores": { "correctness": 0.92, "efficiency": 0.78 } },
            "total_cost": { "input_tokens": 12400, "output_tokens": 3200, "usd": 0.0183 },
            "wall_time_ms": 47000
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "finalize run: {body}");
    assert_eq!(body["ok"], true);

    // Step 5: Read the run back and verify.
    let (status, run) = get_json(&router, &format!("/v1/runs/{}", run_id), &api_key).await;
    assert_eq!(status, StatusCode::OK, "get run: {run}");
    assert_eq!(run["id"].as_str().unwrap(), run_id);
    assert_eq!(run["status"], "completed");
    assert_eq!(run["scenario"], "popcorn.single_problem");
    assert_eq!(run["world"], "popcorn");
    assert_eq!(run["backend"], "claude-sonnet-4-5");
    assert_eq!(run["wall_time_ms"], 47000);
    assert_eq!(run["outcome"]["scores"]["correctness"], 0.92);
    assert_eq!(run["total_cost"]["usd"], 0.0183);
    assert!(run["started_at"].is_string(), "started_at should be set");
    assert!(run["ended_at"].is_string(), "ended_at should be set");
    assert!(run["created_at"].is_string(), "created_at should be set");
    assert_eq!(run["metadata"]["seed"], 42);

    // Step 6: Fetch events and verify all 21 are present in order.
    let (status, events) = get_json(
        &router,
        &format!("/v1/runs/{}/events", run_id),
        &api_key,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "get events: {events}");
    let events = events.as_array().expect("events should be an array");
    assert_eq!(events.len(), 21, "should have 21 events (20 + 1 non-duplicate)");

    // Verify ascending sequence order
    for (i, event) in events.iter().enumerate() {
        let seq = event["sequence_number"].as_i64().unwrap();
        assert_eq!(seq, (i + 1) as i64, "events should be in ascending sequence order");
    }

    // Verify event kinds are present
    let kinds: Vec<&str> = events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"system"));
    assert!(kinds.contains(&"user_message"));
    assert!(kinds.contains(&"agent_message"));
    assert!(kinds.contains(&"tool_call"));
    assert!(kinds.contains(&"tool_result"));
    assert!(kinds.contains(&"state_diff"));
    assert!(kinds.contains(&"cost"));
    assert!(kinds.contains(&"progress"));

    // Step 7: Verify the since cursor works for live polling.
    let (status, partial) = get_json(
        &router,
        &format!("/v1/runs/{}/events?since=10", run_id),
        &api_key,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "events since 10: {partial}");
    let partial = partial.as_array().unwrap();
    assert_eq!(partial.len(), 11, "since=10 should return events 11-21");
    assert_eq!(partial[0]["sequence_number"], 11);

    // Step 8: Verify the run appears in the project's run list.
    let (status, list) = get_json(
        &router,
        "/v1/projects/testuser/testproject/runs",
        &api_key,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "list runs: {list}");
    let runs = list["runs"].as_array().unwrap();
    assert!(!runs.is_empty(), "project should have at least one run");
    let listed = runs.iter().find(|r| r["id"].as_str() == Some(&run_id));
    assert!(listed.is_some(), "our run should appear in the list");
    assert_eq!(listed.unwrap()["status"], "completed");

    eprintln!();
    eprintln!("Push-to-view smoke test passed.");
    eprintln!("  Run ID:   {}", run_id);
    eprintln!("  Status:   completed");
    eprintln!("  Events:   21 accepted, idempotency verified");
    eprintln!("  Outcome:  correctness=0.92, efficiency=0.78");
    eprintln!("  Cost:     $0.0183");
    eprintln!();
    eprintln!("The ensemble integration can target this server with confidence.");
}
