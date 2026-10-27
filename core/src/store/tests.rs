use super::*;

mod css_and_records;
mod plans;
mod storage;
mod sync;

const WORKOUT: &str = r#"{"version":1,"name":"T","pool_length_m":25,"blocks":[{"name":"Main","repeat":1,"sets":[{"reps":2,"distance_m":50,"stroke":"freestyle"}]}]}"#;

fn workout() -> Workout {
    parse_workout(WORKOUT.into()).unwrap()
}

fn session() -> Session {
    let pos = |rep| Position {
        block: 0,
        round: 0,
        set: 0,
        rep,
    };
    Session {
        workout: workout(),
        started_at_ms: 1_760_000_000_000,
        workout_id: Some("w1".into()),
        laps: vec![
            Lap {
                start_s: 0.0,
                end_s: 20.5,
                stroke: Stroke::Freestyle,
                strokes: Some(18),
            },
            Lap {
                start_s: 20.5,
                end_s: 62.0,
                stroke: Stroke::Kickboard,
                strokes: None,
            },
        ],
        lap_positions: vec![pos(0), pos(1)],
        lap_lengths: vec![1, 2],
        hr: vec![],
        age_years: None,
        location: None,
    }
}

fn count(s: &Store, table: &str) -> i64 {
    s.conn
        .lock()
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
