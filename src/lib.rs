//! Hangar — self-hosted RC hobby inventory tracker (backend library).
pub mod error;
pub mod repo;
pub mod routes;
pub mod service;
pub mod types;
pub mod web;

pub use routes::{router, AppState};
