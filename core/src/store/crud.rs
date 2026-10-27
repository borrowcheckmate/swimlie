use super::{
    Connection, MIGRATIONS, Mutex, OptionalExtension, PersonalRecord, Plan, Session, Stamps, Store,
    StoreError, StoredPlan, StoredSession, StoredWorkout, SyncBatch, Workout, head_row, invalid,
    json_of, mark_deleted, plan_rows, session_rows, soft_delete, timestamps, timestamps_of,
    to_plan, to_session, to_workout, validate_session, workout_rows, write_plan, write_session,
    write_workout,
};

#[uniffi::export]
impl Store {
    #[uniffi::constructor]
    pub fn open(path: &str) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let supported = MIGRATIONS.len() as i64;
        if version > supported {
            return Err(StoreError::TooNew {
                found: version,
                supported,
            });
        }
        for (from, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", from as i64 + 1)?;
            tx.commit()?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn save_workout(
        &self,
        id: Option<String>,
        workout: &Workout,
    ) -> Result<StoredWorkout, StoreError> {
        workout.validate().map_err(|e| invalid(e.to_string()))?;
        let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let (created, updated) = timestamps(timestamps_of(&tx, "workouts", &id)?);
        write_workout(&tx, &id, workout, Stamps::live(created, updated))?;
        tx.commit()?;
        Ok(StoredWorkout {
            id,
            created_at_ms: created,
            updated_at_ms: updated,
            workout: workout.clone(),
        })
    }

    /// Inserts a fixed-id sample workout with timestamps 0, so any real edit or delete from another
    /// device wins sync. Only if no row with `id` exists (tombstones count). Returns whether it inserted.
    pub fn seed_workout(&self, id: String, workout: &Workout) -> Result<bool, StoreError> {
        workout.validate().map_err(|e| invalid(e.to_string()))?;
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        if timestamps_of(&tx, "workouts", &id)?.is_some() {
            return Ok(false);
        }
        write_workout(&tx, &id, workout, Stamps::live(0, 0))?;
        tx.commit()?;
        Ok(true)
    }

    pub fn get_workout(&self, id: &str) -> Result<Option<StoredWorkout>, StoreError> {
        let conn = self.conn();
        let row = conn.query_row("SELECT id, json, created_at, updated_at FROM workouts WHERE id = ?1 AND deleted_at IS NULL", [id], head_row).optional()?;
        row.map(to_workout).transpose()
    }

    pub fn list_workouts(&self) -> Result<Vec<StoredWorkout>, StoreError> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, json, created_at, updated_at FROM workouts WHERE deleted_at IS NULL ORDER BY updated_at DESC, id")?;
        let rows = stmt
            .query_map([], head_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(to_workout).collect()
    }

    pub fn delete_workout(&self, id: &str) -> Result<bool, StoreError> {
        soft_delete(self.conn().unchecked_transaction()?, "workouts", id)
    }

    pub fn save_plan(&self, id: Option<String>, plan: &Plan) -> Result<StoredPlan, StoreError> {
        plan.validate().map_err(|e| invalid(e.to_string()))?;
        let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let (created, updated) = timestamps(timestamps_of(&tx, "plans", &id)?);
        write_plan(&tx, &id, plan, Stamps::live(created, updated))?;
        tx.commit()?;
        Ok(StoredPlan {
            id,
            created_at_ms: created,
            updated_at_ms: updated,
            plan: plan.clone(),
        })
    }

    pub fn list_plans(&self) -> Result<Vec<StoredPlan>, StoreError> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, json, created_at, updated_at FROM plans WHERE deleted_at IS NULL ORDER BY updated_at DESC, id")?;
        let rows = stmt
            .query_map([], head_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(to_plan).collect()
    }

    pub fn delete_plan(&self, id: &str) -> Result<bool, StoreError> {
        soft_delete(self.conn().unchecked_transaction()?, "plans", id)
    }

    pub fn save_session(
        &self,
        id: Option<String>,
        session: &Session,
    ) -> Result<StoredSession, StoreError> {
        validate_session(session)?;
        let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let (created, updated) = timestamps(timestamps_of(&tx, "sessions", &id)?);
        write_session(&tx, &id, session, Stamps::live(created, updated))?;
        tx.commit()?;
        Ok(StoredSession {
            id,
            created_at_ms: created,
            updated_at_ms: updated,
            session: session.clone(),
        })
    }

    pub fn get_session(&self, id: &str) -> Result<Option<StoredSession>, StoreError> {
        let conn = self.conn();
        let row = conn.query_row("SELECT id, workout_json, created_at, updated_at FROM sessions WHERE id = ?1 AND deleted_at IS NULL", [id], head_row).optional()?;
        row.map(|r| to_session(&conn, r)).transpose()
    }

    pub fn list_sessions(&self) -> Result<Vec<StoredSession>, StoreError> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, workout_json, created_at, updated_at FROM sessions WHERE deleted_at IS NULL ORDER BY created_at DESC, id")?;
        let rows = stmt
            .query_map([], head_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(|r| to_session(&conn, r)).collect()
    }

    pub fn personal_records(&self) -> Result<Vec<PersonalRecord>, StoreError> {
        Ok(crate::metrics::personal_records(&self.list_sessions()?))
    }

    /// Soft-deletes the session and its CSS result (same id); its laps are dropped since a tombstone needs only id and timestamps.
    pub fn delete_session(&self, id: &str) -> Result<bool, StoreError> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM laps WHERE session_id = ?1", [id])?;
        mark_deleted(&tx, "css_results", id)?;
        let n = mark_deleted(&tx, "sessions", id)?;
        tx.commit()?;
        Ok(n)
    }

    /// Every workout and session row, tombstones included, as a `SyncBatch`.
    pub fn export_changes(&self) -> Result<String, StoreError> {
        // shortcut: exports the whole store each time; add an updated_at watermark if payloads get large.
        let conn = self.conn();
        json_of(&SyncBatch {
            workouts: workout_rows(&conn, None)?,
            sessions: session_rows(&conn, None)?,
            plans: plan_rows(&conn, None)?,
        })
    }

    /// Merges another device's `export_changes` output, last-write-wins per row. Returns rows written.
    pub fn apply_changes(&self, json: &str) -> Result<u32, StoreError> {
        Ok(self.apply_counted(json)?.0)
    }
}
