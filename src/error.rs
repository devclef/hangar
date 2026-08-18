//! Error types. `DomainError` is the single error type carried from the
//! service layer to the edge, where it maps to a structured JSON response.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("{0} not found")]
    NotFound(NotFound),
    #[error("{0}")]
    Invalid(String),
    #[error("internal database error")]
    Db(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotFound {
    Model(i64),
    Part(i64),
    Link { model_id: i64, part_id: i64 },
    CatalogManufacturer(i64),
    CatalogModel(i64),
    CatalogPart(i64),
    CatalogLink { model_id: i64 },
    PartCatalogLink { part_id: i64 },
}

impl fmt::Display for NotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotFound::Model(id) => write!(f, "model {id}"),
            NotFound::Part(id) => write!(f, "part {id}"),
            NotFound::Link { model_id, part_id } => {
                write!(f, "part {part_id} is not linked to model {model_id}")
            }
            NotFound::CatalogManufacturer(id) => write!(f, "catalog manufacturer {id}"),
            NotFound::CatalogModel(id) => write!(f, "catalog model {id}"),
            NotFound::CatalogPart(id) => write!(f, "catalog part {id}"),
            NotFound::CatalogLink { model_id } => {
                write!(f, "model {model_id} is not linked to a catalog model")
            }
            NotFound::PartCatalogLink { part_id } => {
                write!(f, "part {part_id} is not linked to a catalog part")
            }
        }
    }
}

impl From<sqlx::Error> for DomainError {
    fn from(e: sqlx::Error) -> Self {
        // RowNotFound is never an error path: callers use `fetch_optional`.
        DomainError::Db(anyhow::anyhow!(e))
    }
}

impl IntoResponse for DomainError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            DomainError::NotFound(what) => (StatusCode::NOT_FOUND, "not_found", what.to_string()),
            DomainError::Invalid(msg) => (StatusCode::BAD_REQUEST, "invalid_request", msg.clone()),
            DomainError::Db(err) => {
                tracing::error!(error = ?err, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal server error".to_string(),
                )
            }
        };
        let body = serde_json::json!({ "error": code, "message": message });
        (status, Json(body)).into_response()
    }
}
