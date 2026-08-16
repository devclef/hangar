//! Business logic: input validation, existence checks, and the
//! model<->part association rules. The repo does raw data access;
//! this layer is what the API handlers talk to.

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::error::{DomainError, NotFound};
use crate::repo::sqlite::SqliteRepo;
use crate::repo::HangarRepo;
use crate::types::{
    Model, ModelDetail, ModelInput, Part, PartBulkEdit, PartDetail, PartInput, PartListRow,
    PartSort, Settings, UsageInput, UsageRecord,
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
        Ok(ModelDetail { model, parts })
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
        Ok(PartDetail { part, models })
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
