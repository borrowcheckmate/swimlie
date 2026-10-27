use super::*;
use crate::model::parse_workout;

mod flow;
mod results;

/// One block with the given sets in a 25 m pool, e.g. `r#"{"reps": 2, "distance_m": 50}"#`.
fn engine(sets: &str) -> Engine {
    engine_with(sets, vec![])
}

fn engine_with(sets: &str, records: Vec<PersonalRecord>) -> Engine {
    let json = format!(
        r#"{{"version": 1, "name": "t", "pool_length_m": 25, "blocks": [{{"name": "b", "sets": [{sets}]}}]}}"#
    );
    Engine::new(parse_workout(json).unwrap(), None, 0, records).unwrap()
}

/// Lengths of `secs` each starting at `start`, with `turn` seconds at the wall. Returns the end time.
fn swim(e: &mut Engine, start: f64, n: usize, secs: f64) -> f64 {
    let mut t = start;
    for _ in 0..n {
        e.on_lap(
            Lap {
                start_s: t,
                end_s: t + secs,
                stroke: Stroke::Freestyle,
                strokes: Some(10),
            },
            t + secs + 5.0,
        );
        t += secs + 2.0;
    }
    t - 2.0
}

fn reps(e: &Engine) -> Vec<u32> {
    e.session().lap_positions.iter().map(|p| p.rep).collect()
}
