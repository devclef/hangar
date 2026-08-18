//! Domain types, input types, and small validation helpers.

use serde::{Deserialize, Serialize};
use sqlx::{decode::Decode, encode::Encode, sqlite::SqliteValueRef, Sqlite, Type};
use std::fmt;

/// Generates sqlx Type/Decode/Encode impls for a string-backed enum
/// (SQLite stores these as TEXT).
macro_rules! sqlx_enum {
    ($e:ty) => {
        impl Type<Sqlite> for $e {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <String as Type<Sqlite>>::type_info()
            }
        }

        impl<'r> Decode<'r, Sqlite> for $e {
            fn decode(
                value: SqliteValueRef<'r>,
            ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                let s = <String as Decode<Sqlite>>::decode(value)?;
                s.parse()
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                    })
            }
        }

        impl Encode<'_, Sqlite> for $e {
            fn encode_by_ref(
                &self,
                buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'_>>,
            ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                <String as Encode<Sqlite>>::encode(self.to_string(), buf)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Heli,
    Plane,
    Car,
    Drone,
    Boat,
    Other,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Category::Heli => "heli",
            Category::Plane => "plane",
            Category::Car => "car",
            Category::Drone => "drone",
            Category::Boat => "boat",
            Category::Other => "other",
        })
    }
}

impl std::str::FromStr for Category {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "heli" => Ok(Category::Heli),
            "plane" => Ok(Category::Plane),
            "car" => Ok(Category::Car),
            "drone" => Ok(Category::Drone),
            "boat" => Ok(Category::Boat),
            "other" => Ok(Category::Other),
            other => Err(format!(
                "unknown category `{other}` (expected heli, plane, car, drone, boat, or other)"
            )),
        }
    }
}

sqlx_enum!(Category);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelStatus {
    Active,
    Retired,
    Sold,
}

impl fmt::Display for ModelStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ModelStatus::Active => "active",
            ModelStatus::Retired => "retired",
            ModelStatus::Sold => "sold",
        })
    }
}

impl std::str::FromStr for ModelStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(ModelStatus::Active),
            "retired" => Ok(ModelStatus::Retired),
            "sold" => Ok(ModelStatus::Sold),
            other => Err(format!(
                "unknown status `{other}` (expected active, retired, or sold)"
            )),
        }
    }
}

sqlx_enum!(ModelStatus);

/// UI color theme. `System` follows the OS preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        })
    }
}

impl std::str::FromStr for Theme {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(Theme::System),
            "light" => Ok(Theme::Light),
            "dark" => Ok(Theme::Dark),
            other => Err(format!(
                "unknown theme `{other}` (expected system, light, or dark)"
            )),
        }
    }
}

sqlx_enum!(Theme);

/// A part field the user may choose to show or hide on the "add part" form.
/// The serialized name matches the `parts` column, so the wire value and the
/// database column line up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartFormField {
    Quantity,
    Cost,
    Vendor,
    Link,
    PhotoUrl,
    Notes,
}

impl PartFormField {
    /// Every optional part field, in the order the form lays them out.
    pub const ALL: [PartFormField; 6] = [
        PartFormField::Quantity,
        PartFormField::Cost,
        PartFormField::Vendor,
        PartFormField::Link,
        PartFormField::PhotoUrl,
        PartFormField::Notes,
    ];
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct Model {
    pub id: i64,
    pub name: String,
    pub category: Category,
    pub manufacturer: Option<String>,
    pub notes: Option<String>,
    pub date_acquired: Option<String>,
    pub status: ModelStatus,
    pub photo_url: Option<String>,
    /// Optional link to a reference catalog model. Managed exclusively via
    /// `POST/DELETE /api/models/:id/link-catalog` — `PUT /api/models/:id`
    /// does NOT touch it, so full-replace updates never wipe the link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_model_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Part {
    pub id: i64,
    pub name: String,
    pub quantity: i32,
    pub notes: Option<String>,
    pub link: Option<String>,
    pub photo_url: Option<String>,
    pub cost: Option<f64>,
    pub vendor: Option<String>,
    /// Whether the "low quantity" badge may appear for this part. Users
    /// commonly keep a single spare and opt such parts out.
    pub low_stock_enabled: bool,
}

/// A model as returned by list endpoints (includes how many parts are linked).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct ModelListRow {
    pub id: i64,
    pub name: String,
    pub category: Category,
    pub manufacturer: Option<String>,
    pub notes: Option<String>,
    pub date_acquired: Option<String>,
    pub status: ModelStatus,
    pub photo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_model_id: Option<i64>,
    pub part_count: i64,
}

impl ModelListRow {
    pub fn into_model(self) -> Model {
        Model {
            id: self.id,
            name: self.name,
            category: self.category,
            manufacturer: self.manufacturer,
            notes: self.notes,
            date_acquired: self.date_acquired,
            status: self.status,
            photo_url: self.photo_url,
            catalog_model_id: self.catalog_model_id,
        }
    }
}

/// A part as returned by list endpoints (includes linked-model summary).
/// `model_names` is a `'|'`-joined list of linked model names (NULL when none).
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct PartListRow {
    pub id: i64,
    pub name: String,
    pub quantity: i32,
    pub notes: Option<String>,
    pub link: Option<String>,
    pub photo_url: Option<String>,
    pub cost: Option<f64>,
    pub low_stock_enabled: bool,
    pub vendor: Option<String>,
    pub model_count: i64,
    pub model_names: Option<String>,
}

impl PartListRow {
    pub fn into_part(self) -> Part {
        Part {
            id: self.id,
            name: self.name,
            quantity: self.quantity,
            notes: self.notes,
            link: self.link,
            photo_url: self.photo_url,
            cost: self.cost,
            vendor: self.vendor,
            low_stock_enabled: self.low_stock_enabled,
        }
    }
}

/// Model detail payload: the model plus every part linked to it.
#[derive(Debug, Serialize)]
pub struct ModelDetail {
    pub model: Model,
    pub parts: Vec<PartListRow>,
    /// Set when the model is linked to a reference catalog model: a small
    /// summary (no full parts list — that stays on
    /// `GET /api/catalog/models/:id`) so the detail page can show the
    /// "known parts / diagram" section without guessing the id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<CatalogModelSummary>,
}

/// The embedded catalog summary on `GET /api/models/:id` (see `ModelDetail`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogModelSummary {
    pub catalog_model_name: String,
    /// The model's diagram override as stored (`null` → frontend falls back
    /// to the generic per-category SVG).
    pub diagram_asset: Option<String>,
}

// ---------------------------------------------------------------------------
// Reference catalog
// ---------------------------------------------------------------------------

/// A catalog manufacturer with how many catalog models it has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct CatalogManufacturer {
    pub id: i64,
    pub name: String,
    pub notes: Option<String>,
    pub model_count: i64,
}

/// A catalog model row (joined with its manufacturer's display name).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct CatalogModel {
    pub id: i64,
    pub manufacturer_id: i64,
    pub manufacturer: String,
    pub name: String,
    pub category: Category,
    /// Per-model diagram override (a file in `frontend/src/lib/diagrams/`);
    /// `None` → generic per-category SVG.
    pub diagram_asset: Option<String>,
    /// Catalog file this row was imported from (repo-relative).
    pub source_file: String,
    /// sha256 hex of that file's contents when it was last imported.
    pub source_checksum: String,
}

/// A catalog part: a known official part of a catalog model.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct CatalogPart {
    pub id: i64,
    pub catalog_model_id: i64,
    pub name: String,
    /// Official manufacturer part number; `None` when not verified yet.
    pub part_number: Option<String>,
    /// Free-text grouping for the legend (e.g. "Blade grip").
    pub category: Option<String>,
    pub notes: Option<String>,
    /// Hotspot position on the diagram, percentages 0-100; `None` when the
    /// part is not diagram-placeable.
    pub diagram_x: Option<f64>,
    pub diagram_y: Option<f64>,
}

/// A catalog part as returned by `GET /api/catalog/models/:id`, with the
/// live owned quantity (sum over the inventory parts tied to this catalog
/// part and linked to the linked user models) or `None` when no user model
/// is linked to the catalog model (or none of its parts is owned).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogPartView {
    #[serde(flatten)]
    pub part: CatalogPart,
    pub owned_quantity: Option<i32>,
}

/// A user model linked to a catalog model (drives owned quantities).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogLinkedModel {
    pub id: i64,
    pub name: String,
}

/// Catalog model detail: `{model, diagram_asset, linked_models[], parts[]}`.
#[derive(Debug, Serialize)]
pub struct CatalogModelDetail {
    pub model: CatalogModel,
    /// Effective diagram asset: the model's override when set, else `None`
    /// (the frontend falls back to `<category>-generic.svg`).
    pub diagram_asset: Option<String>,
    /// User models currently linked to this catalog model.
    pub linked_models: Vec<CatalogLinkedModel>,
    pub parts: Vec<CatalogPartView>,
}

/// Part detail payload: the part plus every model it is linked to.
#[derive(Debug, Serialize)]
pub struct PartDetail {
    pub part: Part,
    pub models: Vec<Model>,
}

/// One entry in the part-usage log: `quantity` units of a part were consumed
/// on a model (a repair, build, or swap) at `used_at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct UsageRecord {
    pub id: i64,
    pub part_id: i64,
    pub part_name: String,
    pub model_id: i64,
    pub model_name: String,
    pub model_category: Category,
    pub quantity: i32,
    pub notes: Option<String>,
    /// ISO `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`.
    pub used_at: String,
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// User preferences, stored as a single JSON document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Which optional part fields the "add part" form should show.
    pub part_form_fields: Vec<PartFormField>,
    /// ISO-4217 currency code used to display part costs (e.g. "USD").
    pub currency: String,
    /// Globally enable/disable the "low quantity" badge.
    pub low_stock_enabled: bool,
    /// A part is "low" when its quantity is at or below this value.
    pub low_stock_threshold: i32,
    /// UI color theme; `system` follows the OS preference.
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            part_form_fields: PartFormField::ALL.to_vec(),
            currency: "USD".to_string(),
            low_stock_enabled: true,
            low_stock_threshold: 2,
            theme: Theme::System,
        }
    }
}

impl Settings {
    /// Dedupes the field list (preserving order) and normalizes/validates the
    /// currency code. Returns the normalized settings on success.
    pub fn validate(mut self) -> Result<Self, crate::error::DomainError> {
        let mut seen = std::collections::HashSet::new();
        self.part_form_fields.retain(|f| seen.insert(*f));
        let currency = self.currency.trim().to_uppercase();
        if !(3..=8).contains(&currency.len())
            || !currency.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err(crate::error::DomainError::Invalid(
                "currency: must be a 3-8 character alphanumeric code (e.g. USD)".into(),
            ));
        }
        self.currency = currency;
        if self.low_stock_threshold < 0 || self.low_stock_threshold > 1000 {
            return Err(crate::error::DomainError::Invalid(
                "low_stock_threshold: must be between 0 and 1000".into(),
            ));
        }
        Ok(self)
    }
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ModelInput {
    pub name: String,
    pub category: Category,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub date_acquired: Option<String>,
    /// Defaults to `active` when omitted.
    #[serde(default)]
    pub status: Option<ModelStatus>,
    #[serde(default)]
    pub photo_url: Option<String>,
}

impl ModelInput {
    /// Trims fields, normalizes empty strings to `None`, and validates.
    /// Returns the normalized input on success.
    pub fn validate(mut self) -> Result<Self, crate::error::DomainError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(crate::error::DomainError::Invalid(
                "name: must not be empty".into(),
            ));
        }
        if name.len() > 200 {
            return Err(crate::error::DomainError::Invalid(
                "name: must be at most 200 characters".into(),
            ));
        }
        self.name = name.to_string();
        self.manufacturer = trim_opt(self.manufacturer);
        self.notes = trim_opt(self.notes);
        self.photo_url = trim_opt(self.photo_url);
        if let Some(date) = self.date_acquired.as_deref() {
            if !is_valid_iso_date(date) {
                return Err(crate::error::DomainError::Invalid(
                    "date_acquired: must be a valid date in YYYY-MM-DD format".into(),
                ));
            }
        }
        Ok(self)
    }

    pub fn status(&self) -> ModelStatus {
        self.status.unwrap_or(ModelStatus::Active)
    }
}

#[derive(Debug, Deserialize)]
pub struct PartInput {
    pub name: String,
    pub quantity: i64,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub photo_url: Option<String>,
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub vendor: Option<String>,
    /// Whether the "low quantity" badge may appear for this part.
    /// Defaults to `true` when omitted.
    #[serde(default = "default_true")]
    pub low_stock_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl PartInput {
    pub fn validate(mut self) -> Result<Self, crate::error::DomainError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(crate::error::DomainError::Invalid(
                "name: must not be empty".into(),
            ));
        }
        if name.len() > 200 {
            return Err(crate::error::DomainError::Invalid(
                "name: must be at most 200 characters".into(),
            ));
        }
        self.name = name.to_string();
        self.notes = trim_opt(self.notes);
        self.link = trim_opt(self.link);
        self.photo_url = trim_opt(self.photo_url);
        self.vendor = trim_opt(self.vendor);
        if self.quantity < 0 {
            return Err(crate::error::DomainError::Invalid(
                "quantity: must be zero or a positive integer".into(),
            ));
        }
        if self.quantity > i32::MAX as i64 {
            return Err(crate::error::DomainError::Invalid(
                "quantity: too large (max 2147483647)".into(),
            ));
        }
        if let Some(cost) = self.cost {
            if !cost.is_finite() || cost < 0.0 {
                return Err(crate::error::DomainError::Invalid(
                    "cost: must be a finite number, zero or more".into(),
                ));
            }
        }
        Ok(self)
    }
}

/// One editable field in a bulk edit. **`Skip`** (absent from the JSON)
/// leaves the field untouched, **`null`** clears it, and a **value**
/// overwrites it. A plain `Option<Option<T>>` cannot express this because
/// serde collapses a JSON `null` to the outer `None`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum BulkValue<T> {
    /// Not present on the wire: leave the field alone.
    #[default]
    Skip,
    /// `null` on the wire: clear the field.
    Clear,
    /// A value on the wire: overwrite the field.
    Set(T),
}

impl<T> BulkValue<T> {
    /// True when the field should be written (set or clear).
    pub fn is_present(&self) -> bool {
        !matches!(self, Self::Skip)
    }

    /// `None` for skip/clear, `Some(&value)` for a set.
    pub fn as_value(&self) -> Option<&T> {
        match self {
            Self::Set(v) => Some(v),
            Self::Skip | Self::Clear => None,
        }
    }
}

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for BulkValue<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Option::<T>::deserialize(deserializer)?;
        Ok(match value {
            None => Self::Clear,
            Some(v) => Self::Set(v),
        })
    }
}

impl<T: serde::Serialize> serde::Serialize for BulkValue<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Skip | Self::Clear => serializer.serialize_none(),
            Self::Set(v) => v.serialize(serializer),
        }
    }
}

/// Body for bulk-editing several parts at once. Every editable field is
/// tri-state (see [`BulkValue`]). `model_id` / `unlink_model_ids` manage
/// the model links of every selected part.
#[derive(Debug, Default, Deserialize)]
pub struct PartBulkEdit {
    /// Parts to update; at least one, duplicates collapse.
    pub part_ids: Vec<i64>,
    /// New quantity on hand for every selected part.
    #[serde(default)]
    pub quantity: BulkValue<i64>,
    #[serde(default)]
    pub cost: BulkValue<f64>,
    #[serde(default)]
    pub vendor: BulkValue<String>,
    #[serde(default)]
    pub link: BulkValue<String>,
    #[serde(default)]
    pub photo_url: BulkValue<String>,
    #[serde(default)]
    pub notes: BulkValue<String>,
    /// Whether the "low quantity" badge may appear for each selected part.
    #[serde(default)]
    pub low_stock_enabled: BulkValue<bool>,
    /// When present, link this model to every selected part (idempotent).
    #[serde(default)]
    pub model_id: Option<i64>,
    /// When non-empty, unlink these models from every selected part.
    /// Unlinking a model that is not linked to a part is a no-op.
    #[serde(default)]
    pub unlink_model_ids: Vec<i64>,
}

impl PartBulkEdit {
    /// True when at least one part field (as opposed to only link changes)
    /// should be written.
    pub fn has_field_updates(&self) -> bool {
        self.quantity.is_present()
            || self.cost.is_present()
            || self.vendor.is_present()
            || self.link.is_present()
            || self.photo_url.is_present()
            || self.notes.is_present()
            || self.low_stock_enabled.is_present()
    }

    /// Trims, dedupes ids, validates bounds, and normalizes strings that
    /// trim to empty into explicit clears. Returns the normalized input.
    pub fn validate(mut self) -> Result<Self, crate::error::DomainError> {
        if self.part_ids.is_empty() {
            return Err(crate::error::DomainError::Invalid(
                "part_ids: must not be empty".into(),
            ));
        }
        if self.part_ids.len() > 500 {
            return Err(crate::error::DomainError::Invalid(
                "part_ids: too many ids (max 500)".into(),
            ));
        }
        self.part_ids.sort_unstable();
        self.part_ids.dedup();
        if let BulkValue::Set(q) = &self.quantity {
            if *q < 0 {
                return Err(crate::error::DomainError::Invalid(
                    "quantity: must be zero or a positive integer".into(),
                ));
            }
            if *q > i32::MAX as i64 {
                return Err(crate::error::DomainError::Invalid(
                    "quantity: too large (max 2147483647)".into(),
                ));
            }
        }
        if let BulkValue::Set(c) = &self.cost {
            if !c.is_finite() || *c < 0.0 {
                return Err(crate::error::DomainError::Invalid(
                    "cost: must be a finite number, zero or more".into(),
                ));
            }
        }
        self.vendor = normalize_bulk_string(self.vendor);
        self.link = normalize_bulk_string(self.link);
        self.photo_url = normalize_bulk_string(self.photo_url);
        self.notes = normalize_bulk_string(self.notes);
        if !self.has_field_updates() && self.model_id.is_none() && self.unlink_model_ids.is_empty()
        {
            return Err(crate::error::DomainError::Invalid(
                "nothing to update: set at least one field or a model link change".into(),
            ));
        }
        self.unlink_model_ids.sort_unstable();
        self.unlink_model_ids.dedup();
        Ok(self)
    }
}

/// `Set(s)` that trims to empty becomes a clear; everything else is kept.
fn normalize_bulk_string(v: BulkValue<String>) -> BulkValue<String> {
    match v {
        BulkValue::Set(s) if !s.trim().is_empty() => BulkValue::Set(s.trim().to_string()),
        BulkValue::Set(_) => BulkValue::Clear,
        other => other,
    }
}

/// Body for recording a usage entry. The part/model id not present in the
/// body comes from the request path (`/parts/{id}/usage` vs
/// `/models/{id}/usage`), so only the variable part of the entry lives here.
#[derive(Debug, Default, Deserialize)]
pub struct UsageInput {
    /// Units consumed; defaults to 1.
    #[serde(default)]
    pub quantity: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Backdate; defaults to server "now" when omitted or empty.
    #[serde(default)]
    pub used_at: Option<String>,
}

impl UsageInput {
    /// Trims/normalizes and validates. Returns `(quantity, notes, used_at)`
    /// where `used_at` is `None` when the server clock should be used.
    pub fn validate(
        &self,
    ) -> Result<(i32, Option<String>, Option<String>), crate::error::DomainError> {
        let quantity = self.quantity.unwrap_or(1);
        if quantity < 1 {
            return Err(crate::error::DomainError::Invalid(
                "quantity: must be a positive integer".into(),
            ));
        }
        if quantity > i32::MAX as i64 {
            return Err(crate::error::DomainError::Invalid(
                "quantity: too large (max 2147483647)".into(),
            ));
        }
        let notes = trim_opt(self.notes.clone());
        let used_at = match self.used_at.as_deref() {
            Some(raw) => {
                let t = raw.trim();
                if t.is_empty() {
                    None
                } else {
                    let t = t.replacen(' ', "T", 1);
                    if !is_valid_iso_datetime(&t) {
                        return Err(crate::error::DomainError::Invalid(
                            "used_at: must be a date (YYYY-MM-DD) or datetime (YYYY-MM-DDTHH:MM:SS)"
                                .into(),
                        ));
                    }
                    Some(t)
                }
            }
            None => None,
        };
        Ok((quantity as i32, notes, used_at))
    }
}

fn trim_opt(mut s: Option<String>) -> Option<String> {
    match s {
        Some(ref v) if v.trim().is_empty() => {
            s = None;
        }
        Some(v) => {
            s = Some(v.trim().to_string());
        }
        None => {}
    }
    s
}

// ---------------------------------------------------------------------------
// Query parameters
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ModelListFilter {
    pub q: Option<String>,
    pub category: Option<Category>,
}

#[derive(Debug, Deserialize)]
pub struct UsageFilter {
    pub part_id: Option<i64>,
    pub model_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PartListFilter {
    pub q: Option<String>,
    #[serde(default)]
    pub sort: PartSort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartSort {
    NameAsc,
    NameDesc,
    QuantityAsc,
    QuantityDesc,
    #[default]
    Recent,
}

impl PartSort {
    /// SQL ORDER BY fragment (table alias `p` is assumed).
    pub fn order_by(self) -> &'static str {
        match self {
            PartSort::NameAsc => "p.name COLLATE NOCASE ASC, p.id ASC",
            PartSort::NameDesc => "p.name COLLATE NOCASE DESC, p.id DESC",
            PartSort::QuantityAsc => "p.quantity ASC, p.name COLLATE NOCASE ASC, p.id ASC",
            PartSort::QuantityDesc => "p.quantity DESC, p.name COLLATE NOCASE ASC, p.id ASC",
            PartSort::Recent => "p.id DESC",
        }
    }
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

/// Build a `LIKE` pattern for substring search, escaping `%` and `_`.
pub fn like_pattern(q: &str) -> String {
    let mut out = String::with_capacity(q.len() + 4);
    for c in q.chars() {
        if c == '%' || c == '_' {
            out.push('\\');
        }
        out.push(c);
    }
    out.insert(0, '%');
    out.push('%');
    out
}

/// Validates a `YYYY-MM-DD` date string (calendar-correct, including leap years).
pub fn is_valid_iso_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let Some(year) = parse_num(&s[0..4]) else {
        return false;
    };
    let Some(month) = parse_num(&s[5..7]) else {
        return false;
    };
    let Some(day) = parse_num(&s[8..10]) else {
        return false;
    };
    if year < 1 || !(1..=12).contains(&month) || day < 1 {
        return false;
    }
    day <= days_in_month(year, month)
}

/// Validates `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS` (calendar-correct,
/// including leap years).
pub fn is_valid_iso_datetime(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() == 10 {
        return is_valid_iso_date(s);
    }
    if bytes.len() != 19 || bytes[10] != b'T' {
        return false;
    }
    if !is_valid_iso_date(&s[0..10]) {
        return false;
    }
    let Some(hour) = parse_num(&s[11..13]) else {
        return false;
    };
    let Some(minute) = parse_num(&s[14..16]) else {
        return false;
    };
    let Some(second) = parse_num(&s[17..19]) else {
        return false;
    };
    hour <= 23 && minute <= 59 && second <= 59
}

fn parse_num(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn days_in_month(year: u32, month: u32) -> u32 {
    const DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let base = DAYS[(month - 1) as usize];
    if month == 2
        && year.is_multiple_of(4)
        && (!year.is_multiple_of(100) || year.is_multiple_of(400))
    {
        29
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_iso_dates() {
        assert!(is_valid_iso_date("2026-08-14"));
        assert!(is_valid_iso_date("2024-02-29")); // leap year
        assert!(is_valid_iso_date("1900-01-31"));
        assert!(!is_valid_iso_date("2023-02-29")); // not a leap year
        assert!(!is_valid_iso_date("1900-02-29")); // century non-leap
        assert!(!is_valid_iso_date("2026-13-01"));
        assert!(!is_valid_iso_date("2026-00-10"));
        assert!(!is_valid_iso_date("2026-04-31"));
        assert!(!is_valid_iso_date("2026-1-1"));
        assert!(!is_valid_iso_date("2026/08/14"));
        assert!(!is_valid_iso_date("abcd-ef-gh"));
        assert!(!is_valid_iso_date(""));
        assert!(!is_valid_iso_date("2026-08-140"));
    }

    #[test]
    fn validates_iso_datetimes() {
        assert!(is_valid_iso_datetime("2026-08-14"));
        assert!(is_valid_iso_datetime("2026-08-14T00:00:00"));
        assert!(is_valid_iso_datetime("2026-08-14T23:59:59"));
        assert!(is_valid_iso_datetime("2024-02-29T12:30:45"));
        assert!(!is_valid_iso_datetime("2026-08-14T24:00:00"));
        assert!(!is_valid_iso_datetime("2026-08-14T12:60:00"));
        assert!(!is_valid_iso_datetime("2026-08-14T12:30:60"));
        assert!(!is_valid_iso_datetime("2026-08-14T12:30")); // seconds required
        assert!(!is_valid_iso_datetime("2026-08-14 12:30:00")); // 'T' separator
        assert!(!is_valid_iso_datetime("2023-02-29T00:00:00"));
        assert!(!is_valid_iso_datetime("2026-08-14T12:30:00Z"));
        assert!(!is_valid_iso_datetime(""));
    }

    #[test]
    fn trims_and_normalizes_inputs() {
        let input = ModelInput {
            name: "  Kraken 580  ".into(),
            category: Category::Heli,
            manufacturer: Some("   ".into()),
            notes: None,
            date_acquired: Some("2026-01-05".into()),
            status: None,
            photo_url: None,
        };
        let ok = input.validate().unwrap();
        assert_eq!(ok.name, "Kraken 580");
        assert_eq!(ok.manufacturer, None);
        assert_eq!(ok.status(), ModelStatus::Active);

        let bad = ModelInput {
            name: "   ".into(),
            category: Category::Heli,
            manufacturer: None,
            notes: None,
            date_acquired: None,
            status: None,
            photo_url: None,
        };
        assert!(matches!(
            bad.validate(),
            Err(crate::error::DomainError::Invalid(msg)) if msg.starts_with("name:")
        ));

        let bad_date = ModelInput {
            name: "x".into(),
            category: Category::Car,
            manufacturer: None,
            notes: None,
            date_acquired: Some("2026-02-30".into()),
            status: None,
            photo_url: None,
        };
        assert!(bad_date.validate().is_err());
    }

    #[test]
    fn validates_quantity_bounds() {
        fn sample(quantity: i64) -> PartInput {
            PartInput {
                name: "blades".into(),
                quantity,
                notes: None,
                link: None,
                photo_url: None,
                cost: None,
                vendor: None,
                low_stock_enabled: true,
            }
        }

        let ok = PartInput {
            name: "blades".into(),
            quantity: 0,
            notes: None,
            link: None,
            photo_url: None,
            cost: None,
            vendor: None,
            low_stock_enabled: true,
        }
        .validate();
        assert!(ok.is_ok());

        let neg = sample(-1).validate();
        assert!(neg.is_err());

        let huge = sample(i32::MAX as i64 + 1).validate();
        assert!(huge.is_err());
    }

    #[test]
    fn validates_cost_and_vendor() {
        fn sample(cost: Option<f64>) -> PartInput {
            PartInput {
                name: "blades".into(),
                quantity: 1,
                notes: None,
                link: None,
                photo_url: None,
                cost,
                vendor: None,
                low_stock_enabled: true,
            }
        }

        let mut ok = sample(Some(12.5));
        ok.vendor = Some("  Vortex  ".into());
        let out = ok.validate().unwrap();
        assert_eq!(out.cost, Some(12.5));
        assert_eq!(out.vendor.as_deref(), Some("Vortex"));

        let neg = sample(Some(-1.0));
        assert!(neg.validate().is_err());

        assert!(sample(Some(f64::NAN)).validate().is_err());
        assert!(sample(Some(f64::INFINITY)).validate().is_err());
    }

    #[test]
    fn settings_default_and_validation() {
        let defaults = Settings::default();
        assert_eq!(defaults.part_form_fields, PartFormField::ALL.to_vec());
        assert_eq!(defaults.currency, "USD");
        assert_eq!(defaults.theme, Theme::System);

        let s = Settings {
            part_form_fields: vec![
                PartFormField::Cost,
                PartFormField::Cost,
                PartFormField::Notes,
            ],
            currency: "  usd ".into(),
            low_stock_enabled: true,
            low_stock_threshold: 2,
            theme: Theme::Dark,
        };
        let out = s.validate().unwrap();
        assert_eq!(out.theme, Theme::Dark, "theme passes through untouched");
        assert_eq!(
            out.part_form_fields,
            vec![PartFormField::Cost, PartFormField::Notes]
        );
        assert_eq!(out.currency, "USD");

        let bad = Settings {
            part_form_fields: vec![],
            currency: "U.S!".into(),
            low_stock_enabled: true,
            low_stock_threshold: 2,
            theme: Theme::Light,
        };
        assert!(bad.validate().is_err());

        // threshold bounds: 0 and 1000 are valid, -1 and 1001 are not
        let mk = |threshold: i32| Settings {
            part_form_fields: vec![],
            currency: "USD".into(),
            low_stock_enabled: true,
            low_stock_threshold: threshold,
            theme: Theme::System,
        };
        assert!(mk(0).validate().is_ok());
        assert!(mk(1000).validate().is_ok());
        assert!(mk(-1).validate().is_err());
        assert!(mk(1001).validate().is_err());
    }

    #[test]
    fn deserializes_bulk_value_tri_states() {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            value: BulkValue<String>,
        }
        let absent: Probe = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.value, BulkValue::Skip, "absent stays absent");
        let cleared: Probe = serde_json::from_str(r#"{"value": null}"#).unwrap();
        assert_eq!(cleared.value, BulkValue::Clear, "null clears");
        let set: Probe = serde_json::from_str(r#"{"value": "Vortex"}"#).unwrap();
        assert_eq!(set.value, BulkValue::Set("Vortex".into()));
    }

    #[test]
    fn validates_bulk_edit() {
        fn base() -> PartBulkEdit {
            PartBulkEdit {
                part_ids: vec![2, 1, 2],
                quantity: BulkValue::Skip,
                cost: BulkValue::Skip,
                vendor: BulkValue::Set("  Vortex  ".into()),
                link: BulkValue::Skip,
                photo_url: BulkValue::Skip,
                notes: BulkValue::Set("   ".into()),
                low_stock_enabled: BulkValue::Skip,
                model_id: None,
                unlink_model_ids: vec![9, 9, 4],
            }
        }

        let out = base().validate().unwrap();
        assert_eq!(out.part_ids, vec![1, 2], "ids dedupe and sort");
        assert_eq!(out.vendor, BulkValue::Set("Vortex".into()));
        assert_eq!(
            out.notes,
            BulkValue::Clear,
            "whitespace-only string becomes an explicit clear"
        );
        assert_eq!(out.link, BulkValue::Skip, "absent stays absent");
        assert!(out.has_field_updates());
        assert_eq!(out.unlink_model_ids, vec![4, 9]);

        // empty selection is rejected
        assert!(PartBulkEdit::default().validate().is_err());

        // a selection with no changes is rejected
        let nothing = PartBulkEdit {
            part_ids: vec![1],
            ..Default::default()
        };
        assert!(nothing.validate().is_err());

        // a link change alone is a valid bulk edit
        let links_only = PartBulkEdit {
            part_ids: vec![1],
            model_id: Some(7),
            ..Default::default()
        };
        assert!(!links_only.has_field_updates());
        assert!(links_only.validate().is_ok());

        // bound checks
        let bad_qty = PartBulkEdit {
            part_ids: vec![1],
            quantity: BulkValue::Set(-1),
            ..Default::default()
        };
        assert!(bad_qty.validate().is_err());
        let huge_qty = PartBulkEdit {
            part_ids: vec![1],
            quantity: BulkValue::Set(i32::MAX as i64 + 1),
            ..Default::default()
        };
        assert!(huge_qty.validate().is_err());
        let bad_cost = PartBulkEdit {
            part_ids: vec![1],
            cost: BulkValue::Set(-0.5),
            ..Default::default()
        };
        assert!(bad_cost.validate().is_err());
        let nan_cost = PartBulkEdit {
            part_ids: vec![1],
            cost: BulkValue::Set(f64::NAN),
            ..Default::default()
        };
        assert!(nan_cost.validate().is_err());
    }

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(like_pattern("abc"), "%abc%");
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
    }
}
