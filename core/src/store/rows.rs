use super::{
    Connection, OptionalExtension, Plan, Serialize, Session, StoreError, StoredPlan, StoredWorkout,
    Stroke, SyncRow, SystemTime, Transaction, UNIX_EPOCH, Workout, corrupt, invalid, params,
    parse_plan, parse_workout, plan_to_json, to_session, workout_to_json,
};
use crate::model::Location;

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

pub(super) fn u32_of(v: i64, what: &str) -> Result<u32, StoreError> {
    u32::try_from(v).map_err(|_| corrupt(format!("{what} out of range: {v}")))
}

pub(super) fn stroke_name(s: Stroke) -> String {
    match serde_json::to_value(s) {
        Ok(serde_json::Value::String(n)) => n,
        _ => unreachable!("Stroke serializes to a string"),
    }
}

pub(super) fn parse_stroke(name: &str) -> Result<Stroke, StoreError> {
    serde_json::from_value(serde_json::Value::String(name.into()))
        .map_err(|_| corrupt(format!("unknown stroke '{name}'")))
}

/// `created_at` stays, `updated_at` = max(now, old + 1) so it always increases.
pub(super) fn timestamps(old: Option<(i64, i64)>) -> (i64, i64) {
    let now = now_ms();
    match old {
        Some((created, updated)) => (created, now.max(updated + 1)),
        None => (now, now),
    }
}

pub(super) type Row = (String, String, i64, i64);
pub(super) type LapRow = (i64, f64, f64, String, Option<i64>, [i64; 5]);

pub(super) fn head_row(r: &rusqlite::Row) -> rusqlite::Result<Row> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
}

pub(super) fn lap_row(r: &rusqlite::Row) -> rusqlite::Result<LapRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        [r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?],
    ))
}

pub(super) fn to_workout(
    (id, json, created_at_ms, updated_at_ms): Row,
) -> Result<StoredWorkout, StoreError> {
    let workout = parse_workout(json).map_err(|e| corrupt(e.to_string()))?;
    Ok(StoredWorkout {
        id,
        created_at_ms,
        updated_at_ms,
        workout,
    })
}

pub(super) fn to_plan(
    (id, json, created_at_ms, updated_at_ms): Row,
) -> Result<StoredPlan, StoreError> {
    let plan = parse_plan(json).map_err(|e| corrupt(e.to_string()))?;
    Ok(StoredPlan {
        id,
        created_at_ms,
        updated_at_ms,
        plan,
    })
}

pub(super) fn timestamps_of(
    conn: &Connection,
    table: &str,
    id: &str,
) -> Result<Option<(i64, i64)>, StoreError> {
    // table is only ever a literal from this module
    Ok(conn
        .query_row(
            &format!("SELECT created_at, updated_at FROM {table} WHERE id = ?1"),
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

/// Marks a live row deleted and bumps `updated_at` inside `tx`; false if no live row.
pub(super) fn mark_deleted(tx: &Transaction, table: &str, id: &str) -> Result<bool, StoreError> {
    let Some(old) = timestamps_of(tx, table, id)? else {
        return Ok(false);
    };
    let (_, updated) = timestamps(Some(old));
    // table is only ever a literal from this module
    let n = tx.execute(&format!("UPDATE {table} SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL"), params![id, updated])?;
    Ok(n > 0)
}

/// Marks a live row deleted and bumps `updated_at`; commits `tx`. False if no live row.
pub(super) fn soft_delete(tx: Transaction, table: &str, id: &str) -> Result<bool, StoreError> {
    let n = mark_deleted(&tx, table, id)?;
    tx.commit()?;
    Ok(n)
}

pub(super) type SyncHead = (Row, Option<i64>);

pub(super) fn sync_head(r: &rusqlite::Row) -> rusqlite::Result<SyncHead> {
    Ok(((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?), r.get(4)?))
}

/// Workout rows incl. tombstones; only the row `id` if given.
pub(super) fn workout_rows(
    conn: &Connection,
    id: Option<&str>,
) -> Result<Vec<SyncRow<Workout>>, StoreError> {
    let mut stmt = conn.prepare("SELECT id, json, created_at, updated_at, deleted_at FROM workouts WHERE ?1 IS NULL OR id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([id], sync_head)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(row, deleted_at_ms)| {
            let w = to_workout(row)?;
            Ok(SyncRow {
                id: w.id,
                created_at_ms: w.created_at_ms,
                updated_at_ms: w.updated_at_ms,
                deleted_at_ms,
                data: w.workout,
            })
        })
        .collect()
}

/// Session rows incl. tombstones; only the row `id` if given.
pub(super) fn session_rows(
    conn: &Connection,
    id: Option<&str>,
) -> Result<Vec<SyncRow<Session>>, StoreError> {
    let mut stmt = conn.prepare("SELECT id, workout_json, created_at, updated_at, deleted_at FROM sessions WHERE ?1 IS NULL OR id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([id], sync_head)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(row, deleted_at_ms)| {
            let s = to_session(conn, row)?;
            Ok(SyncRow {
                id: s.id,
                created_at_ms: s.created_at_ms,
                updated_at_ms: s.updated_at_ms,
                deleted_at_ms,
                data: s.session,
            })
        })
        .collect()
}

/// Plan rows incl. tombstones; only the row `id` if given.
pub(super) fn plan_rows(
    conn: &Connection,
    id: Option<&str>,
) -> Result<Vec<SyncRow<Plan>>, StoreError> {
    let mut stmt = conn.prepare("SELECT id, json, created_at, updated_at, deleted_at FROM plans WHERE ?1 IS NULL OR id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([id], sync_head)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(row, deleted_at_ms)| {
            let p = to_plan(row)?;
            Ok(SyncRow {
                id: p.id,
                created_at_ms: p.created_at_ms,
                updated_at_ms: p.updated_at_ms,
                deleted_at_ms,
                data: p.plan,
            })
        })
        .collect()
}

pub(super) fn json_of<T: Serialize>(v: &T) -> Result<String, StoreError> {
    serde_json::to_string(v).map_err(|e| corrupt(e.to_string()))
}

/// Last-write-wins. On a tie a tombstone wins, then the longer row (a missing optional field loses
/// to the fuller row), then the larger JSON, so both devices keep the same row.
pub(super) fn incoming_wins<T: Serialize>(
    incoming: &SyncRow<T>,
    local: Option<&SyncRow<T>>,
) -> Result<bool, StoreError> {
    let Some(local) = local else { return Ok(true) };
    if incoming.updated_at_ms != local.updated_at_ms {
        return Ok(incoming.updated_at_ms > local.updated_at_ms);
    }
    if incoming.deleted_at_ms.is_some() != local.deleted_at_ms.is_some() {
        return Ok(incoming.deleted_at_ms.is_some());
    }
    let (a, b) = (json_of(incoming)?, json_of(local)?);
    Ok((a.len(), &a) > (b.len(), &b))
}

/// Timestamps and tombstone state a row is written with.
#[derive(Clone, Copy, Debug)]
pub(super) struct Stamps {
    pub(super) created: i64,
    pub(super) updated: i64,
    pub(super) deleted: Option<i64>,
}

impl Stamps {
    /// A live row: not deleted.
    pub(super) const fn live(created: i64, updated: i64) -> Self {
        Self {
            created,
            updated,
            deleted: None,
        }
    }
}

impl<T> From<&SyncRow<T>> for Stamps {
    fn from(row: &SyncRow<T>) -> Self {
        Self {
            created: row.created_at_ms,
            updated: row.updated_at_ms,
            deleted: row.deleted_at_ms,
        }
    }
}

/// Upserts keeping the given timestamps and tombstone state.
pub(super) fn write_workout(
    tx: &Transaction<'_>,
    id: &str,
    workout: &Workout,
    stamps: Stamps,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO workouts (id, json, created_at, updated_at, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET json = ?2, created_at = ?3, updated_at = ?4, deleted_at = ?5",
        params![id, workout_to_json(workout.clone()), stamps.created, stamps.updated, stamps.deleted],
    )?;
    Ok(())
}

pub(super) fn write_plan(
    tx: &Transaction<'_>,
    id: &str,
    plan: &Plan,
    stamps: Stamps,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO plans (id, json, created_at, updated_at, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET json = ?2, created_at = ?3, updated_at = ?4, deleted_at = ?5",
        params![id, plan_to_json(plan.clone()), stamps.created, stamps.updated, stamps.deleted],
    )?;
    Ok(())
}

pub(super) fn write_session(
    tx: &Transaction<'_>,
    id: &str,
    session: &Session,
    stamps: Stamps,
) -> Result<(), StoreError> {
    let hr_json = (!session.hr.is_empty())
        .then(|| json_of(&session.hr))
        .transpose()?;
    let location_json = session.location.as_ref().map(json_of).transpose()?;
    tx.execute(
        "INSERT INTO sessions (id, workout_json, started_at_ms, workout_id, created_at, updated_at, deleted_at, hr_json, age_years, location_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET workout_json = ?2, started_at_ms = ?3, workout_id = ?4, created_at = ?5, updated_at = ?6, deleted_at = ?7, hr_json = ?8, age_years = ?9, location_json = ?10",
        params![id, workout_to_json(session.workout.clone()), session.started_at_ms, session.workout_id, stamps.created, stamps.updated, stamps.deleted, hr_json, session.age_years, location_json],
    )?;
    tx.execute("DELETE FROM laps WHERE session_id = ?1", [id])?;
    let mut ins =
        tx.prepare("INSERT INTO laps VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)")?;
    for (i, lap) in session.laps.iter().enumerate() {
        let p = session.lap_positions[i];
        ins.execute(params![
            id,
            i as i64,
            lap.start_s,
            lap.end_s,
            stroke_name(lap.stroke),
            lap.strokes,
            session.lap_lengths[i],
            p.block,
            p.round,
            p.set,
            p.rep
        ])?;
    }
    Ok(())
}

/// A coordinate on the globe with a finite, non-negative accuracy.
fn is_valid_location(l: &Location) -> bool {
    (-90.0..=90.0).contains(&l.lat)
        && (-180.0..=180.0).contains(&l.lon)
        && l.accuracy_m.is_finite()
        && l.accuracy_m >= 0.0
}

pub(super) fn validate_session(session: &Session) -> Result<(), StoreError> {
    session
        .workout
        .validate()
        .map_err(|e| invalid(e.to_string()))?;
    let n = session.laps.len();
    if session.lap_positions.len() != n || session.lap_lengths.len() != n {
        return Err(invalid(
            "lap_positions and lap_lengths must be parallel to laps",
        ));
    }
    if session.lap_lengths.contains(&0) {
        return Err(invalid("lap lengths must be >= 1"));
    }
    // SQLite stores NaN as NULL, which would not read back.
    if session
        .laps
        .iter()
        .any(|l| !l.start_s.is_finite() || !l.end_s.is_finite())
    {
        return Err(invalid("lap times must be finite"));
    }
    if session
        .hr
        .iter()
        .any(|h| !h.t_s.is_finite() || !h.bpm.is_finite() || h.bpm <= 0.0)
    {
        return Err(invalid("heart-rate samples must be finite with bpm > 0"));
    }
    if session.hr.windows(2).any(|w| w[1].t_s < w[0].t_s) {
        return Err(invalid("heart-rate samples must be in time order"));
    }
    if session.location.is_some_and(|l| !is_valid_location(&l)) {
        return Err(invalid(
            "location needs lat in -90..=90, lon in -180..=180 and accuracy_m >= 0",
        ));
    }
    Ok(())
}
