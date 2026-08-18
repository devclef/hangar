//! End-to-end API tests: real axum router + real SQLite (in-memory),
//! exercising handlers, service rules, and the model<->part association.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use hangar::service::{Service, ServiceApi};
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
async fn low_stock_flag_roundtrip_and_bulk_edit() {
    let app = app().await;

    // omitted on create -> defaults to enabled
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Blades A", 5)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let p1 = id(&res.body);
    assert_eq!(res.body["low_stock_enabled"], true);

    // explicit false is honored, in detail and list rows
    let mut body = part_json("Blades B", 1);
    body["low_stock_enabled"] = serde_json::json!(false);
    let res = call(app.clone(), Method::POST, "/api/parts", Some(body)).await;
    assert_eq!(res.status, StatusCode::CREATED);
    let p2 = id(&res.body);
    assert_eq!(res.body["low_stock_enabled"], false);
    let res = call(app.clone(), Method::GET, &format!("/api/parts/{p2}"), None).await;
    assert_eq!(res.body["part"]["low_stock_enabled"], false);
    let res = call(app.clone(), Method::GET, "/api/parts", None).await;
    let rows = res.body.as_array().unwrap();
    let row1 = rows.iter().find(|r| r["id"] == p1).unwrap();
    let row2 = rows.iter().find(|r| r["id"] == p2).unwrap();
    assert_eq!(row1["low_stock_enabled"], true);
    assert_eq!(row2["low_stock_enabled"], false);

    // full-replace update re-enables it
    let res = call(
        app.clone(),
        Method::PUT,
        &format!("/api/parts/{p2}"),
        Some(part_json("Blades B", 1)),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["low_stock_enabled"], true);

    // bulk edit: disable on p2 only, leave p1 untouched
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({
            "part_ids": [p1, p2],
            "low_stock_enabled": false,
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    for row in res.body.as_array().unwrap() {
        assert_eq!(row["low_stock_enabled"], false);
    }
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/bulk-edit",
        Some(serde_json::json!({
            "part_ids": [p1],
            "low_stock_enabled": true,
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let rows = res.body.as_array().unwrap();
    assert_eq!(rows[0]["low_stock_enabled"], true);
    // p2 stays disabled
    let res = call(app, Method::GET, &format!("/api/parts/{p2}"), None).await;
    assert_eq!(res.body["part"]["low_stock_enabled"], false);
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
    assert_eq!(res.body["low_stock_enabled"], true);
    assert_eq!(res.body["low_stock_threshold"], 2);
    assert_eq!(res.body["theme"], "system");

    // update: hide some fields, change currency (normalized, dupes collapsed)
    let res = call(
        app.clone(),
        Method::PUT,
        "/api/settings",
        Some(serde_json::json!({
            "part_form_fields": ["quantity", "cost", "cost"],
            "currency": "  eur ",
            "low_stock_enabled": false,
            "low_stock_threshold": 5,
            "theme": "dark"
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["currency"], "EUR");
    assert_eq!(res.body["low_stock_enabled"], false);
    assert_eq!(res.body["low_stock_threshold"], 5);
    assert_eq!(res.body["theme"], "dark");
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
    assert_eq!(res.body["low_stock_enabled"], false);
    assert_eq!(res.body["low_stock_threshold"], 5);
    assert_eq!(res.body["theme"], "dark");
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
            "currency": "USD",
            "theme": "system"
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
            "currency": "U.S!",
            "theme": "system"
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // a missing required key is a deserialization failure -> 400
    let res = call(
        app.clone(),
        Method::PUT,
        "/api/settings",
        Some(serde_json::json!({ "part_form_fields": [] })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // low_stock_threshold out of range is rejected
    for threshold in [-1, 1001] {
        let res = call(
            app.clone(),
            Method::PUT,
            "/api/settings",
            Some(serde_json::json!({
                "part_form_fields": [],
                "currency": "USD",
                "low_stock_enabled": true,
                "low_stock_threshold": threshold,
                "theme": "system"
            })),
        )
        .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "threshold {threshold}");
    }
    // threshold 0 is valid (low state never appears)
    let res = call(
        app.clone(),
        Method::PUT,
        "/api/settings",
        Some(serde_json::json!({
            "part_form_fields": [],
            "currency": "USD",
            "low_stock_enabled": true,
            "low_stock_threshold": 0,
            "theme": "light"
        })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["theme"], "light");

    // an unknown theme is rejected
    let res = call(
        app,
        Method::PUT,
        "/api/settings",
        Some(serde_json::json!({
            "part_form_fields": [],
            "currency": "USD",
            "low_stock_enabled": true,
            "low_stock_threshold": 0,
            "theme": "neon"
        })),
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

// ---------------------------------------------------------------------------
// Reference catalog
// ---------------------------------------------------------------------------

/// An app plus its service, so tests can drive the catalog importer directly
/// (startup/CLI do the same) before hitting the HTTP API.
async fn app_with_service() -> (axum::Router, std::sync::Arc<hangar::service::Service>) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let service = std::sync::Arc::new(hangar::service::Service::from_sqlite(pool));
    let state = AppState {
        service: service.clone(),
        static_dir: None,
    };
    (hangar::router(state), service)
}

static CATALOG_FILE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Writes a catalog JSON file to a uniquely-named temp path.
fn write_catalog_file(contents: &str) -> std::path::PathBuf {
    let n = CATALOG_FILE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "hangar-catalog-api-{}-{}.json",
        std::process::id(),
        n
    ));
    std::fs::write(&path, contents).unwrap();
    path
}

const M1_JSON: &str = r#"{
  "manufacturer": "OMP Hobby",
  "model": "M1",
  "category": "heli",
  "diagram_asset": "heli-generic.svg",
  "parts": [
    {
      "name": "Main blade grip set",
      "part_number": "OSHM1013",
      "category": "Blade grip",
      "notes": "Includes bearings",
      "diagram_x": 37.0,
      "diagram_y": 18.0
    },
    {
      "name": "Tail blade grip set",
      "part_number": null,
      "category": "Blade grip",
      "notes": "Part number not yet verified",
      "diagram_x": 91.0,
      "diagram_y": 47.0
    },
    { "name": "Hardware bag", "part_number": null, "notes": "Not diagram-placeable" }
  ]
}"#;

#[tokio::test]
async fn catalog_import_browse_and_short_circuit() {
    let (app, service) = app_with_service().await;

    // import a file
    let path = write_catalog_file(M1_JSON);
    let result = service
        .import_catalog_file(&path)
        .await
        .expect("import should succeed");
    assert!(matches!(
        result.status,
        hangar::catalog::ImportStatus::Created
    ));
    assert!(result.model_created);
    assert_eq!(result.parts_created, 3);
    assert!(result.orphaned_parts.is_empty());

    // list manufacturers (with model_count)
    let res = call(app.clone(), Method::GET, "/api/catalog/manufacturers", None).await;
    assert_eq!(res.status, StatusCode::OK);
    let list = res.body.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "OMP Hobby");
    assert_eq!(list[0]["model_count"], 1);
    let mfr_id = list[0]["id"].as_i64().unwrap();

    // list models for a manufacturer
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/manufacturers/{mfr_id}/models"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let models = res.body.as_array().unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["name"], "M1");
    assert_eq!(models[0]["category"], "heli");
    assert_eq!(models[0]["diagram_asset"], "heli-generic.svg");
    assert_eq!(models[0]["manufacturer"], "OMP Hobby");
    assert!(!models[0]["source_file"].as_str().unwrap().is_empty());
    assert_eq!(models[0]["source_checksum"].as_str().unwrap().len(), 64);
    let cm_id = models[0]["id"].as_i64().unwrap();

    // unknown manufacturer / model -> 404
    let res = call(
        app.clone(),
        Method::GET,
        "/api/catalog/manufacturers/999/models",
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = call(app.clone(), Method::GET, "/api/catalog/models/999", None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // catalog model detail: parts with coordinates + null owned quantities
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["model"]["name"], "M1");
    assert_eq!(res.body["diagram_asset"], "heli-generic.svg");
    assert!(res.body["linked_models"].as_array().unwrap().is_empty());
    let parts = res.body["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0]["name"], "Main blade grip set");
    assert_eq!(parts[0]["part_number"], "OSHM1013");
    assert_eq!(parts[0]["diagram_x"], 37.0);
    assert_eq!(parts[0]["diagram_y"], 18.0);
    assert!(parts[1]["part_number"].is_null());
    assert!(parts[2]["diagram_x"].is_null(), "no coordinates -> null");
    for p in parts {
        assert!(
            p["owned_quantity"].is_null(),
            "no linked models -> null quantity"
        );
    }

    // re-importing the same file short-circuits: unchanged, nothing is even
    // re-parsed (counts stay zero — the file was skipped by checksum)
    let result = service.import_catalog_file(&path).await.unwrap();
    assert!(matches!(
        result.status,
        hangar::catalog::ImportStatus::Unchanged
    ));
    assert_eq!(result.parts_created, 0);
    assert_eq!(result.parts_updated, 0);
    assert!(result.orphaned_parts.is_empty());

    // and the data is untouched
    let res = call(
        app,
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    assert_eq!(res.body["parts"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn catalog_reimport_updates_matches_and_orphans() {
    let (app, service) = app_with_service().await;
    let path = write_catalog_file(M1_JSON);
    service.import_catalog_file(&path).await.unwrap();

    // v2: rename the part-numbered part (matched by number), drop the
    // hardware bag (orphan), add a new numbered part.
    let v2 = serde_json::json!({
        "manufacturer": "OMP Hobby",
        "model": "M1",
        "category": "heli",
        "diagram_asset": "heli-generic.svg",
        "parts": [
            {
                "name": "Main blade grip set v2",
                "part_number": "OSHM1013",
                "category": "Blade grip",
                "diagram_x": 37.0,
                "diagram_y": 18.0
            },
            {
                "name": "Tail blade grip set",
                "part_number": null,
                "category": "Blade grip",
                "diagram_x": 91.0,
                "diagram_y": 47.0
            },
            { "name": "Canopy", "part_number": "OSHCAN1" }
        ]
    })
    .to_string();
    std::fs::write(&path, v2).unwrap();
    let result = service.import_catalog_file(&path).await.unwrap();
    assert!(matches!(
        result.status,
        hangar::catalog::ImportStatus::Updated
    ));
    assert!(!result.model_created);
    assert_eq!(
        result.parts_updated, 2,
        "main grip renamed + notes dropped, tail grip notes dropped"
    );
    assert_eq!(result.parts_created, 1, "canopy is new");
    assert_eq!(result.parts_unchanged, 0);
    assert_eq!(result.orphaned_parts.len(), 1);
    assert_eq!(result.orphaned_parts[0].1, "Hardware bag");

    // v3: give the name-matched part a part number — it must be matched by
    // name (its old key) and re-keyed by number, not duplicated.
    let v3 = serde_json::json!({
        "manufacturer": "OMP Hobby",
        "model": "M1",
        "category": "heli",
        "diagram_asset": "heli-generic.svg",
        "parts": [
            {
                "name": "Main blade grip set v2",
                "part_number": "OSHM1013",
                "category": "Blade grip",
                "diagram_x": 37.0,
                "diagram_y": 18.0
            },
            {
                "name": "Tail blade grip set",
                "part_number": "OSHTBL2",
                "category": "Blade grip",
                "diagram_x": 91.0,
                "diagram_y": 47.0
            },
            { "name": "Canopy", "part_number": "OSHCAN1" }
        ]
    })
    .to_string();
    std::fs::write(&path, v3).unwrap();
    let result = service.import_catalog_file(&path).await.unwrap();
    assert_eq!(
        result.parts_updated, 1,
        "tail grip matched by name, gained a number"
    );
    assert_eq!(result.parts_unchanged, 2);
    // the hardware bag was orphaned in v2 and is still in the table — it
    // stays orphaned (orphan rows persist until explicitly deleted)
    assert_eq!(
        result
            .orphaned_parts
            .iter()
            .map(|(_, n)| n.as_str())
            .collect::<Vec<_>>(),
        vec!["Hardware bag"]
    );

    // no duplicates: the model still has exactly 3 parts, and the orphan is
    // still queryable (left in place by design)
    let mfr_id = {
        let res = call(app.clone(), Method::GET, "/api/catalog/manufacturers", None).await;
        res.body.as_array().unwrap()[0]["id"].as_i64().unwrap()
    };
    let cm_id = {
        let res = call(
            app.clone(),
            Method::GET,
            &format!("/api/catalog/manufacturers/{mfr_id}/models"),
            None,
        )
        .await;
        res.body.as_array().unwrap()[0]["id"].as_i64().unwrap()
    };
    let res = call(
        app,
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    let names: Vec<_> = res.body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect();
    // (ordered by id: the hardware bag was imported first and survives as
    // an orphan; canopy arrived in v2)
    assert_eq!(
        names,
        vec![
            "Main blade grip set v2",
            "Tail blade grip set",
            "Hardware bag",
            "Canopy"
        ]
    );
}

#[tokio::test]
async fn catalog_orphan_keeps_inventory_links() {
    let (app, service) = app_with_service().await;
    let path = write_catalog_file(M1_JSON);
    service.import_catalog_file(&path).await.unwrap();

    let cm_id = {
        let res = call(app.clone(), Method::GET, "/api/catalog/manufacturers", None).await;
        let mfr_id = res.body.as_array().unwrap()[0]["id"].as_i64().unwrap();
        let res = call(
            app.clone(),
            Method::GET,
            &format!("/api/catalog/manufacturers/{mfr_id}/models"),
            None,
        )
        .await;
        res.body.as_array().unwrap()[0]["id"].as_i64().unwrap()
    };

    // create a model, link it, and add the hardware bag (no part number) to inventory
    let m = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("My M1", "heli")),
    )
    .await;
    let model_id = id(&m.body);
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_model_id": cm_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);

    let bag_cp_id = {
        let res = call(
            app.clone(),
            Method::GET,
            &format!("/api/catalog/models/{cm_id}"),
            None,
        )
        .await;
        res.body["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "Hardware bag")
            .unwrap()["id"]
            .as_i64()
            .unwrap()
    };
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/catalog/parts/{bag_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": model_id, "quantity": 2 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let inv_part_id = id(&res.body);

    // v2 drops the hardware bag: re-import must NOT touch the inventory part
    let v2 = serde_json::json!({
        "manufacturer": "OMP Hobby",
        "model": "M1",
        "category": "heli",
        "diagram_asset": "heli-generic.svg",
        "parts": [
            {
                "name": "Main blade grip set",
                "part_number": "OSHM1013",
                "category": "Blade grip",
                "diagram_x": 37.0,
                "diagram_y": 18.0
            }
        ]
    })
    .to_string();
    std::fs::write(&path, v2).unwrap();
    let result = service.import_catalog_file(&path).await.unwrap();
    assert_eq!(
        result
            .orphaned_parts
            .iter()
            .map(|(_, n)| n.as_str())
            .collect::<Vec<_>>(),
        vec!["Tail blade grip set", "Hardware bag"]
    );

    // inventory part survives the re-import, still linked, quantity intact
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{inv_part_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["part"]["quantity"], 2);
    assert_eq!(res.body["models"][0]["id"], model_id);
}

#[tokio::test]
async fn catalog_invalid_files_do_not_break_the_rest() {
    let dir = std::env::temp_dir().join(format!(
        "hangar-catalog-dir-{}-{}",
        std::process::id(),
        CATALOG_FILE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bad-json.json"), "{ not json").unwrap();
    std::fs::write(
        dir.join("bad-category.json"),
        r#"{"manufacturer":"M","model":"X","category":"spaceship","parts":[]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("bad-coords.json"),
        r#"{"manufacturer":"M","model":"Y","category":"heli","parts":[{"name":"A","diagram_x":150.0,"diagram_y":1.0}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("lonely-x.json"),
        r#"{"manufacturer":"M","model":"Z","category":"heli","parts":[{"name":"B","diagram_x":10.0}]}"#,
    )
    .unwrap();
    std::fs::write(dir.join("good.json"), M1_JSON).unwrap();

    let (_app, service) = app_with_service().await;
    let summary = service.import_catalog_dir(&dir).await.unwrap();
    assert_eq!(summary.files, 5);
    assert_eq!(summary.created, 1, "the good file still imports");
    assert_eq!(summary.failed.len(), 4);
    for (file, err) in &summary.failed {
        assert!(err.contains(':'), "error carries a field path: {err}");
        assert!(file.ends_with(".json"));
    }
    let files: Vec<_> = summary.failed.iter().map(|(f, _)| f.as_str()).collect();
    for want in [
        "bad-json.json",
        "bad-category.json",
        "bad-coords.json",
        "lonely-x.json",
    ] {
        assert!(files.contains(&want), "{want} should have failed");
    }

    // unknown fields are rejected too (typo protection)
    let bad = write_catalog_file(
        r#"{"manufacturer":"M","model":"T","category":"heli","parts":[],"typo":1}"#,
    );
    let err = service.import_catalog_file(&bad).await.unwrap_err();
    assert!(err.to_string().contains("typo"), "{err}");

    // a missing directory is not an error
    let empty = service
        .import_catalog_dir(std::path::Path::new("/definitely/not/here"))
        .await
        .unwrap();
    assert_eq!(empty.files, 0);
    assert!(empty.ok());

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn catalog_link_unlink_and_model_detail_summary() {
    let (app, service) = app_with_service().await;
    let path = write_catalog_file(M1_JSON);
    service.import_catalog_file(&path).await.unwrap();

    let cm_id = {
        let res = call(app.clone(), Method::GET, "/api/catalog/manufacturers", None).await;
        let mfr_id = res.body.as_array().unwrap()[0]["id"].as_i64().unwrap();
        let res = call(
            app.clone(),
            Method::GET,
            &format!("/api/catalog/manufacturers/{mfr_id}/models"),
            None,
        )
        .await;
        res.body.as_array().unwrap()[0]["id"].as_i64().unwrap()
    };

    let heli = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("My M1", "heli")),
    )
    .await;
    let heli_id = id(&heli.body);
    let car = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Speedy", "car")),
    )
    .await;
    let car_id = id(&car.body);

    // link the heli
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{heli_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_model_id": cm_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["catalog_model_id"], cm_id);

    // model detail now embeds the catalog summary
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/models/{heli_id}"),
        None,
    )
    .await;
    assert_eq!(res.body["model"]["catalog_model_id"], cm_id);
    assert_eq!(res.body["catalog"]["catalog_model_name"], "M1");
    assert_eq!(res.body["catalog"]["diagram_asset"], "heli-generic.svg");

    // re-linking the same value is an idempotent no-op
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{heli_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_model_id": cm_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);

    // category mismatch is rejected
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{car_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_model_id": cm_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body["error"], "invalid_request");
    assert!(res.body["message"]
        .as_str()
        .unwrap()
        .contains("category mismatch"));
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/models/{car_id}"),
        None,
    )
    .await;
    assert!(
        res.body["model"].get("catalog_model_id").is_none()
            || res.body["model"]["catalog_model_id"].is_null()
    );

    // unknown ids -> 404
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{heli_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_model_id": 999 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/models/999/link-catalog",
        Some(serde_json::json!({ "catalog_model_id": cm_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // unlink
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/models/{heli_id}/link-catalog"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/models/{heli_id}"),
        None,
    )
    .await;
    assert!(res.body["model"]["catalog_model_id"].is_null());
    assert!(res.body.get("catalog").is_none());

    // unlinking twice -> 404
    let res = call(
        app,
        Method::DELETE,
        &format!("/api/models/{heli_id}/link-catalog"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.body["error"], "not_found");
}

#[tokio::test]
async fn catalog_add_to_inventory_create_increment_and_scoping() {
    let (app, service) = app_with_service().await;
    let path = write_catalog_file(M1_JSON);
    service.import_catalog_file(&path).await.unwrap();

    let cm_id = {
        let res = call(app.clone(), Method::GET, "/api/catalog/manufacturers", None).await;
        let mfr_id = res.body.as_array().unwrap()[0]["id"].as_i64().unwrap();
        let res = call(
            app.clone(),
            Method::GET,
            &format!("/api/catalog/manufacturers/{mfr_id}/models"),
            None,
        )
        .await;
        res.body.as_array().unwrap()[0]["id"].as_i64().unwrap()
    };
    let grip_cp_id = {
        let res = call(
            app.clone(),
            Method::GET,
            &format!("/api/catalog/models/{cm_id}"),
            None,
        )
        .await;
        res.body["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "Main blade grip set")
            .unwrap()["id"]
            .as_i64()
            .unwrap()
    };

    let m = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("My M1", "heli")),
    )
    .await;
    let model_id = id(&m.body);
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_model_id": cm_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);

    // create (default quantity 1): pre-filled from the catalog entry
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/catalog/parts/{grip_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": model_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let inv_part_id = id(&res.body);
    assert_eq!(res.body["name"], "Main blade grip set");
    assert_eq!(
        res.body["link"], "OSHM1013",
        "part_number pre-fills the link field"
    );
    assert_eq!(res.body["quantity"], 1);

    // increment (idempotent path): no duplicate, quantity adjusts
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/catalog/parts/{grip_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": model_id, "quantity": 2 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(
        id(&res.body),
        inv_part_id,
        "same part adjusted, not duplicated"
    );
    assert_eq!(res.body["quantity"], 3);

    // delta semantics: negative allowed (clamped at 0), zero rejected
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/catalog/parts/{grip_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": model_id, "quantity": -100 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["quantity"], 0, "clamped at 0");
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/catalog/parts/{grip_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": model_id, "quantity": 0 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // negative starting quantity is rejected on the create path (a catalog
    // part that does not have an inventory row on this model yet)
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    let tail_cp_id = res.body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Tail blade grip set")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/catalog/parts/{tail_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": model_id, "quantity": -1 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body["error"], "invalid_request");

    // scoping: a second linked model sees null until it owns something
    let m2 = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Second M1", "heli")),
    )
    .await;
    let model2_id = id(&m2.body);
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model2_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_model_id": cm_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);

    // restock the grip for model 1
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/catalog/parts/{grip_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": model_id, "quantity": 5 })),
    )
    .await;
    assert_eq!(res.body["quantity"], 5);

    // scoped to model 2 -> its own (null) view; unscoped -> sum over both
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}?model_id={model2_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let grip2 = res.body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Main blade grip set")
        .unwrap();
    assert_eq!(
        grip2["owned_quantity"], 0,
        "model 2 is linked but owns nothing yet -> 0, not null"
    );
    assert_eq!(res.body["linked_models"].as_array().unwrap().len(), 2);

    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    let grip_all = res.body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Main blade grip set")
        .unwrap();
    assert_eq!(grip_all["owned_quantity"], 5, "sums over all linked models");

    // unknown scope model -> 404; scoped model not linked -> nulls
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}?model_id=999"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let car = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("Speedy", "car")),
    )
    .await;
    let car_id = id(&car.body);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}?model_id={car_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["owned_quantity"].is_null()));

    // unknown catalog part / model -> 404
    let res = call(
        app.clone(),
        Method::POST,
        "/api/catalog/parts/999/add-to-inventory",
        Some(serde_json::json!({ "model_id": model_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = call(
        app,
        Method::POST,
        &format!("/api/catalog/parts/{grip_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": 999 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn catalog_part_delete_keeps_inventory() {
    let (app, service) = app_with_service().await;
    let path = write_catalog_file(M1_JSON);
    service.import_catalog_file(&path).await.unwrap();

    let cm_id = {
        let res = call(app.clone(), Method::GET, "/api/catalog/manufacturers", None).await;
        let mfr_id = res.body.as_array().unwrap()[0]["id"].as_i64().unwrap();
        let res = call(
            app.clone(),
            Method::GET,
            &format!("/api/catalog/manufacturers/{mfr_id}/models"),
            None,
        )
        .await;
        res.body.as_array().unwrap()[0]["id"].as_i64().unwrap()
    };
    let grip_cp_id = {
        let res = call(
            app.clone(),
            Method::GET,
            &format!("/api/catalog/models/{cm_id}"),
            None,
        )
        .await;
        res.body["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "Main blade grip set")
            .unwrap()["id"]
            .as_i64()
            .unwrap()
    };

    let m = call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("My M1", "heli")),
    )
    .await;
    let model_id = id(&m.body);
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/catalog/parts/{grip_cp_id}/add-to-inventory"),
        Some(serde_json::json!({ "model_id": model_id, "quantity": 4 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let inv_part_id = id(&res.body);

    // admin delete of the catalog part
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/catalog/parts/{grip_cp_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/catalog/parts/{grip_cp_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // inventory part survives, still linked, but the trace link is gone
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{inv_part_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["part"]["quantity"], 4);
    assert_eq!(res.body["models"][0]["id"], model_id);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    assert!(!res.body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["name"] == "Main blade grip set"));
}

#[tokio::test]
async fn catalog_part_search_link_and_owned_quantities() {
    let (app, service) = app_with_service().await;

    // Import the catalog file and resolve ids.
    let path = write_catalog_file(M1_JSON);
    service.import_catalog_file(&path).await.unwrap();
    let res = call(app.clone(), Method::GET, "/api/catalog/manufacturers", None).await;
    let mfr_id = res.body.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/manufacturers/{mfr_id}/models"),
        None,
    )
    .await;
    let cm_id = res.body.as_array().unwrap()[0]["id"].as_i64().unwrap();

    // Search: name substring
    let res = call(
        app.clone(),
        Method::GET,
        "/api/catalog/parts?q=blade%20grip",
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let hits = res.body.as_array().unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0]["name"], "Main blade grip set");
    assert_eq!(hits[0]["part_number"], "OSHM1013");
    assert_eq!(hits[0]["catalog_model_id"], cm_id);
    assert_eq!(hits[0]["catalog_model_name"], "M1");
    assert_eq!(hits[0]["manufacturer"], "OMP Hobby");
    assert_eq!(hits[0]["model_category"], "heli");
    let grip_cp_id = hits[0]["id"].as_i64().unwrap();
    assert_eq!(hits[1]["name"], "Tail blade grip set");
    assert!(hits[1]["part_number"].is_null());
    let tail_cp_id = hits[1]["id"].as_i64().unwrap();

    // Search: part number
    let res = call(
        app.clone(),
        Method::GET,
        "/api/catalog/parts?q=OSHM1013",
        None,
    )
    .await;
    let hits = res.body.as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["id"], grip_cp_id);

    // Search: notes
    let res = call(
        app.clone(),
        Method::GET,
        "/api/catalog/parts?q=bearings",
        None,
    )
    .await;
    assert_eq!(res.body.as_array().unwrap().len(), 1);

    // Browse mode (no q) lists every part; no match yields an empty list.
    let res = call(app.clone(), Method::GET, "/api/catalog/parts", None).await;
    assert_eq!(res.body.as_array().unwrap().len(), 3);
    let res = call(app.clone(), Method::GET, "/api/catalog/parts?q=zzz", None).await;
    assert!(res.body.as_array().unwrap().is_empty());

    // A hand-created part has no catalog link, yet exists fine.
    let model_id = id(&call(
        app.clone(),
        Method::POST,
        "/api/models",
        Some(model_json("My M1", "heli")),
    )
    .await
    .body);
    let part_id = id(&call(
        app.clone(),
        Method::POST,
        "/api/parts",
        Some(part_json("Main blade grip set", 2)),
    )
    .await
    .body);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{part_id}"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.body["catalog"].is_null());
    assert!(res.body["part"]["catalog_part_id"].is_null());

    // Error paths: unlinking a part with no link; linking an unknown
    // catalog part; linking an unknown part.
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/parts/{part_id}/link-catalog"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_part_id": 999 })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = call(
        app.clone(),
        Method::POST,
        "/api/parts/999/link-catalog",
        Some(serde_json::json!({ "catalog_part_id": grip_cp_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Link the part: returns the refreshed detail with the embed.
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_part_id": grip_cp_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let detail = &res.body;
    assert_eq!(detail["part"]["id"], part_id);
    assert_eq!(detail["part"]["catalog_part_id"], grip_cp_id);
    assert_eq!(detail["catalog"]["catalog_part_id"], grip_cp_id);
    assert_eq!(
        detail["catalog"]["catalog_part_name"],
        "Main blade grip set"
    );
    assert_eq!(detail["catalog"]["part_number"], "OSHM1013");
    assert_eq!(detail["catalog"]["catalog_model_id"], cm_id);
    assert_eq!(detail["catalog"]["catalog_model_name"], "M1");
    assert_eq!(detail["catalog"]["manufacturer"], "OMP Hobby");
    assert_eq!(detail["catalog"]["model_category"], "heli");

    // List rows expose the link too.
    let res = call(app.clone(), Method::GET, "/api/parts", None).await;
    let row = res
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == part_id)
        .unwrap();
    assert_eq!(row["catalog_part_id"], grip_cp_id);

    // A linked part does NOT count yet: the part must also be linked to a
    // user model that is linked to the catalog model.
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    let view = res.body["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == grip_cp_id)
        .unwrap();
    assert!(view["owned_quantity"].is_null());

    // Link model -> catalog and part -> model: the quantity now counts.
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/models/{model_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_model_id": cm_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/models"),
        Some(serde_json::json!({ "model_id": model_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    let parts = res.body["parts"].as_array().unwrap();
    let view = parts.iter().find(|p| p["id"] == grip_cp_id).unwrap();
    assert_eq!(view["owned_quantity"], 2);

    // Re-linking replaces the target: owned quantity moves to the new part.
    let res = call(
        app.clone(),
        Method::POST,
        &format!("/api/parts/{part_id}/link-catalog"),
        Some(serde_json::json!({ "catalog_part_id": tail_cp_id })),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["part"]["catalog_part_id"], tail_cp_id);
    assert_eq!(
        res.body["catalog"]["catalog_part_name"],
        "Tail blade grip set"
    );
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    let parts = res.body["parts"].as_array().unwrap();
    // Model is linked, so quantities are integers: the grip drops to 0.
    assert_eq!(
        parts.iter().find(|p| p["id"] == grip_cp_id).unwrap()["owned_quantity"],
        0
    );
    assert_eq!(
        parts.iter().find(|p| p["id"] == tail_cp_id).unwrap()["owned_quantity"],
        2
    );

    // Unlink: quantity drops back out; a second unlink is a 404.
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/parts/{part_id}/link-catalog"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/parts/{part_id}"),
        None,
    )
    .await;
    assert!(res.body["catalog"].is_null());
    assert!(res.body["part"]["catalog_part_id"].is_null());
    let res = call(
        app.clone(),
        Method::GET,
        &format!("/api/catalog/models/{cm_id}"),
        None,
    )
    .await;
    let parts = res.body["parts"].as_array().unwrap();
    assert_eq!(
        parts.iter().find(|p| p["id"] == tail_cp_id).unwrap()["owned_quantity"],
        0
    );
    let res = call(
        app.clone(),
        Method::DELETE,
        &format!("/api/parts/{part_id}/link-catalog"),
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}
