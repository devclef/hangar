//! File-based catalog source.
//!
//! Catalog data (known manufacturer/model combinations and their official
//! parts) is NOT hardcoded: it lives in version-controlled, human-editable
//! JSON files under `catalog-data/` (one file per model:
//! `catalog-data/<manufacturer-slug>/<model-slug>.json`) and is imported
//! into the `catalog_*` tables by this module.
//!
//! The importer is deliberately conservative:
//! - Re-imports are idempotent upserts (matched by `part_number` when
//!   present, else by name) and are short-circuited by the stored
//!   sha256 checksum of the file, so an unchanged file is not even parsed.
//! - Re-imports NEVER delete rows. Parts missing from a newer version of a
//!   file are left in place (with their inventory links intact) and logged
//!   as orphans for a human to review.
//! - Invalid files are reported and skipped; they never abort the rest of
//!   the import, and never crash startup.
//!
//! See `catalog-data/README.md` (file format, slug conventions, how to add
//! a model) and `catalog-data/schema.json` (machine-readable schema).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::DomainError;
use crate::repo::HangarRepo;
use crate::types::Category;

// ---------------------------------------------------------------------------
// File format (what lives in catalog-data/)
// ---------------------------------------------------------------------------

/// One model file, e.g. `catalog-data/omp-hobby/m1.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFile {
    /// Display name of the manufacturer (e.g. "OMP Hobby"). Matching on
    /// re-import is exact (after trimming), so keep the casing stable.
    pub manufacturer: String,
    /// Display name of the model (e.g. "M1").
    pub model: String,
    /// One of `heli | plane | car | drone | boat | other`.
    pub category: Category,
    /// Optional per-model diagram override: a file name in
    /// `frontend/src/lib/diagrams/` (e.g. "heli-generic.svg"). `null` means
    /// the generic per-category SVG is used.
    #[serde(default)]
    pub diagram_asset: Option<String>,
    pub parts: Vec<CatalogFilePart>,
}

/// One part entry inside a model file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFilePart {
    pub name: String,
    /// Official manufacturer part number. Nullable on purpose: adding a row
    /// can come before the number is verified. Empty strings normalize to
    /// `null`.
    #[serde(default)]
    pub part_number: Option<String>,
    /// Free-text grouping for the legend (e.g. "Blade grip", "Tail boom").
    /// Not the user-part `part_type`.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Hotspot x, percentage 0-100 of the diagram width. Must come in a
    /// pair with `diagram_y`.
    #[serde(default)]
    pub diagram_x: Option<f64>,
    /// Hotspot y, percentage 0-100 of the diagram height.
    #[serde(default)]
    pub diagram_y: Option<f64>,
}

impl CatalogFilePart {
    /// Normalized part number (trimmed, empty → `None`).
    pub fn part_number_norm(&self) -> Option<String> {
        trim_to_opt(self.part_number.as_deref())
    }

    /// Normalized category (trimmed, empty → `None`).
    pub fn category_norm(&self) -> Option<String> {
        trim_to_opt(self.category.as_deref())
    }

    /// Normalized notes (trimmed, empty → `None`).
    pub fn notes_norm(&self) -> Option<String> {
        trim_to_opt(self.notes.as_deref())
    }
}

fn trim_to_opt(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// Lowercase sha256 hex digest of a byte slice (the `source_checksum` value).
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A parsed file plus everything the importer needs to write it.
#[derive(Debug, Clone)]
pub struct ParsedCatalogFile {
    /// Repo-relative file identity, e.g. `omp-hobby/m1.json`.
    pub source_file: String,
    /// sha256 hex of the raw file contents.
    pub checksum: String,
    pub file: CatalogFile,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Failure to read/parse/validate one catalog file. The message is meant to
/// be shown verbatim: it carries the file and the exact field.
#[derive(Debug)]
pub struct CatalogFileError {
    pub source_file: String,
    pub message: String,
}

impl std::fmt::Display for CatalogFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.source_file, self.message)
    }
}

impl std::error::Error for CatalogFileError {}

// ---------------------------------------------------------------------------
// Reading / validating one file
// ---------------------------------------------------------------------------

/// Reads, checksums, parses, and validates one catalog file.
///
/// The checksum is computed over the raw bytes BEFORE parsing, so a caller
/// can compare it against the stored `source_checksum` and skip re-importing
/// unchanged files without parsing them at all.
pub fn read_catalog_file(
    source_file: &str,
    path: &Path,
) -> Result<ParsedCatalogFile, CatalogFileError> {
    let bytes = std::fs::read(path).map_err(|e| CatalogFileError {
        source_file: source_file.to_string(),
        message: format!("cannot read file: {e}"),
    })?;
    let checksum = sha256_hex(&bytes);

    let raw: CatalogFile = serde_json::from_slice(&bytes).map_err(|e| CatalogFileError {
        source_file: source_file.to_string(),
        message: format!("invalid JSON: {e}"),
    })?;
    let file = validate(&raw).map_err(|message| CatalogFileError {
        source_file: source_file.to_string(),
        message,
    })?;
    Ok(ParsedCatalogFile {
        source_file: source_file.to_string(),
        checksum,
        file,
    })
}

/// Validates a parsed file, returning a human-readable `field: message`
/// string on the first problem found. Mirrors `catalog-data/schema.json`.
fn validate(file: &CatalogFile) -> Result<CatalogFile, String> {
    let manufacturer = file.manufacturer.trim();
    if manufacturer.is_empty() {
        return Err("manufacturer: must not be empty".into());
    }
    if manufacturer.len() > 200 {
        return Err("manufacturer: must be at most 200 characters".into());
    }
    let model = file.model.trim();
    if model.is_empty() {
        return Err("model: must not be empty".into());
    }
    if model.len() > 200 {
        return Err("model: must be at most 200 characters".into());
    }

    let mut out = file.clone();
    out.manufacturer = manufacturer.to_string();
    out.model = model.to_string();
    out.diagram_asset = match file.diagram_asset.as_deref().map(str::trim) {
        Some(a) if !a.is_empty() => {
            if a.len() > 200 {
                return Err("diagram_asset: must be at most 200 characters".into());
            }
            if a.starts_with('/') || a.contains("..") {
                return Err(
                    "diagram_asset: must be a plain file name inside the diagrams folder (no '/' or '..')"
                        .into(),
                );
            }
            Some(a.to_string())
        }
        // absent or empty-after-trim both mean "no override"
        _ => None,
    };

    // Duplicate detection: a part is keyed by part_number when it has one,
    // else by (case-insensitive) name — the same rule the upsert uses.
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for (i, part) in out.parts.iter_mut().enumerate() {
        let at = format!("parts[{i}]");
        let name = part.name.trim().to_string();
        if name.is_empty() {
            return Err(format!("{at}.name: must not be empty"));
        }
        if name.len() > 200 {
            return Err(format!("{at}.name: must be at most 200 characters"));
        }
        part.name = name.clone();

        let part_number = part.part_number_norm();
        if part_number.as_deref().is_some_and(|n| n.len() > 100) {
            return Err(format!("{at}.part_number: must be at most 100 characters"));
        }
        if part
            .category_norm()
            .as_deref()
            .is_some_and(|c| c.len() > 100)
        {
            return Err(format!("{at}.category: must be at most 100 characters"));
        }
        if part.notes_norm().as_deref().is_some_and(|n| n.len() > 2000) {
            return Err(format!("{at}.notes: must be at most 2000 characters"));
        }

        match (part.diagram_x, part.diagram_y) {
            (Some(x), Some(y)) => {
                if !x.is_finite() || !y.is_finite() {
                    return Err(format!("{at}.diagram_x/diagram_y: must be finite numbers"));
                }
                if !(0.0..=100.0).contains(&x) {
                    return Err(format!(
                        "{at}.diagram_x: must be between 0 and 100 (got {x})"
                    ));
                }
                if !(0.0..=100.0).contains(&y) {
                    return Err(format!(
                        "{at}.diagram_y: must be between 0 and 100 (got {y})"
                    ));
                }
            }
            (Some(x), None) => {
                return Err(format!(
                    "{at}.diagram_x: present without diagram_y (got {x}) — give both or neither"
                ))
            }
            (None, Some(y)) => {
                return Err(format!(
                    "{at}.diagram_y: present without diagram_x (got {y}) — give both or neither"
                ))
            }
            (None, None) => {}
        }

        part.part_number = part_number.clone();
        part.category = part.category_norm();
        part.notes = part.notes_norm();

        let key = part_number
            .map(|n| format!("num\0{n}"))
            .unwrap_or_else(|| format!("name\0{}", name.to_lowercase()));
        if let Some(first) = seen.insert(key.clone(), format!("{at}.name \"{name}\"")) {
            let kind = if key.starts_with("num") {
                "part_number"
            } else {
                "name"
            };
            return Err(format!("{at}: duplicate {kind} — collides with {first}"));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Directory scanning
// ---------------------------------------------------------------------------

/// Default location of the catalog files (override with `CATALOG_DIR`).
pub fn default_catalog_dir() -> PathBuf {
    std::env::var("CATALOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("catalog-data"))
}

/// Recursively collects `*.json` files under `root`, sorted by their
/// repo-relative path so import order (and log output) is deterministic.
pub fn scan_catalog_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
                && path
                    .file_name()
                    .is_some_and(|n| !n.eq_ignore_ascii_case("schema.json"))
            {
                // `schema.json` files are format documents, not catalog data.
                let rel = path
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| path.to_string_lossy().into_owned());
                out.push((rel, path));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ---------------------------------------------------------------------------
// Import results
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportStatus {
    /// No catalog model existed for this file before; everything was created.
    Created,
    /// The model existed; rows were updated/inserted as needed.
    #[default]
    Updated,
    /// The stored checksum already matched the file; nothing was done.
    Unchanged,
}

/// Result of importing one file.
#[derive(Debug, Clone, Default)]
pub struct CatalogImportResult {
    pub source_file: String,
    pub checksum: String,
    pub status: ImportStatus,
    /// True when the catalog model row itself was inserted (vs. updated).
    pub model_created: bool,
    pub parts_created: usize,
    pub parts_updated: usize,
    pub parts_unchanged: usize,
    /// `(id, name)` of rows that were previously imported from this file's
    /// model but are no longer present in it. They are LEFT IN PLACE; a
    /// human reviews them and deletes them explicitly if wanted.
    pub orphaned_parts: Vec<(i64, String)>,
}

impl CatalogImportResult {
    pub fn summary_line(&self) -> String {
        match self.status {
            ImportStatus::Unchanged => {
                format!("{} unchanged", self.source_file)
            }
            _ => {
                let model = if self.model_created {
                    "model created"
                } else {
                    "model updated"
                };
                format!(
                    "{} {model}, {} part(s) created, {} updated, {} unchanged, {} orphaned",
                    self.source_file,
                    self.parts_created,
                    self.parts_updated,
                    self.parts_unchanged,
                    self.orphaned_parts.len()
                )
            }
        }
    }
}

/// Aggregate result over a whole directory (or one file).
#[derive(Debug, Clone, Default)]
pub struct CatalogImportSummary {
    pub files: usize,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// `(source_file, error message)` for every file that failed validation
    /// or import. Failures never abort the rest of the run.
    pub failed: Vec<(String, String)>,
}

impl CatalogImportSummary {
    pub fn log_summary(&self) {
        tracing::info!(
            files = self.files,
            created = self.created,
            updated = self.updated,
            unchanged = self.unchanged,
            failed = self.failed.len(),
            "catalog import finished"
        );
        for (file, err) in &self.failed {
            tracing::warn!(file, error = %err, "catalog import: skipping file");
        }
    }

    /// True when every file imported cleanly.
    pub fn ok(&self) -> bool {
        self.failed.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Import orchestration (file → repo)
// ---------------------------------------------------------------------------

/// Imports one file. `root` is the catalog directory the file lives in (used
/// to derive the `source_file` identity); pass the file's own directory when
/// importing a path outside any catalog root.
///
/// Short-circuits to `Unchanged` when the stored `(source_file, checksum)`
/// already matches — the file is not even parsed.
///
/// Note the order: the checksum is computed from the raw bytes first, and
/// parsing happens only on a miss. That is what makes re-imports of large,
/// unchanged files cheap (no JSON parse, no validation, no DB writes).
pub async fn import_file(
    repo: &dyn HangarRepo,
    root: &Path,
    path: &Path,
) -> Result<CatalogImportResult, DomainError> {
    // Derive the stable identity (the stored `source_file`) from
    // canonicalized paths on both sides, so relative CLI args, the startup
    // scan, and tests all produce the same value for the same file. Files
    // outside the catalog root are identified by their bare file name.
    let source_file = match (path.canonicalize(), root.canonicalize()) {
        (Ok(pc), Ok(rc)) if pc.starts_with(&rc) => pc
            .strip_prefix(&rc)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| {
                pc.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "file".to_string())
            }),
        _ => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned()),
    };
    let source_file = source_file.as_str();

    let bytes = std::fs::read(path)
        .map_err(|e| DomainError::Invalid(format!("cannot read {source_file}: {e}")))?;
    let checksum = sha256_hex(&bytes);

    if repo
        .find_catalog_model_by_source(source_file, &checksum)
        .await?
        .is_some()
    {
        tracing::debug!(file = source_file, "catalog file unchanged, skipping");
        return Ok(CatalogImportResult {
            source_file: source_file.to_string(),
            checksum,
            status: ImportStatus::Unchanged,
            ..Default::default()
        });
    }

    // Checksum changed (or is new): parse and validate the current content.
    let parsed = match read_catalog_file(source_file, path) {
        Ok(parsed) => parsed,
        Err(CatalogFileError {
            source_file,
            message,
        }) => return Err(DomainError::Invalid(format!("{source_file}: {message}"))),
    };

    let result = repo
        .import_catalog_file(&parsed.source_file, &parsed.checksum, &parsed.file)
        .await?;
    for (id, name) in &result.orphaned_parts {
        tracing::warn!(
            part = %name,
            id,
            file = source_file,
            "catalog part no longer present in file — left in place, review manually"
        );
    }
    Ok(result)
}

/// Imports every `*.json` under `root`. A missing/empty directory is not an
/// error (the app runs fine without catalog data). Per-file failures are
/// collected into the summary instead of aborting the run.
pub async fn import_dir(
    repo: &dyn HangarRepo,
    root: &Path,
) -> Result<CatalogImportSummary, DomainError> {
    let mut summary = CatalogImportSummary::default();
    if !root.is_dir() {
        tracing::info!(
            dir = %root.display(),
            "catalog directory not found — skipping catalog import"
        );
        return Ok(summary);
    }
    let files = scan_catalog_files(root);
    summary.files = files.len();

    // Files that used to exist but are gone now: their catalog rows are
    // left in place (never auto-delete), but a human should notice.
    let scanned: std::collections::BTreeSet<String> =
        files.iter().map(|(rel, _)| rel.clone()).collect();
    for (model_id, model_name, source_file) in repo.list_catalog_model_sources().await? {
        if !scanned.contains(&source_file) {
            tracing::warn!(
                file = %source_file,
                model = %model_name,
                id = model_id,
                "catalog source file no longer exists — rows left in place, review manually"
            );
        }
    }

    for (rel, path) in files {
        match import_file(repo, root, &path).await {
            Ok(result) => match result.status {
                ImportStatus::Created => summary.created += 1,
                ImportStatus::Updated => summary.updated += 1,
                ImportStatus::Unchanged => summary.unchanged += 1,
            },
            Err(e) => {
                tracing::warn!(file = %rel, error = %e, "catalog import: skipping file");
                summary.failed.push((rel, e.to_string()));
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hangar-catalog-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, contents: &str) {
        std::fs::write(path, contents).unwrap();
    }

    const GOOD: &str = r#"{
        "manufacturer": "OMP Hobby",
        "model": "M1",
        "category": "heli",
        "diagram_asset": "heli-generic.svg",
        "parts": [
            {
                "name": "Main blade grip set",
                "part_number": "OSHM1013",
                "category": "Blade grip",
                "notes": "Includes bearings",
                "diagram_x": 37.0,
                "diagram_y": 20.0
            },
            {
                "name": "Tail blade grip set",
                "part_number": null,
                "category": "Blade grip",
                "notes": "Part number not yet verified",
                "diagram_x": 91.0,
                "diagram_y": 47.0
            },
            { "name": "Hardware bag", "part_number": null }
        ]
    }"#;

    #[test]
    fn validates_good_file() {
        let parsed: CatalogFile = serde_json::from_str(GOOD).unwrap();
        let out = validate(&parsed).unwrap();
        assert_eq!(out.manufacturer, "OMP Hobby");
        assert!(out.parts[2].part_number.is_none());
        assert!(out.parts[2].diagram_x.is_none());
    }

    #[test]
    fn rejects_bad_files_with_field_paths() {
        fn err(json: &str) -> String {
            let parsed: CatalogFile = serde_json::from_str(json).unwrap();
            validate(&parsed).unwrap_err()
        }
        assert!(
            err(r#"{"manufacturer":"", "model":"M", "category":"heli", "parts":[]}"#)
                .starts_with("manufacturer:")
        );
        assert!(
            err(r#"{"manufacturer":"M", "model":" ", "category":"heli", "parts":[]}"#)
                .starts_with("model:")
        );
        assert!(err(r#"{"manufacturer":"M", "model":"M", "category":"heli", "diagram_asset":"/etc/passwd", "parts":[]}"#)
            .starts_with("diagram_asset:"));
        assert!(err(r#"{"manufacturer":"M", "model":"M", "category":"heli", "parts":[{"name":"A", "diagram_x": 10.0}]}"#)
            .contains("parts[0].diagram_x"));
        assert!(err(r#"{"manufacturer":"M", "model":"M", "category":"heli", "parts":[{"name":"A", "diagram_x": 150.0, "diagram_y": 10.0}]}"#)
            .contains("parts[0].diagram_x"));
        assert!(err(r#"{"manufacturer":"M", "model":"M", "category":"heli", "parts":[{"name":"A", "part_number":"N1"},{"name":"B", "part_number":"N1"}]}"#)
            .contains("duplicate part_number"));
        assert!(err(r#"{"manufacturer":"M", "model":"M", "category":"heli", "parts":[{"name":"A"},{"name":"a"}]}"#)
            .contains("duplicate name"));
        // unknown fields are rejected (typos in hand-written files)
        let e = serde_json::from_str::<CatalogFile>(
            r#"{"manufacturer":"M", "model":"M", "category":"heli", "parts":[], "typo": 1}"#,
        );
        assert!(e.is_err());
    }

    #[test]
    fn empty_strings_normalize_to_null() {
        let parsed: CatalogFile = serde_json::from_str(
            r#"{"manufacturer":"M", "model":"M", "category":"heli",
                 "parts":[{"name":"A", "part_number":"  ", "notes":"  ", "category":""}]}"#,
        )
        .unwrap();
        let out = validate(&parsed).unwrap();
        assert!(out.parts[0].part_number.is_none());
        assert!(out.parts[0].notes.is_none());
        assert!(out.parts[0].category.is_none());
    }

    #[test]
    fn scan_collects_json_recursively_and_sorted() {
        let root = tmp("scan");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("b-manuf")).unwrap();
        std::fs::create_dir_all(root.join("a-manuf/sub")).unwrap();
        let mut f1 = std::fs::File::create(root.join("b-manuf/z.json")).unwrap();
        let mut f2 = std::fs::File::create(root.join("a-manuf/sub/a.json")).unwrap();
        let mut f3 = std::fs::File::create(root.join("a-manuf/top.json")).unwrap();
        let mut f4 = std::fs::File::create(root.join("ignore.txt")).unwrap();
        let mut f5 = std::fs::File::create(root.join("schema.json")).unwrap();
        f1.write_all(b"{}").unwrap();
        f2.write_all(b"{}").unwrap();
        f3.write_all(b"{}").unwrap();
        f4.write_all(b"{}").unwrap();
        f5.write_all(b"{}").unwrap();

        let files = scan_catalog_files(&root);
        let names: Vec<_> = files.iter().map(|(rel, _)| rel.as_str()).collect();
        assert_eq!(
            names,
            vec!["a-manuf/sub/a.json", "a-manuf/top.json", "b-manuf/z.json"]
        );
    }

    #[test]
    fn checksum_is_stable() {
        let root = tmp("checksum");
        let path = root.join("a.json");
        write(&path, GOOD);
        let p1 = read_catalog_file("a.json", &path).unwrap();
        let p2 = read_catalog_file("a.json", &path).unwrap();
        assert_eq!(p1.checksum, p2.checksum);
        assert_eq!(p1.checksum.len(), 64);
        write(&path, &format!("{GOOD}\n"));
        let p3 = read_catalog_file("a.json", &path).unwrap();
        assert_ne!(p1.checksum, p3.checksum);
    }
}
