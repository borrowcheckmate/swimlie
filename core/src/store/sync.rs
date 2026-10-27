use super::{
    DeserializeOwned, Plan, RawBatch, Session, Stamps, Store, StoreError, SyncRow, Workout,
    incoming_wins, invalid, plan_rows, session_rows, validate_session, workout_rows, write_plan,
    write_session, write_workout,
};

impl Store {
    /// `apply_changes` that also returns the rows skipped for failing to parse or validate.
    pub(super) fn apply_counted(&self, json: &str) -> Result<(u32, u32), StoreError> {
        let batch: RawBatch = serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        let mut skipped = 0;
        let workouts = valid_rows(
            batch.workouts,
            |r: &SyncRow<Workout>| r.data.validate().is_ok(),
            &mut skipped,
        );
        let sessions = valid_rows(
            batch.sessions,
            |r: &SyncRow<Session>| validate_session(&r.data).is_ok(),
            &mut skipped,
        );
        let plans = valid_rows(
            batch.plans,
            |r: &SyncRow<Plan>| r.data.validate().is_ok(),
            &mut skipped,
        );
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let mut written = 0;
        for row in &workouts {
            let local = workout_rows(&tx, Some(row.id.as_str()))?.pop();
            if incoming_wins(row, local.as_ref())? {
                write_workout(&tx, &row.id, &row.data, Stamps::from(row))?;
                written += 1;
            }
        }
        for row in &sessions {
            let local = session_rows(&tx, Some(row.id.as_str()))?.pop();
            if incoming_wins(row, local.as_ref())? {
                write_session(&tx, &row.id, &row.data, Stamps::from(row))?;
                written += 1;
            }
        }
        for row in &plans {
            let local = plan_rows(&tx, Some(row.id.as_str()))?.pop();
            if incoming_wins(row, local.as_ref())? {
                write_plan(&tx, &row.id, &row.data, Stamps::from(row))?;
                written += 1;
            }
        }
        tx.commit()?;
        Ok((written, skipped))
    }
}

/// Parses each raw row; rows that fail to parse or fail `valid` are counted in `skipped`.
pub(super) fn valid_rows<T: DeserializeOwned>(
    values: Vec<serde_json::Value>,
    valid: impl Fn(&SyncRow<T>) -> bool,
    skipped: &mut u32,
) -> Vec<SyncRow<T>> {
    values
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<SyncRow<T>>(v) {
            Ok(row) if valid(&row) => Some(row),
            _ => {
                *skipped += 1;
                None
            }
        })
        .collect()
}
