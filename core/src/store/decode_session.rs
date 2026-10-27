use super::{
    Connection, HrSample, Lap, Location, Position, Row, Session, StoreError, StoredSession,
    corrupt, lap_row, parse_stroke, parse_workout, u32_of,
};

/// `started_at_ms`, `workout_id`, `hr_json`, `age_years` and `location_json` of a session row.
type SessionColumns = (
    i64,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
);

pub(super) fn to_session(
    conn: &Connection,
    (id, json, created_at_ms, updated_at_ms): Row,
) -> Result<StoredSession, StoreError> {
    let workout = parse_workout(json).map_err(|e| corrupt(e.to_string()))?;
    let (started_at_ms, workout_id, hr_json, age_years, location_json): SessionColumns =
        conn.query_row("SELECT started_at_ms, workout_id, hr_json, age_years, location_json FROM sessions WHERE id = ?1", [&id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
    let hr: Vec<HrSample> = hr_json
        .map(|j| {
            serde_json::from_str(&j)
                .map_err(|e| corrupt(format!("session {id}: bad heart rate: {e}")))
        })
        .transpose()?
        .unwrap_or_default();
    let age_years = age_years.map(|a| u32_of(a, "age_years")).transpose()?;
    let location: Option<Location> = location_json
        .map(|j| {
            serde_json::from_str(&j)
                .map_err(|e| corrupt(format!("session {id}: bad location: {e}")))
        })
        .transpose()?;
    let mut stmt = conn.prepare(
        "SELECT idx, start_s, end_s, stroke, strokes, lengths, block, round, set_idx, rep FROM laps WHERE session_id = ?1 ORDER BY idx",
    )?;
    let rows = stmt
        .query_map([&id], lap_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut session = Session {
        workout,
        started_at_ms,
        workout_id,
        laps: vec![],
        lap_positions: vec![],
        lap_lengths: vec![],
        hr,
        age_years,
        location,
    };
    for (i, (idx, start_s, end_s, stroke, strokes, [lengths, block, round, set, rep])) in
        rows.into_iter().enumerate()
    {
        if idx != i as i64 {
            return Err(corrupt(format!("session {id}: lap index gap at {i}")));
        }
        let lengths = u32_of(lengths, "lengths")?;
        if lengths == 0 || !start_s.is_finite() || !end_s.is_finite() {
            return Err(corrupt(format!("session {id}: bad lap {i}")));
        }
        let strokes = strokes.map(|s| u32_of(s, "strokes")).transpose()?;
        session.laps.push(Lap {
            start_s,
            end_s,
            stroke: parse_stroke(&stroke)?,
            strokes,
        });
        session.lap_lengths.push(lengths);
        session.lap_positions.push(Position {
            block: u32_of(block, "block")?,
            round: u32_of(round, "round")?,
            set: u32_of(set, "set")?,
            rep: u32_of(rep, "rep")?,
        });
    }
    Ok(StoredSession {
        id,
        created_at_ms,
        updated_at_ms,
        session,
    })
}
