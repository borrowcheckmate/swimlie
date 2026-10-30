#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stderr,
    clippy::float_cmp,
    clippy::while_float,
    reason = "a replay harness over fixture files: a failure should abort with the fixture's own error"
)]

//! Replays recorded swims through the engine and checks every labelled lap against the swimmer's labels.

use serde_json::Value;
use swimlie_core::engine::Engine;
use swimlie_core::metrics;
use swimlie_core::model::{Lap, Position, Session, Stroke, Workout, parse_workout};

const SWIM_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/recorded-swim-a");
const SWIM_B: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/recorded-swim-b");
const SWIM_C: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/recorded-swim-c");

fn stroke(s: &str) -> Stroke {
    serde_json::from_value(Value::String(s.into())).unwrap()
}

/// SWOLF = time + strokes, so strokes = round(swolf − duration); `None` where `HealthKit` gave no SWOLF.
fn strokes(l: &Value) -> Option<u32> {
    let d = l["end_s"].as_f64().unwrap() - l["start_s"].as_f64().unwrap();
    l["swolf"].as_f64().map(|s| (s - d).round() as u32)
}

/// (block name, 1-based set, 1-based rep), as labelled.
fn label(w: &Workout, p: Position) -> (String, u64, u64) {
    (
        w.blocks[p.block as usize].name.clone(),
        u64::from(p.set) + 1,
        u64::from(p.rep) + 1,
    )
}

/// Replays the recording in `dir`. Feeds the engine every lap from the first labelled one on (by
/// `received_s`); unlabelled laps in that range are fed but not scored, and laps the engine ignores
/// have no position. Returns the workout, the
/// final session, and (live matches, final matches, labelled laps).
fn replay(dir: &str) -> (Workout, Session, usize, usize, usize) {
    let workout =
        parse_workout(std::fs::read_to_string(format!("{dir}/workout.json")).unwrap()).unwrap();
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{dir}/laps.json")).unwrap())
            .unwrap();
    let mut laps = data["laps"].as_array().unwrap().clone();
    laps.sort_by(|a, b| {
        a["received_s"]
            .as_f64()
            .unwrap()
            .total_cmp(&b["received_s"].as_f64().unwrap())
    });
    // Laps before the first labelled one are not fed: there is no record of that part of the workout.
    let first = laps.iter().position(|l| !l["block"].is_null()).unwrap();
    let fed = &laps[first..];
    // `fed[k]` is the engine's lap k unless it ignores it; `lap_positions` has one entry per kept lap.
    let want: Vec<Option<(String, u64, u64)>> = fed
        .iter()
        .map(|l| {
            l["block"].as_str().map(|b| {
                (
                    b.to_string(),
                    l["set"].as_u64().unwrap(),
                    l["rep"].as_u64().unwrap(),
                )
            })
        })
        .collect();
    let labelled = want.iter().flatten().count();

    let mut e = Engine::new(workout.clone(), None, 0, vec![]).unwrap();
    let mut live = vec![];
    let mut t = 0.0;
    for l in fed {
        let received = l["received_s"].as_f64().unwrap();
        while t < received {
            let _ = e.tick(t);
            t += 1.0;
        }
        let lap = Lap {
            start_s: l["start_s"].as_f64().unwrap(),
            end_s: l["end_s"].as_f64().unwrap(),
            stroke: stroke(l["stroke"].as_str().unwrap()),
            strokes: strokes(l),
        };
        let before = e.session().laps.len();
        e.on_lap(lap, received);
        // What the engine decided for this lap when it arrived.
        let s = e.session();
        live.push(
            (s.laps.len() > before).then(|| label(&workout, *s.lap_positions.last().unwrap())),
        );
    }
    let session = e.session();
    let mut positions = session.lap_positions.iter();
    let fin: Vec<_> = live
        .iter()
        .map(|l| {
            l.as_ref()
                .and_then(|_| positions.next())
                .map(|&p| label(&workout, p))
        })
        .collect();

    let report = |name: &str, got: &[Option<(String, u64, u64)>]| {
        let mut ok = 0;
        for k in 0..fed.len() {
            match &want[k] {
                None => {}
                Some(w) if got[k].as_ref() == Some(w) => ok += 1,
                Some(w) => eprintln!(
                    "{name} miss: lap {} got {:?} want {:?} note: {}",
                    fed[k]["n"], got[k], w, fed[k]["note"]
                ),
            }
        }
        eprintln!("{name}: {ok}/{labelled} laps match");
        ok
    };
    let live_ok = report("live", &live);
    let final_ok = report("final", &fin);
    (workout, session, live_ok, final_ok, labelled)
}

#[test]
fn swim_a_replay() {
    let (workout, session, live_ok, final_ok, _) = replay(SWIM_A);
    // Remaining misses are explained in PLAN.md section 3; raise these when the engine improves.
    assert!(live_ok >= 67, "live accuracy regressed");
    assert!(final_ok >= 66, "final accuracy regressed");

    // Main set: 4 reps of 4 lengths; pace = 390.11 s / 400 m with turns (lap time alone was 357.727 s), SWOLF = duration + strokes per lap.
    let reps = metrics::reps(&session);
    let main_block = workout
        .blocks
        .iter()
        .position(|b| b.name == "Main")
        .unwrap() as u32;
    let main: Vec<_> = reps.iter().filter(|r| r.pos.block == main_block).collect();
    assert_eq!(main.len(), 4);
    assert!(main.iter().all(|r| r.lengths == 4));
    let pace = metrics::set_pace_s_per_100m(&reps, main_block, 0, 0).unwrap();
    assert!((pace - 97.5275).abs() < 0.01, "main pace {pace}");
    let swolf: Vec<f64> = main
        .iter()
        .flat_map(|r| r.swolf.iter().flatten().copied())
        .collect();
    let mean = swolf.iter().sum::<f64>() / swolf.len() as f64;
    eprintln!(
        "main: pace {pace:.3} s/100m, swolf mean {mean:.3} over {} laps",
        swolf.len()
    );
    assert!((mean - 31.92).abs() < 0.01, "main swolf mean {mean}");
}

#[test]
fn swim_2026_10_14_replay() {
    let (_, session, live_ok, final_ok, labelled) = replay(SWIM_B);
    assert_eq!(labelled, 38, "labelled laps 72-109");
    assert!(live_ok >= 38, "live accuracy regressed");
    assert!(final_ok >= 38, "final accuracy regressed");
    // Lap 110 (3.5 s, delivered 207 s late) is a HealthKit artefact: fed, but not in the session.
    assert_eq!(session.laps.len(), 38);
    assert!(session.laps.iter().all(|l| l.end_s - l.start_s > 10.0));
}

#[test]
fn swim_c_replay() {
    // One-length reps (two 4x25 sets) land in the right rep; their lag is in when, not where (issue #13).
    let (_, _, live_ok, final_ok, labelled) = replay(SWIM_C);
    assert_eq!(labelled, 44, "the fins block is unlabelled");
    assert!(live_ok >= 44, "live accuracy regressed");
    assert!(final_ok >= 44, "final accuracy regressed");
}

#[test]
fn swim_2026_10_14_free_sets() {
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{SWIM_B}/laps.json")).unwrap())
            .unwrap();
    let laps: Vec<Lap> = data["laps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| !l["block"].is_null())
        .map(|l| Lap {
            start_s: l["start_s"].as_f64().unwrap(),
            end_s: l["end_s"].as_f64().unwrap(),
            stroke: stroke(l["stroke"].as_str().unwrap()),
            strokes: strokes(l),
        })
        .collect();
    let sets = metrics::free_sets(laps, f64::INFINITY);
    // Real labels: 8x50 paddles, 100 kickboard, 4x50 paddles, 4x50 breast, cool-down. Rests between reps (16-34 s) are as long
    // as the rests between blocks (20-70 s), so no single gap splits the blocks: each 50 is a set. Cool-down's 17.9 s stop splits too.
    let lengths: Vec<u32> = sets.iter().map(|s| s.lengths).collect();
    assert_eq!(
        lengths,
        [2, 2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1]
    );
    assert!(sets.iter().all(|s| s.done));
    assert_eq!(
        (sets[8].start_s, sets[8].end_s),
        (4085.624, 4360.423),
        "kickboard"
    );
    assert_eq!((sets[0].start_s, sets[18].end_s), (3548.689, 5147.489));
}
