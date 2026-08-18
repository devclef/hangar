//! Data access behind a trait so the storage engine can be swapped
//! (SQLite now, Postgres later) without touching service or route code.

pub mod sqlite;

use crate::catalog::{CatalogFile, CatalogImportResult};
use crate::error::DomainError;
use crate::types::{
    CatalogManufacturer, CatalogModel, CatalogPart, Category, Model, ModelInput, ModelListRow,
    Part, PartBulkEdit, PartDetail, PartInput, PartListRow, PartSort, Settings, UsageRecord,
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

    // -- Reference catalog ----------------------------------------------------

    /// Upserts a manufacturer/model/parts set from one parsed catalog file,
    /// keyed by `(manufacturer, model)`. Parts are matched by `part_number`
    /// when present, else by (case-insensitive) name. NEVER deletes rows:
    /// parts missing from the file are reported as orphans instead.
    async fn import_catalog_file(
        &self,
        source_file: &str,
        checksum: &str,
        file: &CatalogFile,
    ) -> Result<CatalogImportResult, DomainError>;
    /// Finds the catalog model imported from `source_file` whose stored
    /// checksum already equals `checksum` (re-import short-circuit).
    async fn find_catalog_model_by_source(
        &self,
        source_file: &str,
        checksum: &str,
    ) -> Result<Option<CatalogModel>, DomainError>;

    async fn list_catalog_manufacturers(&self) -> Result<Vec<CatalogManufacturer>, DomainError>;
    async fn catalog_manufacturer_exists(&self, id: i64) -> Result<bool, DomainError>;
    /// Empty list for an unknown manufacturer id (caller 404s via
    /// `catalog_manufacturer_exists`).
    async fn list_catalog_models(
        &self,
        manufacturer_id: i64,
    ) -> Result<Vec<CatalogModel>, DomainError>;
    async fn get_catalog_model(&self, id: i64) -> Result<Option<CatalogModel>, DomainError>;
    async fn list_catalog_parts(
        &self,
        catalog_model_id: i64,
    ) -> Result<Vec<CatalogPart>, DomainError>;
    async fn get_catalog_part(&self, id: i64) -> Result<Option<CatalogPart>, DomainError>;
    /// Explicit admin deletion of one catalog part. Inventory parts keep
    /// existing; their `catalog_part_id` is set to NULL (schema-level
    /// `ON DELETE SET NULL`).
    async fn delete_catalog_part(&self, id: i64) -> Result<bool, DomainError>;

    /// Sets (or clears, with `None`) the user model's catalog link.
    async fn set_model_catalog_link(
        &self,
        model_id: i64,
        catalog_model_id: Option<i64>,
    ) -> Result<(), DomainError>;

    /// The inventory part tied to `catalog_part_id` that is linked to
    /// `model_id`, if any (add-to-inventory idempotency check).
    async fn find_linked_inventory_part(
        &self,
        catalog_part_id: i64,
        model_id: i64,
    ) -> Result<Option<Part>, DomainError>;
    /// Creates an inventory part pre-filled from a catalog entry and links
    /// it to the model in one transaction.
    async fn create_part_from_catalog(
        &self,
        catalog_part_id: i64,
        name: &str,
        link: Option<&str>,
        quantity: i32,
        model_id: i64,
    ) -> Result<Part, DomainError>;
    /// For every catalog part of `catalog_model_id`, the sum of quantities
    /// of inventory parts tied to it that are linked to one of `model_ids`.
    /// An empty `model_ids` yields an empty map.
    async fn catalog_owned_quantities(
        &self,
        catalog_model_id: i64,
        model_ids: &[i64],
    ) -> Result<std::collections::BTreeMap<i64, i32>, DomainError>;
    /// The user models linked to a catalog model, ordered by name.
    async fn list_models_for_catalog_model(
        &self,
        catalog_model_id: i64,
    ) -> Result<Vec<(i64, String)>, DomainError>;
    /// `(model id, model name, source_file)` for every catalog model — used
    /// at import time to warn about source files that no longer exist.
    async fn list_catalog_model_sources(&self) -> Result<Vec<(i64, String, String)>, DomainError>;
}

/// Convenience for tests and app wiring.
pub fn detail_from_rows(part: Part, models: Vec<Model>) -> PartDetail {
    PartDetail { part, models }
}
