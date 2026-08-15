//! SQLite implementation of [`HangarRepo`].

use async_trait::async_trait;
use sqlx::sqlite::SqlitePool;
use sqlx::{query, query_as, query_scalar};

use super::HangarRepo;
use crate::error::DomainError;
use crate::types::{
    like_pattern, Category, Model, ModelInput, ModelListRow, Part, PartInput, PartListRow,
    PartSort, Settings,
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
            "INSERT INTO parts (name, quantity, notes, link, photo_url, cost, vendor) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING *",
        )
        .bind(&input.name)
        .bind(input.quantity as i32)
        .bind(&input.notes)
        .bind(&input.link)
        .bind(&input.photo_url)
        .bind(input.cost)
        .bind(&input.vendor)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn update_part(&self, id: i64, input: &PartInput) -> Result<Option<Part>, DomainError> {
        let row = query_as::<_, Part>(
            "UPDATE parts SET name = ?1, quantity = ?2, notes = ?3, link = ?4, \
              photo_url = ?5, cost = ?6, vendor = ?7, \
              updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?8 RETURNING *",
        )
        .bind(&input.name)
        .bind(input.quantity as i32)
        .bind(&input.notes)
        .bind(&input.link)
        .bind(&input.photo_url)
        .bind(input.cost)
        .bind(&input.vendor)
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
}
