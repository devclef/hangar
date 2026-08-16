//! Data access behind a trait so the storage engine can be swapped
//! (SQLite now, Postgres later) without touching service or route code.

pub mod sqlite;

use crate::error::DomainError;
use crate::types::{
    Category, Model, ModelInput, ModelListRow, Part, PartBulkEdit, PartDetail, PartInput,
    PartListRow, PartSort, Settings, UsageRecord,
};
use async_trait::async_trait;

#[async_trait]
pub trait HangarRepo: Send + Sync {
    // -- Models -------------------------------------------------------------

    async fn list_models(
        &self,
        q: Option<&str>,
        category: Option<Category>,
    ) -> Result<Vec<ModelListRow>, DomainError>;
    async fn get_model(&self, id: i64) -> Result<Option<Model>, DomainError>;
    async fn create_model(&self, input: &ModelInput) -> Result<Model, DomainError>;
    async fn update_model(&self, id: i64, input: &ModelInput)
        -> Result<Option<Model>, DomainError>;
    /// Returns true if a row was deleted, false if the model did not exist.
    async fn delete_model(&self, id: i64) -> Result<bool, DomainError>;

    // -- Parts --------------------------------------------------------------

    async fn list_parts(
        &self,
        q: Option<&str>,
        sort: PartSort,
    ) -> Result<Vec<PartListRow>, DomainError>;
    async fn get_part(&self, id: i64) -> Result<Option<Part>, DomainError>;
    async fn create_part(&self, input: &PartInput) -> Result<Part, DomainError>;
    async fn update_part(&self, id: i64, input: &PartInput) -> Result<Option<Part>, DomainError>;
    async fn delete_part(&self, id: i64) -> Result<bool, DomainError>;
    /// Absolute set; caller must have validated bounds.
    async fn set_quantity(&self, id: i64, quantity: i32) -> Result<Option<Part>, DomainError>;
    /// Atomic relative change, clamped at 0 in SQL.
    async fn adjust_quantity(&self, id: i64, delta: i64) -> Result<Option<Part>, DomainError>;
    /// Applies a validated bulk edit (field updates + model link changes) to
    /// the given parts in a single transaction. Parts must exist
    /// (caller-checked); an unknown model is a caller error too.
    async fn bulk_edit_parts(
        &self,
        part_ids: &[i64],
        input: &PartBulkEdit,
    ) -> Result<(), DomainError>;
    /// Part list rows for exactly these ids, in id order. Empty input
    /// yields an empty list.
    async fn list_parts_by_ids(&self, part_ids: &[i64]) -> Result<Vec<PartListRow>, DomainError>;

    // -- Model <-> part association -----------------------------------------

    async fn list_model_parts(&self, model_id: i64) -> Result<Vec<PartListRow>, DomainError>;
    /// Idempotent: linking an already-linked part is a no-op.
    async fn add_link(&self, model_id: i64, part_id: i64) -> Result<(), DomainError>;
    /// Returns true if a link row was removed.
    async fn remove_link(&self, model_id: i64, part_id: i64) -> Result<bool, DomainError>;
    /// Replaces the model's full set of linked parts (deduped by caller).
    async fn replace_links(&self, model_id: i64, part_ids: &[i64]) -> Result<(), DomainError>;
    async fn list_part_models(&self, part_id: i64) -> Result<Vec<Model>, DomainError>;

    // -- Part usage log -----------------------------------------------------

    /// Latest entries first; either filter may be `None` for "any".
    async fn list_usage(
        &self,
        part_id: Option<i64>,
        model_id: Option<i64>,
    ) -> Result<Vec<UsageRecord>, DomainError>;
    /// Inserts the entry and decrements the part's stock (clamped at 0) in a
    /// single transaction. `used_at` is `None` when "now" should be stamped.
    async fn add_usage(
        &self,
        part_id: i64,
        model_id: i64,
        quantity: i32,
        notes: Option<&str>,
        used_at: Option<&str>,
    ) -> Result<UsageRecord, DomainError>;

    // -- Settings -----------------------------------------------------------

    /// Returns `None` when no settings have been stored yet, or when the
    /// stored document no longer parses (e.g. it references a removed field).
    async fn get_settings(&self) -> Result<Option<Settings>, DomainError>;
    /// Upserts the full settings document.
    async fn save_settings(&self, settings: &Settings) -> Result<(), DomainError>;
}

/// Convenience for tests and app wiring.
pub fn detail_from_rows(part: Part, models: Vec<Model>) -> PartDetail {
    PartDetail { part, models }
}
