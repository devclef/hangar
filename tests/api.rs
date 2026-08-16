//! End-to-end API tests: real axum router + real SQLite (in-memory),
//! exercising handlers, service rules, and the model<->part association.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use hangar::service::Service;
use hangar::AppState;
use std::sync::Arc;
use tower::ServiceExt;

async fn app() -> axum::Router {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let state = AppState {
        service: Arc::new(Service::from_sqlite(pool)),
        static_dir: None,
    };
    hangar::router(state)
}

struct Res {
    status: StatusCode,
    body: serde_json::Value,
}

async fn call(
    app: axum::Router,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> Res {
    let body_bytes = body.map(|b| b.to_string()).unwrap_or_default();
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body_bytes))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::String(
            String::from_utf8_lossy(&bytes).into_owned(),
        ))
    };
    Res { status, body }
}

fn model_json(name: &str, category: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "category": category,
        "manufacturer": "FooRC",
        "notes": "test model",
        "date_acquired": "2026-01-05",
        "status": "active"
    })
}

fn part_json(name: &str, quantity: i64) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "quantity": quantity,
        "notes": "test part",
        "link": "https://example.com/sku-1"
    })
}

fn id(v: &serde_json::Value) -> i64 {
    v["id"].as_i64().expect("id in response")
}

#[tokio::test]
async fn health_ok() {
    let app = app().await;
    let res = call(app, Method::GET, "/api/health", None).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["status"], "ok");
}

#[tokio::test]
async fn model_crud_lifecycle() {
    let app = app().await;

    // create
    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let model_id = id(&res.body);
    assert_eq!(res.body["category"], "heli");
    assert_eq!(res.body["status"], "active");

    // list
    let res = call(app.clone(), Method::GET, "/api/models", None).await;
    assert_eq!(res.status, StatusCode::OK);
    let list = res.body.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["part_count"], 0);

    // detail
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/models/{model_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["model"]["name"], "Kraken 580");
    assert!(res.body["parts"].as_array().unwrap().is_empty());

    // update
    let mut updated = model_json("Kraken 580 V2", "heli");
    updated["status"] = "retired".into();
    let res = call(
        app.clone(),
        Method::PUT,
        &format!("/api/models/{model_id}"),
        Some(updated),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["status"], "retired");
    assert_eq!(res.body["name"], "Kraken 580 V2");

    // delete
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/models/{model_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/models/{model_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.body["error"], "not_found");

    // delete again -> 404
    let res = call(
        app,
        Method::DELETE,
        &format!("/api/models/{model_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn model_validation_errors() {
    let app = app().await;

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(serde_json::json!({"name": "  ", "category": "heli"})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body["error"], "invalid_request");

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(serde_json::json!({"name": "X", "category": "spaceship"})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(serde_json::json!({"name": "X", "category": "car", "date_acquired": "2026-02-30"})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(serde_json::json!({"name": "X", "category": "car", "status": "flying"})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // default status is active when omitted
    let res = call(
        app,
        Method::POST,
        "/api/models",
        Some(serde_json::json!({"name": "No Status", "category": "car"})),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.body["status"], "active");
}

#[tokio::test]
async fn model_filters() {
    let app = app().await;
    for (name, cat) in [
        ("Alpha Heli", "heli"),
        ("Beta Plane", "plane"),
        ("Gamma Heli", "heli"),
    ] {
        let res = call(
            app.clone(),
            Method::POST,
            "/api/models",
            Some(model_json(name, cat)),
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED);
    }

    // category filter
    let res = call(app.clone(), Method::GET, "/api/models?category=heli", None).await;
    let list = res.body.as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|m| m["category"] == "heli"));

    // name search (case-insensitive)
    let res = call(app.clone(), Method::GET, "/api/models?q=ALPHA", None).await;
    let list = res.body.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "Alpha Heli");

    // search hits manufacturer too
    let res = call(app, Method::GET, "/api/models?q=foorc", None).await;
    assert_eq!(res.body.as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn part_crud_and_quantity_math() {
    let app = app().await;

    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Main rotor blades", 5)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let part_id = id(&res.body);
    assert_eq!(res.body["quantity"], 5);

    // negative quantity rejected
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Bad", -1)),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // absolute set
    let res = call(
        app.clone(),
        Method::PUT,
        &format!("/api/parts/{part_id}"),
        Some(part_json("Main rotor blades", 2)),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["quantity"], 2);

    // relative adjust: down, then clamp at 0
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/quantity"),
        Some(serde_json::json!({"delta": -1})),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["quantity"], 1);

    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/quantity"),
        Some(serde_json::json!({"delta": -100})),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["quantity"], 0, "quantity clamps at zero");

    // delta of zero is invalid
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/quantity"),
        Some(serde_json::json!({"delta": 0})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // back up
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/quantity"),
        Some(serde_json::json!({"delta": 7})),
    )
    .await;
    assert_eq!(res.body["quantity"], 7);

    // unknown part
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/9999/quantity",
        Some(serde_json::json!({"delta": 1})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // delete
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/parts/{part_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(app, Method::GET, &format!("/api/parts/{part_id}"), None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn part_list_search_and_sort() {
    let app = app().await;
    let mut ids = Vec::new();
    for (name, qty) in [
        ("Blade set C", 5),
        ("Blade set A", 0),
        ("ESC 60A", 2),
        ("Skid set", 1),
    ] {
        let res = call(
            app.clone(),
            Method::POST,
            "/api/parts",
            Some(part_json(name, qty)),
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED);
        ids.push(id(&res.body));
    }

    // quantity ascending surfaces out-of-stock first
    let res = call(
        app.clone(),
        Method::GET,
        "/api/parts?sort=quantity_asc",
        None,
    )
    .await;
    let list = res.body.as_array().unwrap();
    let qtys: Vec<i64> = list
        .iter()
        .map(|p| p["quantity"].as_i64().unwrap())
        .collect();
    assert_eq!(qtys, vec![0, 1, 2, 5]);

    // name descending
    let res = call(app.clone(), Method::GET, "/api/parts?sort=name_desc", None).await;
    let names: Vec<&str> = res
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["Skid set", "ESC 60A", "Blade set C", "Blade set A"]
    );

    // invalid sort -> 400
    let res = call(app.clone(), Method::GET, "/api/parts?sort=bogus", None).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // search by name
    let res = call(app.clone(), Method::GET, "/api/parts?q=blade", None).await;
    assert_eq!(res.body.as_array().unwrap().len(), 2);

    // search reaches the link field too
    let res = call(app.clone(), Method::GET, "/api/parts?q=sku-1", None).await;
    let list = res.body.as_array().unwrap();
    assert_eq!(list.len(), 4);

    // a search matching nothing comes back empty
    let res = call(app, Method::GET, "/api/parts?q=nosuchpart", None).await;
    assert!(res.body.as_array().unwrap().is_empty());

    // ids used above keep lints quiet
    assert!(!ids.is_empty());
}

#[tokio::test]
async fn association_link_unlink_replace() {
    let app = app().await;

    let m = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    let model_id = id(&m.body);
    let p1 = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Main rotor blade set", 2)),
    )
    .await;
    let part1 = id(&p1.body);
    let p2 = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Tail rotor blades", 4)),
    )
    .await;
    let part2 = id(&p2.body);
    let p3 = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Canopy", 1)),
    )
    .await;
    let part3 = id(&p3.body);

    // link one
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_id}/parts"),
        Some(serde_json::json!({"part_id": part1})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    // duplicate link is an idempotent success
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_id}/parts"),
        Some(serde_json::json!({"part_id": part1})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    // link another; the model detail now shows both, in quantity
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_id}/parts"),
        Some(serde_json::json!({"part_id": part2})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/models/{model_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let parts = res.body["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    let by_name: std::collections::HashMap<&str, i64> = parts
        .iter()
        .map(|p| (p["name"].as_str().unwrap(), p["quantity"].as_i64().unwrap()))
        .collect();
    assert_eq!(
        by_name["Main rotor blade set"], 2,
        "linked part carries its quantity"
    );
    assert_eq!(by_name["Tail rotor blades"], 4);

    // link to nonexistent part -> 404
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_id}/parts"),
        Some(serde_json::json!({"part_id": 424242})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.body["error"], "not_found");

    // replace with [p2, p3]: p1 drops, p3 joins; dupes in input collapse
    let res = call(
        app.clone(),
        Method::PUT,
        &format!("/api/models/{model_id}/parts"),
        Some(serde_json::json!({"part_ids": [part3, part2, part3]})),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let ids: Vec<i64> = res
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&part2) && ids.contains(&part3));

    // replace with a nonexistent part -> 404 and the set is unchanged
    let res = call(
        app.clone(),
        Method::PUT,
        &format!("/api/models/{model_id}/parts"),
        Some(serde_json::json!({"part_ids": [part1, 424242]})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/models/{model_id}/parts"),
        None,
    )
    .await;
    assert_eq!(res.body.as_array().unwrap().len(), 2);

    // unlink
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/models/{model_id}/parts/{part2}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    // unlink again -> 404
    let res = call(
        app,
        Method::DELETE,
        &format!("/api/models/{model_id}/parts/{part2}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn part_reverse_links_and_cascade() {
    let app = app().await;

    let m1 = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    let m1_id = id(&m1.body);
    let m2 = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("S500", "heli")),
    )
    .await;
    let m2_id = id(&m2.body);
    let p = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Shared blades", 9)),
    )
    .await;
    let p_id = id(&p.body);

    // link the part to two models via the part-side endpoint
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{p_id}/models"),
        Some(serde_json::json!({"model_id": m1_id})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{p_id}/models"),
        Some(serde_json::json!({"model_id": m2_id})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    // part detail lists both models
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{p_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["part"]["quantity"], 9);
    let model_ids: Vec<i64> = res.body["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_i64().unwrap())
        .collect();
    assert!(model_ids.contains(&m1_id) && model_ids.contains(&m2_id));

    // part list row carries model summary
    let res = call(app.clone(), Method::GET, "/api/parts", None).await;
    let row = &res.body.as_array().unwrap()[0];
    assert_eq!(row["model_count"], 2);
    assert!(row["model_names"].as_str().unwrap().contains("Kraken 580"));

    // unlink one side
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/parts/{p_id}/models/{m2_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{p_id}/models"),
        None,
    )
    .await;
    assert_eq!(res.body.as_array().unwrap().len(), 1);

    // deleting a model cascades: link survives nowhere, part survives
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/models/{m1_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{p_id}/models"),
        None,
    )
    .await;
    assert!(res.body.as_array().unwrap().is_empty());
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{p_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);

    // deleting the part removes it everywhere
    let res = call(app, Method::DELETE, &format!("/api/parts/{p_id}"), None).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn unknown_api_route_returns_json_404() {
    let app = app().await;
    let res = call(app, Method::GET, "/api/nope/nothing", None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.body["error"], "not_found");
}

#[tokio::test]
async fn part_cost_and_vendor_roundtrip() {
    let app = app().await;

    let mut body = part_json("Main rotor blades", 2);
    body["cost"] = serde_json::json!(45.99);
    body["vendor"] = "Heli-Flex".into();
    let res = call(app.clone(), Method::POST, "/api/parts", Some(body)).await;
    assert_eq!(res.status, StatusCode::CREATED);
    let part_id = id(&res.body);
    assert_eq!(res.body["cost"], 45.99);
    assert_eq!(res.body["vendor"], "Heli-Flex");

    // list rows carry the new fields too
    let res = call(app.clone(), Method::GET, "/api/parts", None).await;
    let row = &res.body.as_array().unwrap()[0];
    assert_eq!(row["cost"], 45.99);
    assert_eq!(row["vendor"], "Heli-Flex");

    // full-replace update without the fields clears them
    let res = call(
        app.clone(),
        Method::PUT,
        &format!("/api/parts/{part_id}"),
        Some(part_json("Main rotor blades", 3)),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["cost"], serde_json::Value::Null);
    assert_eq!(res.body["vendor"], serde_json::Value::Null);

    // negative cost is rejected
    let mut bad = part_json("Bad", 1);
    bad["cost"] = serde_json::json!(-5.0);
    let res = call(app.clone(), Method::POST, "/api/parts", Some(bad)).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // a string where a number is expected is rejected
    let mut bad = part_json("Bad", 1);
    bad["cost"] = serde_json::json!("12.50");
    let res = call(app, Method::POST, "/api/parts", Some(bad)).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn settings_default_and_update() {
    let app = app().await;

    // first read returns the built-in defaults
    let res = call(app.clone(), Method::GET, "/api/settings", None).await;
    assert_eq!(res.status, StatusCode::OK);
    let fields: Vec<&str> = res.body["part_form_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        fields,
        vec!["quantity", "cost", "vendor", "link", "photo_url", "notes"]
    );
    assert_eq!(res.body["currency"], "USD");

    // update: hide some fields, change currency (normalized, dupes collapsed)
    let res = call(
        app.clone(),
        Method::PUT,
        "/api/settings",
        Some(serde_json::json!({
            "part_form_fields": ["quantity", "cost", "cost"],
            "currency": "  eur "
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["currency"], "EUR");
    let fields: Vec<&str> = res.body["part_form_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(fields, vec!["quantity", "cost"]);

    // the update is persisted
    let res = call(app, Method::GET, "/api/settings", None).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["currency"], "EUR");
    assert_eq!(res.body["part_form_fields"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn settings_validation_errors() {
    let app = app().await;

    // unknown field keys are rejected
    let res = call(
        app.clone(),
        Method::PUT,
        "/api/settings",
        Some(serde_json::json!({
            "part_form_fields": ["quantity", "bogus_field"],
            "currency": "USD"
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body["error"], "invalid_request");

    // invalid currency codes are rejected
    let res = call(
        app.clone(),
        Method::PUT,
        "/api/settings",
        Some(serde_json::json!({
            "part_form_fields": [],
            "currency": "U.S!"
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // a missing required key is a deserialization failure -> 400
    let res = call(
        app,
        Method::PUT,
        "/api/settings",
        Some(serde_json::json!({ "part_form_fields": [] })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invalid_route_param_is_400() {
    let app = app().await;
    let res = call(app, Method::GET, "/api/models/not-a-number", None).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

/// Regression: the pool must actually attach to the on-disk file, not
/// silently fall back to an in-memory database (e.g. if the URL were dropped).
#[tokio::test]
async fn database_file_is_created_on_disk() {
    let dir = std::env::temp_dir().join(format!(
        "hangar-api-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("hangar.db");
    let url = format!("sqlite://{}?mode=rwc", db_path.display());

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let state = AppState {
        service: Arc::new(Service::from_sqlite(pool)),
        static_dir: None,
    };
    let app = hangar::router(state);

    let res = call(
        app,
        Method::POST,
        "/api/models",
        Some(model_json("Disk Model", "boat")),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);

    assert!(
        db_path.exists(),
        "sqlite database file must exist at {}",
        db_path.display()
    );
    assert!(
        std::fs::metadata(&db_path).unwrap().len() > 0,
        "database file must not be empty"
    );

    drop(res);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Part usage log
// ---------------------------------------------------------------------------

fn usage_part_body(model_id: i64, quantity: i64) -> serde_json::Value {
    serde_json::json!({
        "model_id": model_id,
        "quantity": quantity,
        "notes": "replaced during repair"
    })
}

fn usage_model_body(part_id: i64, quantity: i64) -> serde_json::Value {
    serde_json::json!({
        "part_id": part_id,
        "quantity": quantity
    })
}

#[tokio::test]
async fn usage_lifecycle_and_stock_decrement() {
    let app = app().await;

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let model_id = id(&res.body);

    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Main rotor blades", 5)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let part_id = id(&res.body);

    // record a usage (part-scoped)
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/usage"),
        Some(usage_part_body(model_id, 2)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let usage_id = id(&res.body);
    assert_eq!(res.body["part_name"], "Main rotor blades");
    assert_eq!(res.body["model_name"], "Kraken 580");
    assert_eq!(res.body["model_category"], "heli");
    assert_eq!(res.body["quantity"], 2);
    assert_eq!(res.body["notes"], "replaced during repair");
    assert!(res.body["used_at"].is_string());

    // stock was decremented by the logged quantity
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{part_id}"),
        None,
    )
    .await;
    assert_eq!(res.body["part"]["quantity"], 3);

    // global log
    let res = call(app.clone(), Method::GET, "/api/usage", None).await;
    assert_eq!(res.status, StatusCode::OK);
    let list = res.body.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], usage_id);

    // filters
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/usage?part_id={part_id}"),
        None,
    )
    .await;
    assert_eq!(res.body.as_array().unwrap().len(), 1);

    let res = call(app.clone(), Method::GET, "/api/usage?part_id=9999", None).await;
    assert_eq!(res.body.as_array().unwrap().len(), 0);

    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/usage?part_id={part_id}&model_id={model_id}"),
        None,
    )
    .await;
    assert_eq!(res.body.as_array().unwrap().len(), 1);

    let res = call(app.clone(), Method::GET, "/api/usage?model_id=9999", None).await;
    assert_eq!(res.body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn usage_validation_and_errors() {
    let app = app().await;

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    let model_id = id(&res.body);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Main rotor blades", 5)),
    )
    .await;
    let part_id = id(&res.body);

    // quantity must be positive
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/usage"),
        Some(usage_part_body(model_id, 0)),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // unknown model / unknown part
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/usage"),
        Some(usage_part_body(9999, 1)),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/9999/usage",
        Some(usage_part_body(model_id, 1)),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // bad backdate
    let bad_date = serde_json::json!({
        "model_id": model_id,
        "quantity": 1,
        "used_at": "2026-02-30"
    });
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/usage"),
        Some(bad_date),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // date-only backdate round-trips; space separator is normalized to 'T'
    let ok_date = serde_json::json!({
        "model_id": model_id,
        "quantity": 1,
        "used_at": "2026-01-02 08:30:00"
    });
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/usage"),
        Some(ok_date),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.body["used_at"], "2026-01-02T08:30:00");
}

#[tokio::test]
async fn usage_clamps_stock_at_zero() {
    let app = app().await;

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    let model_id = id(&res.body);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Canopy", 1)),
    )
    .await;
    let part_id = id(&res.body);

    // logging more usage than is on hand: record keeps the real quantity,
    // stock clamps at 0 (same rule as quantity adjusts)
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/usage"),
        Some(usage_part_body(model_id, 3)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.body["quantity"], 3);

    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{part_id}"),
        None,
    )
    .await;
    assert_eq!(res.body["part"]["quantity"], 0);
}

#[tokio::test]
async fn usage_model_scoped_endpoint_and_ordering() {
    let app = app().await;

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    let model_a = id(&res.body);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Sparrow 90", "plane")),
    )
    .await;
    let model_b = id(&res.body);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Main rotor blades", 10)),
    )
    .await;
    let part_id = id(&res.body);

    // model-scoped record with a backdate
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_a}/usage"),
        Some(serde_json::json!({
            "part_id": part_id,
            "quantity": 1,
            "used_at": "2026-01-02"
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.body["model_name"], "Kraken 580");

    // second record defaults to "now" (later than the backdate)
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_b}/usage"),
        Some(usage_model_body(part_id, 1)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);

    // newest first
    let res = call(app.clone(), Method::GET, "/api/usage", None).await;
    let list = res.body.as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["model_id"], model_b, "newest entry first");
    assert_eq!(list[1]["model_id"], model_a);

    // model-scoped filter
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/usage?model_id={model_a}"),
        None,
    )
    .await;
    let list = res.body.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["part_id"], part_id);
}

#[tokio::test]
async fn usage_deleted_with_its_part_and_model() {
    let app = app().await;

    let res = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    let model_id = id(&res.body);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Main rotor blades", 5)),
    )
    .await;
    let part_id = id(&res.body);
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/usage"),
        Some(usage_part_body(model_id, 1)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);

    // deleting the part cascades its usage entries
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/parts/{part_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(app.clone(), Method::GET, "/api/usage", None).await;
    assert_eq!(res.body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn part_bulk_edit_fields_and_links() {
    let app = app().await;

    let m = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    let model_id = id(&m.body);
    let mut ids = Vec::new();
    for (name, qty) in [("Blades A", 5), ("Blades B", 3), ("Canopy", 1)] {
        let res = call(
            app.clone(),
            Method::POST,
            "/api/parts",
            Some(part_json(name, qty)),
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED);
        ids.push(id(&res.body));
    }
    let [p1, p2, p3] = ids.try_into().unwrap();

    // set vendor + cost + quantity on p1/p2 and clear their link; p3 untouched
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({
            "part_ids": [p1, p2],
            "vendor": "Vortex",
            "cost": 12.5,
            "quantity": 4,
            "link": null,
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let rows = res.body.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row["vendor"], "Vortex");
        assert_eq!(row["cost"], 12.5);
        assert_eq!(row["quantity"], 4);
        assert!(row["link"].is_null(), "link cleared");
        assert_eq!(row["notes"], "test part", "untouched field stays");
        assert_eq!(row["model_count"], 0);
    }

    // p3 untouched
    let res = call(app.clone(), Method::GET, &format!("/api/parts/{p3}"), None).await;
    assert!(res.body["part"]["link"].as_str().is_some());
    assert!(res.body["part"]["vendor"].is_null());
    assert_eq!(res.body["part"]["quantity"], 1);

    // bulk-link a model to p1+p2; field values carry through
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({
            "part_ids": [p1, p2],
            "model_id": model_id,
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    for row in res.body.as_array().unwrap() {
        assert_eq!(row["model_count"], 1);
        assert_eq!(row["model_names"], "Kraken 580");
    }
    assert_eq!(res.body.as_array().unwrap()[0]["vendor"], "Vortex");

    // re-linking is idempotent
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({
            "part_ids": [p1, p2],
            "model_id": model_id,
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body.as_array().unwrap()[0]["model_count"], 1);

    // bulk-unlink; unlinking a model that is not linked is a no-op
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({
            "part_ids": [p1, p2],
            "unlink_model_ids": [model_id],
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    for row in res.body.as_array().unwrap() {
        assert_eq!(row["model_count"], 0);
    }
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({
            "part_ids": [p1, p2],
            "unlink_model_ids": [model_id],
        })),
    )
    .await;
    assert_eq!(
        res.status,
        StatusCode::OK,
        "unlinking an absent link is a no-op"
    );

    // link + unlink in one call
    let m2 = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("S500", "heli")),
    )
    .await;
    let m2_id = id(&m2.body);
    let res = call(
        app,
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({
            "part_ids": [p2],
            "model_id": model_id,
            "unlink_model_ids": [m2_id],
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let row = &res.body.as_array().unwrap()[0];
    assert_eq!(row["model_names"], "Kraken 580");
}

#[tokio::test]
async fn part_bulk_edit_validation_and_errors() {
    let app = app().await;

    let p = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Blades", 3)),
    )
    .await;
    let part_id = id(&p.body);
    let m = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Kraken 580", "heli")),
    )
    .await;
    let model_id = id(&m.body);

    // empty part_ids
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": []})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body["error"], "invalid_request");

    // nothing to change
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": [part_id]})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // unknown part -> 404
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": [part_id, 424242], "vendor": "X"})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.body["error"], "not_found");

    // bad quantity / cost
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": [part_id], "quantity": -1})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": [part_id], "cost": -2.5})),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // unknown models on both link sides
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": [part_id], "model_id": 424242})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": [part_id], "unlink_model_ids": [424242]})),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // the rejected calls left the part untouched
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{part_id}"),
        None,
    )
    .await;
    assert_eq!(res.body["part"]["quantity"], 3);
    assert!(res.body["part"]["vendor"].is_null());

    // duplicate ids collapse
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": [part_id, part_id], "vendor": "Vortex"})),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body.as_array().unwrap().len(), 1);

    // a whitespace-only string acts as a clear
    let res = call(
        app,
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({"part_ids": [part_id], "notes": "   "})),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.body.as_array().unwrap()[0]["notes"].is_null());
    assert_eq!(model_id, id(&m.body)); // keep the created model in scope
}
