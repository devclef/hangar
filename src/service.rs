//! Business logic: input validation, existence checks, and the
//! model<->part association rules. The repo does raw data access;
//! this layer is what the API handlers talk to.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::catalog::{self, CatalogImportResult, CatalogImportSummary};
use crate::error::{DomainError, NotFound};
use crate::repo::sqlite::SqliteRepo;
use crate::repo::HangarRepo;
use crate::types::{
    CatalogLinkedModel, CatalogManufacturer, CatalogModel, CatalogModelDetail, CatalogModelSummary,
    CatalogPartSearchHit, CatalogPartView, Model, ModelDetail, ModelInput, Part, PartBulkEdit,
    PartDetail, PartInput, PartListRow, PartSort, Settings, UsageInput, UsageRecord,
};

#[async_trait]
pub trait ServiceApi: Send + Sync {
    // -- Models -------------------------------------------------------------
    async fn list_models(
        &self,
        q: Option<&str>,
        category: Option<crate::types::Category>,
    ) -> Result<Vec<crate::types::ModelListRow>, DomainError>;
    async fn get_model_detail(&self, id: i64) -> Result<ModelDetail, DomainError>;
    async fn create_model(&self, input: ModelInput) -> Result<Model, DomainError>;
    async fn update_model(&self, id: i64, input: ModelInput) -> Result<Model, DomainError>;
    async fn delete_model(&self, id: i64) -> Result<(), DomainError>;

    // -- Parts --------------------------------------------------------------
    async fn list_parts(
        &self,
        q: Option<&str>,
        sort: PartSort,
    ) -> Result<Vec<PartListRow>, DomainError>;
    async fn get_part_detail(&self, id: i64) -> Result<PartDetail, DomainError>;
    /// Links an existing inventory part to a reference catalog part (the
    /// `parts.catalog_part_id` trace link). Re-linking replaces the target.
    /// Both the part and the catalog part must exist (404 otherwise).
    async fn link_part_catalog(
        &self,
        part_id: i64,
        catalog_part_id: i64,
    ) -> Result<Part, DomainError>;
    /// Removes the part's catalog trace link. 404 when the part has none.
    async fn unlink_part_catalog(&self, part_id: i64) -> Result<(), DomainError>;
    async fn create_part(&self, input: PartInput) -> Result<Part, DomainError>;
    async fn update_part(&self, id: i64, input: PartInput) -> Result<Part, DomainError>;
    async fn delete_part(&self, id: i64) -> Result<(), DomainError>;
    async fn set_quantity(&self, id: i64, quantity: i64) -> Result<Part, DomainError>;
    async fn adjust_quantity(&self, id: i64, delta: i64) -> Result<Part, DomainError>;
    /// Bulk-edit several parts at once: tri-state field updates plus
    /// optional model link/unlink. Returns the updated list rows.
    async fn bulk_edit_parts(&self, input: PartBulkEdit) -> Result<Vec<PartListRow>, DomainError>;

    // -- Association ----------------------------------------------------------
    async fn list_model_parts(&self, model_id: i64) -> Result<Vec<PartListRow>, DomainError>;
    async fn list_part_models(&self, part_id: i64) -> Result<Vec<Model>, DomainError>;
    async fn link_part(&self, model_id: i64, part_id: i64) -> Result<(), DomainError>;
    async fn unlink_part(&self, model_id: i64, part_id: i64) -> Result<(), DomainError>;
    async fn replace_model_parts(
        &self,
        model_id: i64,
        part_ids: Vec<i64>,
    ) -> Result<Vec<PartListRow>, DomainError>;

    // -- Part usage log -----------------------------------------------------

    /// Usage entries, latest first; either filter may be `None` for "any".
    async fn list_usage(
        &self,
        part_id: Option<i64>,
        model_id: Option<i64>,
    ) -> Result<Vec<UsageRecord>, DomainError>;
    /// Records that `quantity` units of `part_id` were used on `model_id`
    /// and decrements the part's stock by the same amount (clamped at 0).
    async fn record_usage(
        &self,
        part_id: i64,
        model_id: i64,
        input: UsageInput,
    ) -> Result<UsageRecord, DomainError>;

    // -- Settings -----------------------------------------------------------

    /// Returns the stored settings, or the built-in defaults when none exist.
    async fn get_settings(&self) -> Result<Settings, DomainError>;
    /// Validates and stores the full settings document; returns what was stored.
    async fn update_settings(&self, settings: Settings) -> Result<Settings, DomainError>;

    // -- Reference catalog ----------------------------------------------------

    async fn list_catalog_manufacturers(&self) -> Result<Vec<CatalogManufacturer>, DomainError>;
    async fn list_catalog_models(
        &self,
        manufacturer_id: i64,
    ) -> Result<Vec<CatalogModel>, DomainError>;
    /// `scope_model_id` (optional) restricts each part's `owned_quantity` to
    /// the inventory tied to that one user model; omitted means "sum over all
    /// user models linked to this catalog model". When no user model is
    /// linked (or the scoped model isn't), every quantity is `None`.
    async fn get_catalog_model_detail(
        &self,
        id: i64,
        scope_model_id: Option<i64>,
    ) -> Result<CatalogModelDetail, DomainError>;
    /// Links (or re-points) a user model to a catalog model. POST with
    /// replace semantics: the link is single-valued, so replacing it is a
    /// full replace — same family of semantics as `PUT`, but expressed as an
    /// action because there is no "catalog link" body to PUT.
    async fn link_model_catalog(
        &self,
        model_id: i64,
        catalog_model_id: i64,
    ) -> Result<Model, DomainError>;
    async fn unlink_model_catalog(&self, model_id: i64) -> Result<(), DomainError>;
    /// Explicit admin deletion of a catalog part (orphan cleanup). Inventory
    /// parts keep existing; their trace link is set to NULL.
    async fn delete_catalog_part(&self, id: i64) -> Result<(), DomainError>;
    /// Searches catalog parts across all models by name, part number, or
    /// notes (case-insensitive substring); empty/blank query lists the
    /// first 100 parts in name order.
    async fn search_catalog_parts(
        &self,
        q: Option<&str>,
    ) -> Result<Vec<CatalogPartSearchHit>, DomainError>;
    /// Adds a catalog part to a user model's inventory: creates the part
    /// pre-filled from the catalog entry, or — when the same catalog part is
    /// already tied to an inventory part on that model — adjusts that part's
    /// quantity by `quantity` (delta semantics, clamped at 0). Returns the
    /// part and whether it was newly created.
    async fn add_catalog_part_to_inventory(
        &self,
        catalog_part_id: i64,
        model_id: i64,
        quantity: Option<i64>,
    ) -> Result<(bool, Part), DomainError>;

    // -- Catalog import ---------------------------------------------------------

    /// Imports one catalog file (see `catalog-data/README.md`). Used at
    /// startup, by the `import-catalog` CLI, and by tests.
    async fn import_catalog_file(
        &self,
        path: &std::path::Path,
    ) -> Result<CatalogImportResult, DomainError>;
    /// Imports every `*.json` under `root`; per-file failures are collected,
    /// never fatal.
    async fn import_catalog_dir(
        &self,
        root: &std::path::Path,
    ) -> Result<CatalogImportSummary, DomainError>;
}

/// The concrete, trait-backed service used by the app and tests.
#[derive(Clone)]
pub struct Service {
    repo: Arc<dyn HangarRepo>,
}

impl Service {
    pub fn new(repo: Arc<dyn HangarRepo>) -> Self {
        Self { repo }
    }

    /// Build a service backed by a SQLite pool (production + tests).
    pub fn from_sqlite(pool: SqlitePool) -> Self {
        Self::new(Arc::new(SqliteRepo::new(pool)))
    }

    fn require_model(&self, model: Option<Model>, id: i64) -> Result<Model, DomainError> {
        model.ok_or_else(|| DomainError::NotFound(NotFound::Model(id)))
    }

    fn require_part(&self, part: Option<Part>, id: i64) -> Result<Part, DomainError> {
        part.ok_or_else(|| DomainError::NotFound(NotFound::Part(id)))
    }

    async fn verify_parts_exist(&self, ids: &[i64]) -> Result<(), DomainError> {
        for id in ids {
            if self.repo.get_part(*id).await?.is_none() {
                return Err(DomainError::NotFound(NotFound::Part(*id)));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ServiceApi for Service {
    async fn list_models(
        &self,
        q: Option<&str>,
        category: Option<crate::types::Category>,
    ) -> Result<Vec<crate::types::ModelListRow>, DomainError> {
        self.repo.list_models(q, category).await
    }

    async fn get_model_detail(&self, id: i64) -> Result<ModelDetail, DomainError> {
        let model = self.require_model(self.repo.get_model(id).await?, id)?;
        let parts = self.repo.list_model_parts(id).await?;
        // Embedded summary only (no parts list): the full catalog parts stay
        // on GET /api/catalog/models/:id so this hot path stays cheap.
        let catalog = match model.catalog_model_id {
            Some(cm_id) => {
                self.repo
                    .get_catalog_model(cm_id)
                    .await?
                    .map(|cm| CatalogModelSummary {
                        catalog_model_name: cm.name,
                        diagram_asset: cm.diagram_asset,
                    })
            }
            None => None,
        };
        Ok(ModelDetail {
            model,
            parts,
            catalog,
        })
    }

    async fn create_model(&self, input: ModelInput) -> Result<Model, DomainError> {
        let input = input.validate()?;
        self.repo.create_model(&input).await
    }

    async fn update_model(&self, id: i64, input: ModelInput) -> Result<Model, DomainError> {
        let input = input.validate()?;
        self.require_part_model_update(id, input).await
    }

    async fn delete_model(&self, id: i64) -> Result<(), DomainError> {
        if !self.repo.delete_model(id).await? {
            return Err(DomainError::NotFound(NotFound::Model(id)));
        }
        Ok(())
    }

    async fn list_parts(
        &self,
        q: Option<&str>,
        sort: PartSort,
    ) -> Result<Vec<PartListRow>, DomainError> {
        self.repo.list_parts(q, sort).await
    }

    async fn get_part_detail(&self, id: i64) -> Result<PartDetail, DomainError> {
        let part = self.require_part(self.repo.get_part(id).await?, id)?;
        let models = self.repo.list_part_models(id).await?;
        // Embedded catalog summary (no full model payload): the detail page
        // shows the "catalog" section and offers unlink without a second
        // round trip. Can't dangle — deleting a catalog part SETs the
        // part's link to NULL at the schema level.
        let catalog = match part.catalog_part_id {
            Some(cp_id) => self.repo.get_catalog_part_link(cp_id).await?,
            None => None,
        };
        Ok(PartDetail {
            part,
            models,
            catalog,
        })
    }

    async fn link_part_catalog(
        &self,
        part_id: i64,
        catalog_part_id: i64,
    ) -> Result<Part, DomainError> {
        self.require_part(self.repo.get_part(part_id).await?, part_id)?;
        self.repo
            .get_catalog_part(catalog_part_id)
            .await?
            .ok_or_else(|| DomainError::NotFound(NotFound::CatalogPart(catalog_part_id)))?;
        let part = self
            .repo
            .set_part_catalog_link(part_id, Some(catalog_part_id))
            .await?;
        self.require_part(part, part_id)
    }

    async fn unlink_part_catalog(&self, part_id: i64) -> Result<(), DomainError> {
        let part = self.require_part(self.repo.get_part(part_id).await?, part_id)?;
        if part.catalog_part_id.is_none() {
            return Err(DomainError::NotFound(NotFound::PartCatalogLink { part_id }));
        }
        self.repo.set_part_catalog_link(part_id, None).await?;
        Ok(())
    }

    async fn create_part(&self, input: PartInput) -> Result<Part, DomainError> {
        let input = input.validate()?;
        self.repo.create_part(&input).await
    }

    async fn update_part(&self, id: i64, input: PartInput) -> Result<Part, DomainError> {
        let input = input.validate()?;
        let part = self.repo.update_part(id, &input).await?;
        self.require_part(part, id)
    }

    async fn delete_part(&self, id: i64) -> Result<(), DomainError> {
        if !self.repo.delete_part(id).await? {
            return Err(DomainError::NotFound(NotFound::Part(id)));
        }
        Ok(())
    }

    async fn set_quantity(&self, id: i64, quantity: i64) -> Result<Part, DomainError> {
        if quantity < 0 {
            return Err(DomainError::Invalid(
                "quantity: must be zero or a positive integer".into(),
            ));
        }
        if quantity > i32::MAX as i64 {
            return Err(DomainError::Invalid("quantity: too large".into()));
        }
        let part = self.repo.set_quantity(id, quantity as i32).await?;
        self.require_part(part, id)
    }

    async fn adjust_quantity(&self, id: i64, delta: i64) -> Result<Part, DomainError> {
        if delta == 0 {
            return Err(DomainError::Invalid("delta: must be non-zero".into()));
        }
        // Guard against overflow of the i64 intermediate in SQL (quantity + delta).
        if delta.abs() > i64::MAX / 4 {
            return Err(DomainError::Invalid("delta: too large".into()));
        }
        let part = self.repo.adjust_quantity(id, delta).await?;
        self.require_part(part, id)
    }

    async fn bulk_edit_parts(&self, input: PartBulkEdit) -> Result<Vec<PartListRow>, DomainError> {
        let input = input.validate()?;
        self.verify_parts_exist(&input.part_ids).await?;
        if let Some(model_id) = input.model_id {
            self.require_model(self.repo.get_model(model_id).await?, model_id)?;
        }
        for model_id in &input.unlink_model_ids {
            self.require_model(self.repo.get_model(*model_id).await?, *model_id)?;
        }
        self.repo.bulk_edit_parts(&input.part_ids, &input).await?;
        self.repo.list_parts_by_ids(&input.part_ids).await
    }

    async fn list_model_parts(&self, model_id: i64) -> Result<Vec<PartListRow>, DomainError> {
        if self.repo.get_model(model_id).await?.is_none() {
            return Err(DomainError::NotFound(NotFound::Model(model_id)));
        }
        self.repo.list_model_parts(model_id).await
    }

    async fn list_part_models(&self, part_id: i64) -> Result<Vec<Model>, DomainError> {
        if self.repo.get_part(part_id).await?.is_none() {
            return Err(DomainError::NotFound(NotFound::Part(part_id)));
        }
        self.repo.list_part_models(part_id).await
    }

    async fn link_part(&self, model_id: i64, part_id: i64) -> Result<(), DomainError> {
        if self.repo.get_model(model_id).await?.is_none() {
            return Err(DomainError::NotFound(NotFound::Model(model_id)));
        }
        if self.repo.get_part(part_id).await?.is_none() {
            return Err(DomainError::NotFound(NotFound::Part(part_id)));
        }
        // add_link is INSERT OR IGNORE, so re-linking is an idempotent no-op.
        self.repo.add_link(model_id, part_id).await
    }

    async fn unlink_part(&self, model_id: i64, part_id: i64) -> Result<(), DomainError> {
        let removed = self.repo.remove_link(model_id, part_id).await?;
        if !removed {
            return Err(DomainError::NotFound(NotFound::Link { model_id, part_id }));
        }
        Ok(())
    }

    async fn replace_model_parts(
        &self,
        model_id: i64,
        part_ids: Vec<i64>,
    ) -> Result<Vec<PartListRow>, DomainError> {
        if self.repo.get_model(model_id).await?.is_none() {
            return Err(DomainError::NotFound(NotFound::Model(model_id)));
        }
        let unique: BTreeSet<i64> = part_ids.into_iter().collect();
        let ids: Vec<i64> = unique.into_iter().collect();
        self.verify_parts_exist(&ids).await?;
        self.repo.replace_links(model_id, &ids).await?;
        self.repo.list_model_parts(model_id).await
    }

    async fn list_usage(
        &self,
        part_id: Option<i64>,
        model_id: Option<i64>,
    ) -> Result<Vec<UsageRecord>, DomainError> {
        self.repo.list_usage(part_id, model_id).await
    }

    async fn record_usage(
        &self,
        part_id: i64,
        model_id: i64,
        input: UsageInput,
    ) -> Result<UsageRecord, DomainError> {
        self.require_part(self.repo.get_part(part_id).await?, part_id)?;
        self.require_model(self.repo.get_model(model_id).await?, model_id)?;
        let (quantity, notes, used_at) = input.validate()?;
        self.repo
            .add_usage(
                part_id,
                model_id,
                quantity,
                notes.as_deref(),
                used_at.as_deref(),
            )
            .await
    }

    async fn get_settings(&self) -> Result<Settings, DomainError> {
        Ok(self.repo.get_settings().await?.unwrap_or_default())
    }

    async fn update_settings(&self, settings: Settings) -> Result<Settings, DomainError> {
        let settings = settings.validate()?;
        self.repo.save_settings(&settings).await?;
        Ok(settings)
    }

    async fn list_catalog_manufacturers(&self) -> Result<Vec<CatalogManufacturer>, DomainError> {
        self.repo.list_catalog_manufacturers().await
    }

    async fn list_catalog_models(
        &self,
        manufacturer_id: i64,
    ) -> Result<Vec<CatalogModel>, DomainError> {
        if !self
            .repo
            .catalog_manufacturer_exists(manufacturer_id)
            .await?
        {
            return Err(DomainError::NotFound(NotFound::CatalogManufacturer(
                manufacturer_id,
            )));
        }
        self.repo.list_catalog_models(manufacturer_id).await
    }

    async fn get_catalog_model_detail(
        &self,
        id: i64,
        scope_model_id: Option<i64>,
    ) -> Result<CatalogModelDetail, DomainError> {
        let model = self
            .repo
            .get_catalog_model(id)
            .await?
            .ok_or_else(|| DomainError::NotFound(NotFound::CatalogModel(id)))?;

        let linked = self.repo.list_models_for_catalog_model(id).await?;

        // Which user model(s) do owned quantities count against?
        let scope_ids: Vec<i64> = match scope_model_id {
            Some(mid) => match self.repo.get_model(mid).await? {
                Some(m) if m.catalog_model_id == Some(id) => vec![mid],
                // Exists but not linked to this catalog model: treat as
                // "no link" (all quantities null) rather than erroring — the
                // link can go stale between page loads.
                Some(_) => Vec::new(),
                None => return Err(DomainError::NotFound(NotFound::Model(mid))),
            },
            None => linked.iter().map(|(mid, _)| *mid).collect(),
        };

        let quantities = if scope_ids.is_empty() {
            None
        } else {
            Some(self.repo.catalog_owned_quantities(id, &scope_ids).await?)
        };

        let parts = self.repo.list_catalog_parts(id).await?;
        let views: Vec<CatalogPartView> = parts
            .into_iter()
            .map(|part| CatalogPartView {
                owned_quantity: quantities.as_ref().and_then(|q| q.get(&part.id)).copied(),
                part,
            })
            .collect();
        let linked_models = linked
            .into_iter()
            .map(|(mid, name)| CatalogLinkedModel { id: mid, name })
            .collect();

        Ok(CatalogModelDetail {
            diagram_asset: model.diagram_asset.clone(),
            linked_models,
            parts: views,
            model,
        })
    }

    async fn link_model_catalog(
        &self,
        model_id: i64,
        catalog_model_id: i64,
    ) -> Result<Model, DomainError> {
        let model = self.require_model(self.repo.get_model(model_id).await?, model_id)?;
        let cm = self
            .repo
            .get_catalog_model(catalog_model_id)
            .await?
            .ok_or_else(|| DomainError::NotFound(NotFound::CatalogModel(catalog_model_id)))?;
        if cm.category != model.category {
            return Err(DomainError::Invalid(format!(
                "category mismatch: model {model_id} is `{}` but catalog model {} (\"{}\") is `{}` — a catalog model\'s parts are only meaningful for the same vehicle type",
                model.category, cm.id, cm.name, cm.category
            )));
        }
        // Replace semantics: the model had at most one catalog link, and
        // setting the same value again is a no-op — idempotent by nature.
        self.repo
            .set_model_catalog_link(model_id, Some(catalog_model_id))
            .await?;
        self.require_model(self.repo.get_model(model_id).await?, model_id)
    }

    async fn unlink_model_catalog(&self, model_id: i64) -> Result<(), DomainError> {
        let model = self.require_model(self.repo.get_model(model_id).await?, model_id)?;
        if model.catalog_model_id.is_none() {
            return Err(DomainError::NotFound(NotFound::CatalogLink { model_id }));
        }
        self.repo.set_model_catalog_link(model_id, None).await?;
        Ok(())
    }

    async fn delete_catalog_part(&self, id: i64) -> Result<(), DomainError> {
        if !self.repo.delete_catalog_part(id).await? {
            return Err(DomainError::NotFound(NotFound::CatalogPart(id)));
        }
        Ok(())
    }

    async fn search_catalog_parts(
        &self,
        q: Option<&str>,
    ) -> Result<Vec<CatalogPartSearchHit>, DomainError> {
        let q = q.map(str::trim).filter(|t| !t.is_empty());
        self.repo.search_catalog_parts(q, 100).await
    }

    async fn add_catalog_part_to_inventory(
        &self,
        catalog_part_id: i64,
        model_id: i64,
        quantity: Option<i64>,
    ) -> Result<(bool, Part), DomainError> {
        let cp = self
            .repo
            .get_catalog_part(catalog_part_id)
            .await?
            .ok_or_else(|| DomainError::NotFound(NotFound::CatalogPart(catalog_part_id)))?;
        self.require_model(self.repo.get_model(model_id).await?, model_id)?;

        let delta = quantity.unwrap_or(1);
        // Idempotency: the same catalog part tied to an inventory part that
        // is already linked to this model is ADJUSTED (delta semantics,
        // clamped at 0 by the repo SQL — same as POST /api/parts/:id/quantity),
        // never duplicated.
        if let Some(existing) = self
            .repo
            .find_linked_inventory_part(catalog_part_id, model_id)
            .await?
        {
            if delta == 0 {
                return Err(DomainError::Invalid("quantity: must be non-zero".into()));
            }
            if delta.abs() > i64::MAX / 4 {
                return Err(DomainError::Invalid("quantity: too large".into()));
            }
            let part = self.require_part(
                self.repo.adjust_quantity(existing.id, delta).await?,
                existing.id,
            )?;
            return Ok((false, part));
        }

        // Fresh creation: quantity here is an absolute starting count (>= 0).
        if delta < 0 {
            return Err(DomainError::Invalid(
                "quantity: must be zero or a positive integer".into(),
            ));
        }
        if delta > i32::MAX as i64 {
            return Err(DomainError::Invalid(
                "quantity: too large (max 2147483647)".into(),
            ));
        }
        let part = self
            .repo
            .create_part_from_catalog(
                catalog_part_id,
                &cp.name,
                cp.part_number.as_deref(),
                delta as i32,
                model_id,
            )
            .await?;
        Ok((true, part))
    }

    async fn import_catalog_file(
        &self,
        path: &std::path::Path,
    ) -> Result<CatalogImportResult, DomainError> {
        // Anchor the source_file identity: a file inside the default catalog
        // directory keeps its repo-relative path (so CLI and startup imports
        // of the same file short-circuit against the same stored row);
        // anything else is identified by its bare file name.
        let root = match path.canonicalize() {
            Ok(canon) => match catalog::default_catalog_dir().canonicalize() {
                // Canonical on both sides so the stripped `source_file`
                // identity is the same whether the file is imported by the
                // startup scan, the CLI, or a test.
                Ok(defc) if canon.starts_with(&defc) => defc,
                _ => canon
                    .parent()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(".")),
            },
            Err(_) => path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
        };
        catalog::import_file(&*self.repo, &root, path).await
    }

    async fn import_catalog_dir(
        &self,
        root: &std::path::Path,
    ) -> Result<CatalogImportSummary, DomainError> {
        catalog::import_dir(&*self.repo, root).await
    }
}

// Private helper kept off the public trait.
impl Service {
    async fn require_part_model_update(
        &self,
        id: i64,
        input: ModelInput,
    ) -> Result<Model, DomainError> {
        let model = self.repo.update_model(id, &input).await?;
        self.require_model(model, id)
    }
}
