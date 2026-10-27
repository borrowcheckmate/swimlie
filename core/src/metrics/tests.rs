use super::*;

mod creator;
mod laps_and_weeks;
mod records;
mod reps;

use laps_and_weeks::*;
use records::*;
#[test]
fn duration_and_hr_zone() {
    let w = crate::model::parse_workout(
        r#"{"version":1,"name":"t","pool_length_m":25,"blocks":[{"name":"b","repeat":2,"sets":[
        {"reps":2,"distance_m":100,"rest":{"type":"fixed","seconds":20}},
        {"reps":1,"distance_m":50,"goal_s":40,"rest":{"type":"interval","seconds":30}},
        {"reps":1,"distance_m":50,"goal_s":40,"rest":{"type":"interval","seconds":60}}]}]}"#
            .into(),
    )
    .unwrap();
    // CSS 90: 2 x (90 + 20) + 40 + 60 = 320 per round, twice, plus the countdown.
    assert_eq!(estimated_duration_s(w.clone(), Some(90.0)), 650.0);
    assert_eq!(
        estimated_duration_s(w, None),
        2.0f64.mul_add(2.0f64.mul_add(140.0, 40.0) + 60.0, 10.0)
    );
    assert_eq!(
        [100.0, 120.0, 140.0, 160.0, 180.0].map(|b| hr_zone(b, 30)),
        [Some(1), Some(2), Some(3), Some(4), Some(5)]
    );
    assert_eq!(hr_zone(0.0, 30), None);
}

#[test]
fn target_zones_of_sprint() {
    assert_eq!(target_zones(Intensity::Sprint), vec![4, 5]);
}

use crate::model::{HrSample, Lap, Stroke, Workout};

fn pos(set: u32, rep: u32) -> Position {
    Position {
        block: 0,
        round: 0,
        set,
        rep,
    }
}

/// `n` one-length laps of `dur` s starting at `start`, with 2 s turns.
fn rep(
    p: Position,
    start: f64,
    dur: f64,
    n: usize,
    strokes: Option<u32>,
) -> Vec<(Lap, Position, u32)> {
    (0..n)
        .map(|i| {
            let s = (i as f64).mul_add(dur + 2.0, start);
            (
                Lap {
                    start_s: s,
                    end_s: s + dur,
                    stroke: Stroke::Freestyle,
                    strokes,
                },
                p,
                1,
            )
        })
        .collect()
}

fn session(parts: Vec<(Lap, Position, u32)>) -> Session {
    let workout = Workout {
        version: 1,
        name: "t".into(),
        pool_length_m: 25,
        blocks: vec![],
        description: None,
    };
    Session {
        workout,
        started_at_ms: 0,
        workout_id: None,
        laps: parts.iter().map(|p| p.0).collect(),
        lap_positions: parts.iter().map(|p| p.1).collect(),
        lap_lengths: parts.iter().map(|p| p.2).collect(),
        hr: vec![],
        age_years: None,
        location: None,
    }
}
