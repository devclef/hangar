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
    CatalogManufacturer, CatalogModel, CatalogModelDetail, CatalogPartSearchHit, Model,
    ModelDetail, ModelInput, ModelListFilter, ModelListRow, Part, PartBulkEdit, PartDetail,
    PartInput, PartListFilter, PartListRow, Settings, UsageFilter, UsageInput, UsageRecord,
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
            get(list_model_parts)
                .post(link_part)
                .put(replace_model_parts),
        )
        .route("/models/{id}/parts/{part_id}", delete(unlink_part))
        .route("/parts", get(list_parts).post(create_part))
        .route("/parts/bulk-edit", post(bulk_edit_parts))
        .route(
            "/parts/{id}",
            get(get_part).put(update_part).delete(delete_part),
        )
        .route("/parts/{id}/quantity", post(adjust_quantity))
        .route(
            "/parts/{id}/link-catalog",
            post(link_part_catalog).delete(unlink_part_catalog),
        )
        .route("/parts/{id}/models", get(list_part_models).post(link_model))
        .route("/parts/{id}/models/{model_id}", delete(unlink_model))
        .route("/usage", get(list_usage))
        .route("/parts/{id}/usage", post(log_part_usage))
        .route("/models/{id}/usage", post(log_model_usage))
        .route(
            "/models/{id}/link-catalog",
            post(link_catalog).delete(unlink_catalog),
        )
        .route("/catalog/manufacturers", get(list_catalog_manufacturers))
        .route(
            "/catalog/manufacturers/{id}/models",
            get(list_catalog_models),
        )
        .route("/catalog/models/{id}", get(get_catalog_model))
        .route("/catalog/parts", get(search_catalog_parts))
        .route("/catalog/parts/{id}", delete(delete_catalog_part))
        .route(
            "/catalog/parts/{id}/add-to-inventory",
            post(add_to_inventory),
        )
        .route("/settings", get(get_settings).put(update_settings));

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

#[derive(Deserialize)]
pub struct LinkCatalogBody {
    pub catalog_model_id: i64,
}

/// Body for `POST /api/parts/:id/link-catalog`: the reference catalog part
/// the inventory part traces back to.
#[derive(Deserialize)]
pub struct LinkPartCatalogBody {
    pub catalog_part_id: i64,
}

/// `GET /api/catalog/parts` filters: case-insensitive substring match on
/// part name, part number, or notes; omitted lists the first 100 parts.
#[derive(Deserialize)]
pub struct CatalogPartSearchFilter {
    #[serde(default)]
    pub q: Option<String>,
}

/// Body for `POST /api/catalog/parts/:id/add-to-inventory`. `quantity` is
/// the delta applied to an existing tied part (or the starting count of a
/// new one); omitted means +1.
#[derive(Deserialize)]
pub struct AddToInventoryBody {
    pub model_id: i64,
    #[serde(default)]
    pub quantity: Option<i64>,
}

/// `GET /api/catalog/models/:id` filters. `model_id` restricts owned
/// quantities to one specific user model (omitted: all linked models).
#[derive(Deserialize)]
pub struct CatalogModelFilter {
    #[serde(default)]
    pub model_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct LogPartUsageBody {
    pub model_id: i64,
    #[serde(default)]
    pub quantity: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub used_at: Option<String>,
}

#[derive(Deserialize)]
pub struct LogModelUsageBody {
    pub part_id: i64,
    #[serde(default)]
    pub quantity: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub used_at: Option<String>,
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
    Ok(Json(
        st.service
            .list_models(filter.q.as_deref(), filter.category)
            .await?,
    ))
}

async fn create_model(
    State(st): State<AppState>,
    input: Result<Json<ModelInput>, JsonRejection>,
) -> Result<(StatusCode, Json<Model>), DomainError> {
    let input = parse_body(input)?;
    Ok((
        StatusCode::CREATED,
        Json(st.service.create_model(input).await?),
    ))
}

async fn get_model(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ModelDetail>, DomainError> {
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

async fn delete_model(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, DomainError> {
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
    Ok(StatusCode::NO_CONTENT)
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
    Ok(Json(
        st.service.replace_model_parts(id, body.part_ids).await?,
    ))
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
            .list_parts(filter.q.as_deref(), filter.sort)
            .await?,
    ))
}

async fn bulk_edit_parts(
    State(st): State<AppState>,
    body: Result<Json<PartBulkEdit>, JsonRejection>,
) -> Result<Json<Vec<PartListRow>>, DomainError> {
    let body = parse_body(body)?;
    Ok(Json(st.service.bulk_edit_parts(body).await?))
}

async fn create_part(
    State(st): State<AppState>,
    input: Result<Json<PartInput>, JsonRejection>,
) -> Result<(StatusCode, Json<Part>), DomainError> {
    let input = parse_body(input)?;
    Ok((
        StatusCode::CREATED,
        Json(st.service.create_part(input).await?),
    ))
}

async fn get_part(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<PartDetail>, DomainError> {
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

async fn delete_part(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, DomainError> {
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

/// Links (or re-points) an inventory part to a reference catalog part.
/// The trace link powers the catalog view's owned quantities; it is never
/// set by `PUT /api/parts/:id`, so full-replace edits can't wipe it.
/// Returns the refreshed part detail (with the catalog summary embedded).
async fn link_part_catalog(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<LinkPartCatalogBody>, JsonRejection>,
) -> Result<Json<PartDetail>, DomainError> {
    let body = parse_body(body)?;
    st.service
        .link_part_catalog(id, body.catalog_part_id)
        .await?;
    Ok(Json(st.service.get_part_detail(id).await?))
}

async fn unlink_part_catalog(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, DomainError> {
    st.service.unlink_part_catalog(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_part_models(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Model>>, DomainError> {
    Ok(Json(st.service.list_part_models(id).await?))
}

async fn link_model(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<LinkModelBody>, JsonRejection>,
) -> Result<StatusCode, DomainError> {
    let body = parse_body(body)?;
    st.service.link_part(body.model_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn unlink_model(
    State(st): State<AppState>,
    Path((id, model_id)): Path<(i64, i64)>,
) -> Result<StatusCode, DomainError> {
    st.service.unlink_part(model_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Part usage log
// ---------------------------------------------------------------------------

async fn list_usage(
    State(st): State<AppState>,
    filter: Result<Query<UsageFilter>, QueryRejection>,
) -> Result<Json<Vec<UsageRecord>>, DomainError> {
    let filter = parse_query(filter)?;
    Ok(Json(
        st.service
            .list_usage(filter.part_id, filter.model_id)
            .await?,
    ))
}

async fn log_part_usage(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<LogPartUsageBody>, JsonRejection>,
) -> Result<(StatusCode, Json<UsageRecord>), DomainError> {
    let body = parse_body(body)?;
    let input = UsageInput {
        quantity: body.quantity,
        notes: body.notes,
        used_at: body.used_at,
    };
    let record = st.service.record_usage(id, body.model_id, input).await?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn log_model_usage(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<LogModelUsageBody>, JsonRejection>,
) -> Result<(StatusCode, Json<UsageRecord>), DomainError> {
    let body = parse_body(body)?;
    let input = UsageInput {
        quantity: body.quantity,
        notes: body.notes,
        used_at: body.used_at,
    };
    let record = st.service.record_usage(body.part_id, id, input).await?;
    Ok((StatusCode::CREATED, Json(record)))
}

// ---------------------------------------------------------------------------
// Model <-> catalog link
// ---------------------------------------------------------------------------

/// Links (or re-points) a model to a catalog model. POST with replace
/// semantics rather than PUT: there is no body-shaped resource to replace —
/// the link is single-valued, so "replace" is simply "set to this value" —
/// which keeps the endpoint action-shaped like the part link endpoints
/// while staying idempotent when the same catalog model is re-linked.
async fn link_catalog(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<LinkCatalogBody>, JsonRejection>,
) -> Result<Json<Model>, DomainError> {
    let body = parse_body(body)?;
    Ok(Json(
        st.service
            .link_model_catalog(id, body.catalog_model_id)
            .await?,
    ))
}

async fn unlink_catalog(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, DomainError> {
    st.service.unlink_model_catalog(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Reference catalog
// ---------------------------------------------------------------------------

async fn list_catalog_manufacturers(
    State(st): State<AppState>,
) -> Result<Json<Vec<CatalogManufacturer>>, DomainError> {
    Ok(Json(st.service.list_catalog_manufacturers().await?))
}

async fn list_catalog_models(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<CatalogModel>>, DomainError> {
    Ok(Json(st.service.list_catalog_models(id).await?))
}

async fn get_catalog_model(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    filter: Result<Query<CatalogModelFilter>, QueryRejection>,
) -> Result<Json<CatalogModelDetail>, DomainError> {
    let filter = parse_query(filter)?;
    Ok(Json(
        st.service
            .get_catalog_model_detail(id, filter.model_id)
            .await?,
    ))
}

/// Catalog part search across all models (name / part number / notes),
/// joined with the model and manufacturer so the part-detail link picker
/// can render each hit without extra round trips.
async fn search_catalog_parts(
    State(st): State<AppState>,
    filter: Result<Query<CatalogPartSearchFilter>, QueryRejection>,
) -> Result<Json<Vec<CatalogPartSearchHit>>, DomainError> {
    let filter = parse_query(filter)?;
    Ok(Json(
        st.service.search_catalog_parts(filter.q.as_deref()).await?,
    ))
}

/// Explicit admin deletion of a catalog part (typically an orphan left
/// behind by a re-import). Inventory parts keep existing; their
/// `catalog_part_id` trace link becomes NULL.
async fn delete_catalog_part(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, DomainError> {
    st.service.delete_catalog_part(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// One-click "add this catalog part to my inventory". Creates the part
/// pre-filled from the catalog entry (name, part_number -> `link`,
/// catalog trace link) and links it to the model; if that catalog part is
/// already tied to an inventory part on the model, the existing part's
/// quantity is adjusted instead (clamped at 0).
async fn add_to_inventory(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Result<Json<AddToInventoryBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Part>), DomainError> {
    let body = parse_body(body)?;
    let (created, part) = st
        .service
        .add_catalog_part_to_inventory(id, body.model_id, body.quantity)
        .await?;
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(part),
    ))
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

async fn get_settings(State(st): State<AppState>) -> Result<Json<Settings>, DomainError> {
    Ok(Json(st.service.get_settings().await?))
}

async fn update_settings(
    State(st): State<AppState>,
    input: Result<Json<Settings>, JsonRejection>,
) -> Result<Json<Settings>, DomainError> {
    let input = parse_body(input)?;
    Ok(Json(st.service.update_settings(input).await?))
}
