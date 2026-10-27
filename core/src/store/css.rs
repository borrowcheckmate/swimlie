use super::{
    OptionalExtension, Store, StoreError, StoredCss, corrupt, css_s_per_100m, css_test_times,
    invalid, now_ms, params, soft_delete, timestamps, timestamps_of,
};

impl Store {
    pub fn delete_css(&self, id: &str) -> Result<bool, StoreError> {
        soft_delete(self.conn().unchecked_transaction()?, "css_results", id)
    }

    pub fn save_css(
        &self,
        id: Option<String>,
        t400_s: f64,
        t200_s: f64,
    ) -> Result<StoredCss, StoreError> {
        if css_s_per_100m(t400_s, t200_s).is_none() {
            return Err(invalid("CSS needs finite times with 0 < t200 < t400"));
        }
        let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let (created, updated) = timestamps(timestamps_of(&tx, "css_results", &id)?);
        tx.execute(
            "INSERT INTO css_results (id, t400_s, t200_s, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET t400_s = ?2, t200_s = ?3, updated_at = ?5, deleted_at = NULL",
            params![id, t400_s, t200_s, created, updated],
        )?;
        tx.commit()?;
        Ok(StoredCss {
            id,
            created_at_ms: created,
            updated_at_ms: updated,
            t400_s,
            t200_s,
        })
    }
}

#[uniffi::export]
impl Store {
    /// CSS pace of the newest result, if any.
    pub fn latest_css_s_per_100m(&self) -> Result<Option<f64>, StoreError> {
        let row: Option<(f64, f64)> = self.conn()
            .query_row("SELECT t400_s, t200_s FROM css_results WHERE deleted_at IS NULL ORDER BY created_at DESC, id LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        row.map(|(t400, t200)| {
            css_s_per_100m(t400, t200)
                .ok_or_else(|| corrupt(format!("bad CSS times {t400}, {t200}")))
        })
        .transpose()
    }

    /// Saves the CSS of each live session that is a CSS test and has no `css_results` row under its id (tombstones count). Returns how many.
    pub fn import_css_tests(&self) -> Result<u32, StoreError> {
        let ids: Vec<String> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT id FROM sessions WHERE deleted_at IS NULL AND id NOT IN (SELECT id FROM css_results)")?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()?
        };
        let mut found = vec![];
        for id in ids {
            let Some(s) = self.get_session(&id)? else {
                continue;
            };
            if let Some(t) = css_test_times(&s.session) {
                found.push((s.id, s.session.started_at_ms, t));
            }
        }
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let mut saved = 0u32;
        for (id, started_at_ms, t) in found {
            // OR IGNORE: a row another caller saved since the select is skipped, not an error.
            saved += tx.execute("INSERT OR IGNORE INTO css_results (id, t400_s, t200_s, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![id, t.t400_s, t.t200_s, started_at_ms, now_ms()])? as u32;
        }
        tx.commit()?;
        Ok(saved)
    }
}
