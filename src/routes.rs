//! HTTP layer: routes, extractors, and status codes. All business rules
//! live in `ServiceApi`; handlers are thin.

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::error::DomainError;
use crate::service::ServiceApi;
use crate::types::{
    Model, ModelDetail, ModelInput, ModelListFilter, ModelListRow, Part, PartDetail, PartInput,
    PartListFilter, PartListRow,
};

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<dyn ServiceApi>,
    pub static_dir: Option<PathBuf>,
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/models", get(list_models).post(create_model))
        .route(
            "/models/{id}",
            get(get_model).put(update_model).delete(delete_model),
        )
        .route(
            "/models/{id}/parts",
            get(list_model_parts).post(link_part).put(replace_model_parts),
        )
        .route(
            "/models/{id}/parts/{part_id}",
            delete(unlink_part),
        )
        .route("/parts", get(list_parts).post(create_part))
        .route(
            "/parts/{id}",
            get(get_part).put(update_part).delete(delete_part),
        )
        .route("/parts/{id}/quantity", post(adjust_quantity))
        .route(
            "/parts/{id}/models",
            get(list_part_models).post(link_model),
        )
        .route("/parts/{id}/models/{model_id}", delete(unlink_model));

    Router::new()
        .nest("/api", api)
        .fallback(crate::web::fallback)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Rejection handling: normalize axum rejections into the API error contract.
// ---------------------------------------------------------------------------

impl From<JsonRejection> for crate::error::DomainError {
    fn from(rej: JsonRejection) -> Self {
        crate::error::DomainError::Invalid(format!("invalid request body: {rej:?}"))
    }
}

impl From<QueryRejection> for crate::error::DomainError {
    fn from(rej: QueryRejection) -> Self {
        crate::error::DomainError::Invalid(format!("invalid query parameters: {rej:?}"))
    }
}

fn parse_body<T: serde::de::DeserializeOwned>(
    res: Result<Json<T>, JsonRejection>,
) -> Result<T, crate::error::DomainError> {
    Ok(res?.0)
}

fn parse_query<T: serde::de::DeserializeOwned>(
    res: Result<Query<T>, QueryRejection>,
) -> Result<T, crate::error::DomainError> {
    Ok(res?.0)
}

// ---------------------------------------------------------------------------
// Bodies
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LinkPartBody {
    pub part_id: i64,
}

#[derive(Deserialize)]
pub struct LinkModelBody {
    pub model_id: i64,
}

#[derive(Deserialize)]
pub struct ReplacePartsBody {
    pub part_ids: Vec<i64>,
}

#[derive(Deserialize)]
pub struct AdjustQuantityBody {
    pub delta: i64,
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

async fn list_models(
    State(st): State<AppState>,
    filter: Result<Query<ModelListFilter>, QueryRejection>,
) -> Result<Json<Vec<ModelListRow>>, DomainError> {
    let filter = parse_query(filter)?;
    Ok(Json(st.service.list_models(filter.q.as_deref(), filter.category).await?))
}

async fn create_model(
    State(st): State<AppState>,
    input: Result<Json<ModelInput>, JsonRejection>,
) -> Result<(StatusCode, Json<Model>), DomainError> {
    let input = parse_body(input)?;
    Ok((StatusCode::CREATED, Json(st.service.create_model(input).await?)))
}

async fn get_model(State(st): State<AppState>, Path(id): Path<i64>) -> Result<Json<ModelDetail>, DomainError> {
    Ok(Json(st.service.get_model_detail(id).await?))
}

async fn update_model(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    input: Result<Json<ModelInput>, JsonRejection>,
) -> Result<Json<Model>, DomainError> {
    let input = parse_body(input)?;
    Ok(Json(st.service.update_model(id, input).await?))
}

async fn delete_model(State(st): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, DomainError> {
    st.service.delete_model(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_model_parts(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<PartListRow>>, DomainError> {
    Ok(Json(st.service.list_model_parts(id).await?))
}

async fn link_part(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<LinkPartBody>, JsonRejection>,
) -> Result<StatusCode, DomainError> {
    let body = parse_body(body)?;
    st.service.link_part(id, body.part_id).await?;
    Ok(StatusCode::CREATED)
}

async fn unlink_part(
    State(st): State<AppState>,
    Path((id, part_id)): Path<(i64, i64)>,
) -> Result<StatusCode, DomainError> {
    st.service.unlink_part(id, part_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn replace_model_parts(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<ReplacePartsBody>, JsonRejection>,
) -> Result<Json<Vec<PartListRow>>, DomainError> {
    let body = parse_body(body)?;
    Ok(Json(st.service.replace_model_parts(id, body.part_ids).await?))
}

// ---------------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------------

async fn list_parts(
    State(st): State<AppState>,
    filter: Result<Query<PartListFilter>, QueryRejection>,
) -> Result<Json<Vec<PartListRow>>, DomainError> {
    let filter = parse_query(filter)?;
    Ok(Json(
        st.service
            .list_parts(filter.q.as_deref(), filter.part_type.as_deref(), filter.sort)
            .await?,
    ))
}

async fn create_part(State(st): State<AppState>, input: Result<Json<PartInput>, JsonRejection>) -> Result<(StatusCode, Json<Part>), DomainError> {
    let input = parse_body(input)?;
    Ok((StatusCode::CREATED, Json(st.service.create_part(input).await?)))
}

async fn get_part(State(st): State<AppState>, Path(id): Path<i64>) -> Result<Json<PartDetail>, DomainError> {
    Ok(Json(st.service.get_part_detail(id).await?))
}

async fn update_part(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    input: Result<Json<PartInput>, JsonRejection>,
) -> Result<Json<Part>, DomainError> {
    let input = parse_body(input)?;
    Ok(Json(st.service.update_part(id, input).await?))
}

async fn delete_part(State(st): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, DomainError> {
    st.service.delete_part(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn adjust_quantity(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<AdjustQuantityBody>, JsonRejection>,
) -> Result<Json<Part>, DomainError> {
    let body = parse_body(body)?;
    Ok(Json(st.service.adjust_quantity(id, body.delta).await?))
}

async fn list_part_models(State(st): State<AppState>, Path(id): Path<i64>) -> Result<Json<Vec<Model>>, DomainError> {
    Ok(Json(st.service.list_part_models(id).await?))
}

async fn link_model(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<LinkModelBody>, JsonRejection>,
) -> Result<StatusCode, DomainError> {
    let body = parse_body(body)?;
    st.service.link_part(body.model_id, id).await?;
    Ok(StatusCode::CREATED)
}

async fn unlink_model(
    State(st): State<AppState>,
    Path((id, model_id)): Path<(i64, i64)>,
) -> Result<StatusCode, DomainError> {
    st.service.unlink_part(model_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

