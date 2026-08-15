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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct Part {
    pub id: i64,
    pub name: String,
    pub part_type: Option<String>,
    pub quantity: i32,
    pub notes: Option<String>,
    pub link: Option<String>,
    pub photo_url: Option<String>,
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
        }
    }
}

/// A part as returned by list endpoints (includes linked-model summary).
/// `model_names` is a `'|'`-joined list of linked model names (NULL when none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct PartListRow {
    pub id: i64,
    pub name: String,
    pub part_type: Option<String>,
    pub quantity: i32,
    pub notes: Option<String>,
    pub link: Option<String>,
    pub photo_url: Option<String>,
    pub model_count: i64,
    pub model_names: Option<String>,
}

impl PartListRow {
    pub fn into_part(self) -> Part {
        Part {
            id: self.id,
            name: self.name,
            part_type: self.part_type,
            quantity: self.quantity,
            notes: self.notes,
            link: self.link,
            photo_url: self.photo_url,
        }
    }
}

/// Model detail payload: the model plus every part linked to it.
#[derive(Debug, Serialize)]
pub struct ModelDetail {
    pub model: Model,
    pub parts: Vec<PartListRow>,
}

/// Part detail payload: the part plus every model it is linked to.
#[derive(Debug, Serialize)]
pub struct PartDetail {
    pub part: Part,
    pub models: Vec<Model>,
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
    #[serde(default)]
    pub part_type: Option<String>,
    pub quantity: i64,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub photo_url: Option<String>,
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
        self.part_type = trim_opt(self.part_type);
        self.notes = trim_opt(self.notes);
        self.link = trim_opt(self.link);
        self.photo_url = trim_opt(self.photo_url);
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
        Ok(self)
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
pub struct PartListFilter {
    pub q: Option<String>,
    pub part_type: Option<String>,
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
        let ok = PartInput {
            name: "blades".into(),
            part_type: None,
            quantity: 0,
            notes: None,
            link: None,
            photo_url: None,
        }
        .validate();
        assert!(ok.is_ok());

        let neg = PartInput {
            name: "blades".into(),
            part_type: None,
            quantity: -1,
            notes: None,
            link: None,
            photo_url: None,
        }
        .validate();
        assert!(neg.is_err());

        let huge = PartInput {
            name: "blades".into(),
            part_type: None,
            quantity: i32::MAX as i64 + 1,
            notes: None,
            link: None,
            photo_url: None,
        }
        .validate();
        assert!(huge.is_err());
    }

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(like_pattern("abc"), "%abc%");
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
    }
}
