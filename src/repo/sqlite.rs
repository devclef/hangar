//! SQLite implementation of [`HangarRepo`].

use async_trait::async_trait;
use sqlx::sqlite::SqlitePool;
use sqlx::{query, query_as, query_scalar, QueryBuilder, Sqlite};

use std::collections::BTreeMap;

use super::HangarRepo;
use crate::catalog::{CatalogFile, CatalogImportResult, ImportStatus};
use crate::error::DomainError;
use crate::types::{
    like_pattern, CatalogManufacturer, CatalogModel, CatalogPart, Category, Model, ModelInput,
    ModelListRow, Part, PartBulkEdit, PartInput, PartListRow, PartSort, Settings, UsageRecord,
};

pub struct SqliteRepo {
    pool: SqlitePool,
}

impl SqliteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl HangarRepo for SqliteRepo {
    // -- Models -------------------------------------------------------------

    async fn list_models(
        &self,
        q: Option<&str>,
        category: Option<Category>,
    ) -> Result<Vec<ModelListRow>, DomainError> {
        let sql = "SELECT m.*, \
                   (SELECT COUNT(*) FROM model_parts mp WHERE mp.model_id = m.id) AS part_count \
                   FROM models m \
                   WHERE (?1 IS NULL \
                          OR m.name LIKE ?1 ESCAPE '\\' \
                          OR m.manufacturer LIKE ?1 ESCAPE '\\' \
                          OR m.notes LIKE ?1 ESCAPE '\\') \
                     AND (?2 IS NULL OR m.category = ?2) \
                   ORDER BY m.name COLLATE NOCASE ASC, m.id ASC";
        let pattern: Option<String> = q.map(like_pattern);
        let cat: Option<String> = category.map(|c| c.to_string());
        let rows = query_as::<_, ModelListRow>(sql)
            .bind(pattern)
            .bind(cat)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    async fn get_model(&self, id: i64) -> Result<Option<Model>, DomainError> {
        let row = query_as::<_, Model>("SELECT * FROM models WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    async fn create_model(&self, input: &ModelInput) -> Result<Model, DomainError> {
        let row = query_as::<_, Model>(
            "INSERT INTO models (name, category, manufacturer, notes, date_acquired, status, photo_url) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING *",
        )
        .bind(&input.name)
        .bind(input.category)
        .bind(&input.manufacturer)
        .bind(&input.notes)
        .bind(&input.date_acquired)
        .bind(input.status())
        .bind(&input.photo_url)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn update_model(
        &self,
        id: i64,
        input: &ModelInput,
    ) -> Result<Option<Model>, DomainError> {
        let row = query_as::<_, Model>(
            "UPDATE models SET name = ?1, category = ?2, manufacturer = ?3, notes = ?4, \
              date_acquired = ?5, status = ?6, photo_url = ?7, \
              updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?8 RETURNING *",
        )
        .bind(&input.name)
        .bind(input.category)
        .bind(&input.manufacturer)
        .bind(&input.notes)
        .bind(&input.date_acquired)
        .bind(input.status())
        .bind(&input.photo_url)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn delete_model(&self, id: i64) -> Result<bool, DomainError> {
        let res = query("DELETE FROM models WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    // -- Parts --------------------------------------------------------------

    async fn list_parts(
        &self,
        q: Option<&str>,
        sort: PartSort,
    ) -> Result<Vec<PartListRow>, DomainError> {
        let sql = format!(
            "SELECT p.*, \
                   (SELECT COUNT(*) FROM model_parts mp WHERE mp.part_id = p.id) AS model_count, \
                   (SELECT GROUP_CONCAT(m.name, '|') \
                      FROM model_parts mp JOIN models m ON m.id = mp.model_id \
                     WHERE mp.part_id = p.id) AS model_names \
                   FROM parts p \
                   WHERE (?1 IS NULL \
                          OR p.name LIKE ?1 ESCAPE '\\' \
                          OR p.notes LIKE ?1 ESCAPE '\\' \
                          OR p.link LIKE ?1 ESCAPE '\\') \
                   ORDER BY {}",
            sort.order_by()
        );
        let pattern: Option<String> = q.map(like_pattern);
        let rows = query_as::<_, PartListRow>(&sql)
            .bind(pattern)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    async fn get_part(&self, id: i64) -> Result<Option<Part>, DomainError> {
        let row = query_as::<_, Part>("SELECT * FROM parts WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    async fn create_part(&self, input: &PartInput) -> Result<Part, DomainError> {
        let row = query_as::<_, Part>(
            "INSERT INTO parts (name, quantity, notes, link, photo_url, cost, vendor, \
                                low_stock_enabled) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) RETURNING *",
        )
        .bind(&input.name)
        .bind(input.quantity as i32)
        .bind(&input.notes)
        .bind(&input.link)
        .bind(&input.photo_url)
        .bind(input.cost)
        .bind(&input.vendor)
        .bind(input.low_stock_enabled)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn update_part(&self, id: i64, input: &PartInput) -> Result<Option<Part>, DomainError> {
        let row = query_as::<_, Part>(
            "UPDATE parts SET name = ?1, quantity = ?2, notes = ?3, link = ?4, \
              photo_url = ?5, cost = ?6, vendor = ?7, low_stock_enabled = ?8, \
              updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?9 RETURNING *",
        )
        .bind(&input.name)
        .bind(input.quantity as i32)
        .bind(&input.notes)
        .bind(&input.link)
        .bind(&input.photo_url)
        .bind(input.cost)
        .bind(&input.vendor)
        .bind(input.low_stock_enabled)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn delete_part(&self, id: i64) -> Result<bool, DomainError> {
        let res = query("DELETE FROM parts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn set_quantity(&self, id: i64, quantity: i32) -> Result<Option<Part>, DomainError> {
        let row = query_as::<_, Part>(
            "UPDATE parts SET quantity = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?2 RETURNING *",
        )
        .bind(quantity)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn adjust_quantity(&self, id: i64, delta: i64) -> Result<Option<Part>, DomainError> {
        let row = query_as::<_, Part>(
            "UPDATE parts SET quantity = MAX(0, quantity + ?1), \
              updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?2 RETURNING *",
        )
        .bind(delta)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn bulk_edit_parts(
        &self,
        part_ids: &[i64],
        input: &PartBulkEdit,
    ) -> Result<(), DomainError> {
        let mut tx = self.pool.begin().await?;
        if input.has_field_updates() {
            // Fields are tri-state: `Some(v)` writes (a clear when `v` is
            // None), `None` leaves the column untouched.
            let mut qb = QueryBuilder::<Sqlite>::new(String::from(
                "UPDATE parts SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
            ));
            if input.quantity.is_present() {
                qb.push(", quantity = ");
                qb.push_bind(input.quantity.as_value().map(|q| *q as i32));
            }
            if input.cost.is_present() {
                qb.push(", cost = ");
                qb.push_bind(input.cost.as_value().copied());
            }
            if input.vendor.is_present() {
                qb.push(", vendor = ");
                qb.push_bind(input.vendor.as_value().cloned());
            }
            if input.link.is_present() {
                qb.push(", link = ");
                qb.push_bind(input.link.as_value().cloned());
            }
            if input.photo_url.is_present() {
                qb.push(", photo_url = ");
                qb.push_bind(input.photo_url.as_value().cloned());
            }
            if input.notes.is_present() {
                qb.push(", notes = ");
                qb.push_bind(input.notes.as_value().cloned());
            }
            if input.low_stock_enabled.is_present() {
                qb.push(", low_stock_enabled = ");
                qb.push_bind(input.low_stock_enabled.as_value().copied());
            }
            qb.push(" WHERE id IN (");
            qb.push_values(part_ids.iter().copied(), |mut w, id| {
                w.push_bind(id);
            });
            qb.push(")");
            qb.build().execute(&mut *tx).await?;
        }
        if let Some(model_id) = input.model_id {
            for part_id in part_ids {
                query("INSERT OR IGNORE INTO model_parts (model_id, part_id) VALUES (?1, ?2)")
                    .bind(model_id)
                    .bind(*part_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        for model_id in &input.unlink_model_ids {
            let mut del = QueryBuilder::<Sqlite>::new(String::from(
                "DELETE FROM model_parts WHERE model_id = ",
            ));
            del.push_bind(*model_id);
            del.push(" AND part_id IN (");
            del.push_values(part_ids.iter().copied(), |mut w, id| {
                w.push_bind(id);
            });
            del.push(")");
            del.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn list_parts_by_ids(&self, part_ids: &[i64]) -> Result<Vec<PartListRow>, DomainError> {
        if part_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut qb = QueryBuilder::<Sqlite>::new(String::from(
            "SELECT p.*, \
                   (SELECT COUNT(*) FROM model_parts mp WHERE mp.part_id = p.id) AS model_count, \
                   (SELECT GROUP_CONCAT(m.name, '|') \
                      FROM model_parts mp JOIN models m ON m.id = mp.model_id \
                     WHERE mp.part_id = p.id) AS model_names \
               FROM parts p WHERE p.id IN (",
        ));
        qb.push_values(part_ids.iter().copied(), |mut w, id| {
            w.push_bind(id);
        });
        qb.push(") ORDER BY p.id ASC");
        let rows = qb
            .build_query_as::<PartListRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // -- Model <-> part association -----------------------------------------

    async fn list_model_parts(&self, model_id: i64) -> Result<Vec<PartListRow>, DomainError> {
        let sql = "SELECT p.*, \
                         (SELECT COUNT(*) FROM model_parts mp2 WHERE mp2.part_id = p.id) AS model_count, \
                         (SELECT GROUP_CONCAT(m2.name, '|') \
                            FROM model_parts mp2 JOIN models m2 ON m2.id = mp2.model_id \
                           WHERE mp2.part_id = p.id) AS model_names \
                   FROM parts p \
                   JOIN model_parts mp ON mp.part_id = p.id AND mp.model_id = ?1 \
                   ORDER BY p.name COLLATE NOCASE ASC, p.id ASC";
        let rows = query_as::<_, PartListRow>(sql)
            .bind(model_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    async fn add_link(&self, model_id: i64, part_id: i64) -> Result<(), DomainError> {
        query("INSERT OR IGNORE INTO model_parts (model_id, part_id) VALUES (?1, ?2)")
            .bind(model_id)
            .bind(part_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn remove_link(&self, model_id: i64, part_id: i64) -> Result<bool, DomainError> {
        let res = query("DELETE FROM model_parts WHERE model_id = ?1 AND part_id = ?2")
            .bind(model_id)
            .bind(part_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn replace_links(&self, model_id: i64, part_ids: &[i64]) -> Result<(), DomainError> {
        let mut tx = self.pool.begin().await?;
        query("DELETE FROM model_parts WHERE model_id = ?1")
            .bind(model_id)
            .execute(&mut *tx)
            .await?;
        for part_id in part_ids {
            query("INSERT OR IGNORE INTO model_parts (model_id, part_id) VALUES (?1, ?2)")
                .bind(model_id)
                .bind(part_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn list_part_models(&self, part_id: i64) -> Result<Vec<Model>, DomainError> {
        let rows = query_as::<_, Model>(
            "SELECT m.* FROM models m \
             JOIN model_parts mp ON mp.model_id = m.id \
             WHERE mp.part_id = ?1 \
             ORDER BY m.name COLLATE NOCASE ASC, m.id ASC",
        )
        .bind(part_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -- Part usage log -----------------------------------------------------

    async fn list_usage(
        &self,
        part_id: Option<i64>,
        model_id: Option<i64>,
    ) -> Result<Vec<UsageRecord>, DomainError> {
        let sql = "SELECT u.id, u.part_id, p.name AS part_name, u.model_id, m.name AS model_name, \
                      m.category AS model_category, u.quantity, u.notes, u.used_at \
                   FROM part_usage u \
                   JOIN parts p ON p.id = u.part_id \
                   JOIN models m ON m.id = u.model_id \
                   WHERE (?1 IS NULL OR u.part_id = ?1) \
                     AND (?2 IS NULL OR u.model_id = ?2) \
                   ORDER BY u.used_at DESC, u.id DESC";
        let rows = query_as::<_, UsageRecord>(sql)
            .bind(part_id)
            .bind(model_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    async fn add_usage(
        &self,
        part_id: i64,
        model_id: i64,
        quantity: i32,
        notes: Option<&str>,
        used_at: Option<&str>,
    ) -> Result<UsageRecord, DomainError> {
        let mut tx = self.pool.begin().await?;
        let res = match used_at {
            Some(stamp) => {
                query(
                    "INSERT INTO part_usage (part_id, model_id, quantity, notes, used_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind(part_id)
                .bind(model_id)
                .bind(quantity)
                .bind(notes)
                .bind(stamp)
                .execute(&mut *tx)
                .await?
            }
            None => {
                query(
                    "INSERT INTO part_usage (part_id, model_id, quantity, notes) \
                 VALUES (?1, ?2, ?3, ?4)",
                )
                .bind(part_id)
                .bind(model_id)
                .bind(quantity)
                .bind(notes)
                .execute(&mut *tx)
                .await?
            }
        };
        // Same clamp-at-0 semantics as adjust_quantity.
        query(
            "UPDATE parts SET quantity = MAX(0, quantity - ?1), \
               updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
               WHERE id = ?2",
        )
        .bind(quantity as i64)
        .bind(part_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let row = query_as::<_, UsageRecord>(
            "SELECT u.id, u.part_id, p.name AS part_name, u.model_id, m.name AS model_name, \
                     m.category AS model_category, u.quantity, u.notes, u.used_at \
             FROM part_usage u \
             JOIN parts p ON p.id = u.part_id \
             JOIN models m ON m.id = u.model_id \
             WHERE u.id = ?1",
        )
        .bind(res.last_insert_rowid())
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    // -- Settings -----------------------------------------------------------

    async fn get_settings(&self) -> Result<Option<Settings>, DomainError> {
        let row: Option<String> = query_scalar("SELECT value FROM settings WHERE key = 'app'")
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(json) => {
                match serde_json::from_str::<Settings>(&json) {
                    Ok(settings) => Ok(Some(settings)),
                    // A stored document that no longer parses (e.g. it still
                    // references a removed field like "part_type") is treated
                    // as "no settings": the service returns the defaults and
                    // the next PUT heals the row.
                    Err(e) => {
                        tracing::warn!(error = %e, "ignoring unparseable stored settings");
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }

    async fn save_settings(&self, settings: &Settings) -> Result<(), DomainError> {
        let json = serde_json::to_string(settings)
            .map_err(|e| DomainError::Db(anyhow::anyhow!("serializing settings: {e}")))?;
        query(
            "INSERT INTO settings (key, value) VALUES ('app', ?1) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(&json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -- Reference catalog ----------------------------------------------------

    async fn import_catalog_file(
        &self,
        source_file: &str,
        checksum: &str,
        file: &CatalogFile,
    ) -> Result<CatalogImportResult, DomainError> {
        let mut tx = self.pool.begin().await?;

        // 1) Manufacturer: upsert by (trimmed, exact) name. Notes are
        //    importer-unset; on conflict we only bump updated_at so manually
        //    added notes survive re-imports.
        let mfr_id: i64 = query_scalar(
            "INSERT INTO catalog_manufacturers (name) VALUES (?1)
             ON CONFLICT(name) DO UPDATE SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             RETURNING id",
        )
        .bind(&file.manufacturer)
        .fetch_one(&mut *tx)
        .await?;

        // 2) Catalog model: upsert by (manufacturer_id, name).
        let existing: Option<(i64, String, Option<String>)> = query_as(
            "SELECT id, category, diagram_asset FROM catalog_models
             WHERE manufacturer_id = ?1 AND name = ?2",
        )
        .bind(mfr_id)
        .bind(&file.model)
        .fetch_optional(&mut *tx)
        .await?;

        let (model_id, model_created) = match existing {
            Some((id, _, _)) => {
                // The file changed (checksum miss got us here); the model row
                // is refreshed even when only whitespace changed, because the
                // checksum itself must advance to short-circuit next time.
                query(
                    "UPDATE catalog_models
                     SET category = ?1, diagram_asset = ?2, source_file = ?3,
                         source_checksum = ?4,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     WHERE id = ?5",
                )
                .bind(file.category)
                .bind(&file.diagram_asset)
                .bind(source_file)
                .bind(checksum)
                .bind(id)
                .execute(&mut *tx)
                .await?;
                (id, false)
            }
            None => {
                let id: i64 = query_scalar(
                    "INSERT INTO catalog_models (manufacturer_id, name, category, diagram_asset,
                                                source_file, source_checksum)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id",
                )
                .bind(mfr_id)
                .bind(&file.model)
                .bind(file.category)
                .bind(&file.diagram_asset)
                .bind(source_file)
                .bind(checksum)
                .fetch_one(&mut *tx)
                .await?;
                (id, true)
            }
        };

        // 3) Parts: matching rules (kept in sync with the file validator):
        //    - a file part WITH a part number matches an existing row with
        //      the same number (exact); if no such row exists, it falls back
        //      to an existing row WITHOUT a number and with the same
        //      (case-insensitive) name — this is what lets a follow-up edit
        //      "fill in the part number" re-key an existing row instead of
        //      orphaning it and duplicating the part.
        //    - a file part WITHOUT a number only matches an unnumbered row
        //      by (case-insensitive) name (it never strips a number).
        //    Unmatched file parts are inserted; unmatched existing rows
        //    become orphans (LEFT IN PLACE, reported for manual review).
        let existing_parts: Vec<CatalogPart> =
            query_as("SELECT * FROM catalog_parts WHERE catalog_model_id = ?1 ORDER BY id")
                .bind(model_id)
                .fetch_all(&mut *tx)
                .await?;
        let mut by_number: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut by_unnumbered_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, p) in existing_parts.iter().enumerate() {
            match &p.part_number {
                Some(n) => by_number.entry(n.clone()).or_default().push(i),
                None => by_unnumbered_name
                    .entry(p.name.to_lowercase())
                    .or_default()
                    .push(i),
            }
        }
        let mut claimed: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();

        let mut result = CatalogImportResult {
            source_file: source_file.to_string(),
            checksum: checksum.to_string(),
            status: if model_created {
                ImportStatus::Created
            } else {
                ImportStatus::Updated
            },
            model_created,
            ..Default::default()
        };
        for fp in &file.parts {
            let name_lower = fp.name.to_lowercase();
            let matched = fp
                .part_number
                .as_ref()
                .and_then(|n| by_number.get_mut(n).and_then(|v| v.pop()))
                .or_else(|| {
                    by_unnumbered_name
                        .get_mut(&name_lower)
                        .and_then(|v| v.pop())
                })
                .filter(|i| !claimed.contains(i));
            match matched {
                Some(i) => {
                    claimed.insert(i);
                    let cur = &existing_parts[i];
                    let changed = cur.name != fp.name
                        || cur.part_number != fp.part_number
                        || cur.category != fp.category
                        || cur.notes != fp.notes
                        || cur.diagram_x != fp.diagram_x
                        || cur.diagram_y != fp.diagram_y;
                    if changed {
                        query(
                            "UPDATE catalog_parts
                             SET name = ?1, part_number = ?2, category = ?3, notes = ?4,
                                 diagram_x = ?5, diagram_y = ?6,
                                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                             WHERE id = ?7",
                        )
                        .bind(&fp.name)
                        .bind(&fp.part_number)
                        .bind(&fp.category)
                        .bind(&fp.notes)
                        .bind(fp.diagram_x)
                        .bind(fp.diagram_y)
                        .bind(cur.id)
                        .execute(&mut *tx)
                        .await?;
                        result.parts_updated += 1;
                    } else {
                        result.parts_unchanged += 1;
                    }
                }
                None => {
                    query(
                        "INSERT INTO catalog_parts (catalog_model_id, name, part_number,
                                                    category, notes, diagram_x, diagram_y)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    )
                    .bind(model_id)
                    .bind(&fp.name)
                    .bind(&fp.part_number)
                    .bind(&fp.category)
                    .bind(&fp.notes)
                    .bind(fp.diagram_x)
                    .bind(fp.diagram_y)
                    .execute(&mut *tx)
                    .await?;
                    result.parts_created += 1;
                }
            }
        }
        for (i, p) in existing_parts.iter().enumerate() {
            if !claimed.contains(&i) {
                result.orphaned_parts.push((p.id, p.name.clone()));
            }
        }
        tx.commit().await?;
        Ok(result)
    }

    async fn find_catalog_model_by_source(
        &self,
        source_file: &str,
        checksum: &str,
    ) -> Result<Option<CatalogModel>, DomainError> {
        let row = query_as::<_, CatalogModel>(
            "SELECT cm.id, cm.manufacturer_id, cmf.name AS manufacturer, cm.name, cm.category,
                    cm.diagram_asset, cm.source_file, cm.source_checksum
             FROM catalog_models cm
             JOIN catalog_manufacturers cmf ON cmf.id = cm.manufacturer_id
             WHERE cm.source_file = ?1 AND cm.source_checksum = ?2",
        )
        .bind(source_file)
        .bind(checksum)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn list_catalog_manufacturers(&self) -> Result<Vec<CatalogManufacturer>, DomainError> {
        let rows = query_as::<_, CatalogManufacturer>(
            "SELECT m.*,                    (SELECT COUNT(*) FROM catalog_models cm WHERE cm.manufacturer_id = m.id)                        AS model_count              FROM catalog_manufacturers m              ORDER BY m.name COLLATE NOCASE ASC, m.id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn catalog_manufacturer_exists(&self, id: i64) -> Result<bool, DomainError> {
        let row: Option<i64> = query_scalar("SELECT id FROM catalog_manufacturers WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    async fn list_catalog_models(
        &self,
        manufacturer_id: i64,
    ) -> Result<Vec<CatalogModel>, DomainError> {
        let rows = query_as::<_, CatalogModel>(
            "SELECT cm.id, cm.manufacturer_id, cmf.name AS manufacturer, cm.name, cm.category,
                    cm.diagram_asset, cm.source_file, cm.source_checksum
             FROM catalog_models cm
             JOIN catalog_manufacturers cmf ON cmf.id = cm.manufacturer_id
             WHERE cm.manufacturer_id = ?1
             ORDER BY cm.name COLLATE NOCASE ASC, cm.id ASC",
        )
        .bind(manufacturer_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn get_catalog_model(&self, id: i64) -> Result<Option<CatalogModel>, DomainError> {
        let row = query_as::<_, CatalogModel>(
            "SELECT cm.id, cm.manufacturer_id, cmf.name AS manufacturer, cm.name, cm.category,
                    cm.diagram_asset, cm.source_file, cm.source_checksum
             FROM catalog_models cm
             JOIN catalog_manufacturers cmf ON cmf.id = cm.manufacturer_id
             WHERE cm.id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn list_catalog_parts(
        &self,
        catalog_model_id: i64,
    ) -> Result<Vec<CatalogPart>, DomainError> {
        let rows = query_as::<_, CatalogPart>(
            "SELECT * FROM catalog_parts WHERE catalog_model_id = ?1 ORDER BY id",
        )
        .bind(catalog_model_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn get_catalog_part(&self, id: i64) -> Result<Option<CatalogPart>, DomainError> {
        let row = query_as::<_, CatalogPart>("SELECT * FROM catalog_parts WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    async fn delete_catalog_part(&self, id: i64) -> Result<bool, DomainError> {
        let res = query("DELETE FROM catalog_parts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn set_model_catalog_link(
        &self,
        model_id: i64,
        catalog_model_id: Option<i64>,
    ) -> Result<(), DomainError> {
        query(
            "UPDATE models SET catalog_model_id = ?1,                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?2",
        )
        .bind(catalog_model_id)
        .bind(model_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn find_linked_inventory_part(
        &self,
        catalog_part_id: i64,
        model_id: i64,
    ) -> Result<Option<Part>, DomainError> {
        let row = query_as::<_, Part>(
            "SELECT p.* FROM parts p
             JOIN model_parts mp ON mp.part_id = p.id
             WHERE p.catalog_part_id = ?1 AND mp.model_id = ?2
             ORDER BY p.id LIMIT 1",
        )
        .bind(catalog_part_id)
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn create_part_from_catalog(
        &self,
        catalog_part_id: i64,
        name: &str,
        link: Option<&str>,
        quantity: i32,
        model_id: i64,
    ) -> Result<Part, DomainError> {
        let mut tx = self.pool.begin().await?;
        let part = query_as::<_, Part>(
            "INSERT INTO parts (name, quantity, link, catalog_part_id)
             VALUES (?1, ?2, ?3, ?4) RETURNING *",
        )
        .bind(name)
        .bind(quantity)
        .bind(link)
        .bind(catalog_part_id)
        .fetch_one(&mut *tx)
        .await?;
        query("INSERT OR IGNORE INTO model_parts (model_id, part_id) VALUES (?1, ?2)")
            .bind(model_id)
            .bind(part.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(part)
    }

    async fn catalog_owned_quantities(
        &self,
        catalog_model_id: i64,
        model_ids: &[i64],
    ) -> Result<BTreeMap<i64, i32>, DomainError> {
        if model_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        // A part counts only when it is tied to the catalog part AND linked
        // to one of the candidate models; each inventory part is counted
        // once. The gate lives in a conditional aggregate (not a WHERE), so
        // every catalog part appears in the result: no inventory at all -> 0,
        // inventory on another model -> 0, only this model's stock counts.
        let mut qb = QueryBuilder::<Sqlite>::new(String::from(
            "SELECT cp.id, COALESCE(SUM(CASE WHEN p.id IN (SELECT part_id FROM model_parts \
             WHERE model_id IN (",
        ));
        qb.push_values(model_ids, |mut w, id| {
            w.push_bind(id);
        });
        qb.push(
            " )) THEN p.quantity END), 0) AS qty FROM catalog_parts cp LEFT JOIN parts p ON \
             p.catalog_part_id = cp.id WHERE cp.catalog_model_id = ",
        );
        qb.push_bind(catalog_model_id);
        qb.push(" GROUP BY cp.id");
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i64,
            qty: i64,
        }
        let rows = qb.build_query_as::<Row>().fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.id, r.qty.clamp(0, i32::MAX as i64) as i32))
            .collect())
    }

    async fn list_models_for_catalog_model(
        &self,
        catalog_model_id: i64,
    ) -> Result<Vec<(i64, String)>, DomainError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i64,
            name: String,
        }
        let rows = query_as::<_, Row>(
            "SELECT id, name FROM models
             WHERE catalog_model_id = ?1
             ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .bind(catalog_model_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.id, r.name)).collect())
    }

    async fn list_catalog_model_sources(&self) -> Result<Vec<(i64, String, String)>, DomainError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i64,
            name: String,
            source_file: String,
        }
        let rows =
            query_as::<_, Row>("SELECT id, name, source_file FROM catalog_models ORDER BY id")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.id, r.name, r.source_file))
            .collect())
    }
}
