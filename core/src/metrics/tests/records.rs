use super::*;

pub(super) fn pr_workout() -> Workout {
    let w = |distance_m, stroke| WorkoutSet {
        reps: 1,
        distance_m,
        stroke,
        intensity: Intensity::default(),
        goal_s: None,
        equipment: vec![],
        drill: None,
        rest: None,
        notes: None,
    };
    let sets = vec![w(50, Stroke::Freestyle), w(50, Stroke::Any)];
    Workout {
        version: 1,
        name: "t".into(),
        pool_length_m: 25,
        blocks: vec![crate::model::Block {
            name: "b".into(),
            repeat: 1,
            sets,
        }],
        description: None,
    }
}

pub(super) fn lap(
    start_s: f64,
    end_s: f64,
    stroke: Stroke,
    length: u32,
    set: u32,
) -> (Lap, Position, u32) {
    (
        Lap {
            start_s,
            end_s,
            stroke,
            strokes: None,
        },
        pos(set, 0),
        length,
    )
}

pub(super) fn stored(id: &str, parts: Vec<(Lap, Position, u32)>) -> StoredSession {
    let mut session = session(parts);
    session.workout = pr_workout();
    StoredSession {
        id: id.into(),
        created_at_ms: 0,
        updated_at_ms: 0,
        session,
    }
}

#[test]
fn exercise_reps_are_no_records() {
    let store = crate::store::Store::open(":memory:").unwrap();
    let fr = Stroke::Freestyle;
    // a: set 0 (a plain 50) in 44 s. b: the same set as a drill, 40 s, which would beat a but is no record.
    let a = stored(
        "a",
        vec![lap(0.0, 22.0, fr, 1, 0), lap(24.0, 46.0, fr, 1, 0)],
    );
    let mut b = stored(
        "b",
        vec![lap(0.0, 20.0, fr, 1, 0), lap(22.0, 42.0, fr, 1, 0)],
    );
    b.session.workout.blocks[0].sets[0].drill = Some("catch-up".into());
    for s in [&a, &b] {
        store.save_session(Some(s.id.clone()), &s.session).unwrap();
    }
    let prs = personal_records(&store.list_sessions().unwrap());
    let got: Vec<_> = prs
        .iter()
        .map(|p| (p.distance_m, p.time_s, p.session_id.as_str()))
        .collect();
    assert_eq!(got, vec![(50, 46.0, "a")]);
}

#[test]
fn records_pick_best_and_skip_guesses() {
    let store = crate::store::Store::open(":memory:").unwrap();
    let (fr, bk) = (Stroke::Freestyle, Stroke::Backstroke);
    // a: set 0 in 44 s; set 1 (planned Any) all backstroke in 40 s.
    let a = stored(
        "a",
        vec![
            lap(0.0, 22.0, fr, 1, 0),
            lap(24.0, 44.0, fr, 1, 0),
            lap(100.0, 120.0, bk, 1, 1),
            lap(120.0, 140.0, bk, 1, 1),
        ],
    );
    // b: set 0 in 42 s (better); set 1 has mixed strokes, so no record.
    let b = stored(
        "b",
        vec![
            lap(0.0, 21.0, fr, 1, 0),
            lap(23.0, 42.0, fr, 1, 0),
            lap(100.0, 120.0, bk, 1, 1),
            lap(120.0, 130.0, fr, 1, 1),
        ],
    );
    // c: a short rep (1 length) and a rep reached only via a merged lap: both excluded despite fast times.
    let c = stored(
        "c",
        vec![lap(0.0, 10.0, fr, 1, 0), lap(100.0, 105.0, bk, 2, 1)],
    );
    for s in [&a, &b, &c] {
        store.save_session(Some(s.id.clone()), &s.session).unwrap();
    }
    let prs = personal_records(&store.list_sessions().unwrap());
    let got: Vec<_> = prs
        .iter()
        .map(|p| {
            (
                p.distance_m,
                p.stroke,
                p.time_s,
                p.session_id.as_str(),
                p.pos.set,
            )
        })
        .collect();
    assert_eq!(got, vec![(50, fr, 42.0, "b", 0), (50, bk, 40.0, "a", 1)]);
}

#[test]
fn records_are_per_equipment() {
    let store = crate::store::Store::open(":memory:").unwrap();
    let fr = Stroke::Freestyle;
    // a: plain 50 in 44 s. p: the same set with paddles in 40 s, faster but its own record.
    let a = stored(
        "a",
        vec![lap(0.0, 22.0, fr, 1, 0), lap(24.0, 46.0, fr, 1, 0)],
    );
    let mut p = stored(
        "p",
        vec![lap(0.0, 20.0, fr, 1, 0), lap(22.0, 42.0, fr, 1, 0)],
    );
    p.session.workout.blocks[0].sets[0].equipment = vec![Equipment::Paddles];
    for s in [&a, &p] {
        store.save_session(Some(s.id.clone()), &s.session).unwrap();
    }
    let prs = personal_records(&store.list_sessions().unwrap());
    let got: Vec<_> = prs
        .iter()
        .map(|r| {
            (
                r.distance_m,
                r.time_s,
                r.session_id.as_str(),
                r.equipment.clone(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (50, 46.0, "a", vec![]),
            (50, 42.0, "p", vec![Equipment::Paddles])
        ]
    );
}

pub(super) fn state(
    pos: Position,
    phase: Phase,
    rest_remaining_s: Option<f64>,
    rep_elapsed_s: Option<f64>,
) -> State {
    State {
        pos,
        done_m: 0,
        target_m: 0,
        phase,
        rest_remaining_s,
        last_rep_pace_s_per_100m: None,
        rep_elapsed_s,
        swum_m: 0,
        last_rep: None,
    }
}

#[test]
fn remaining_for_foreign_position_is_zero() {
    let w = crate::model::parse_workout(r#"{"version":1,"name":"t","pool_length_m":25,"blocks":[{"name":"b","sets":[{"reps":1,"distance_m":50}]}]}"#.into()).unwrap();
    assert_eq!(
        estimated_remaining_s(
            w.clone(),
            state(pos(3, 0), Phase::Swimming, None, Some(1.0)),
            None
        ),
        0.0
    );
    // The real position still counts: 50 m at the default 120 s/100 m is 60 s, 1 s in.
    assert_eq!(
        estimated_remaining_s(w, state(pos(0, 0), Phase::Swimming, None, Some(1.0)), None),
        59.0
    );
}

#[test]
fn remaining_from_live_state() {
    // Set 0: 2 x 50 m in 40 s, 15 s fixed rest. Set 1: same with a 60 s interval.
    let w = crate::model::parse_workout(
        r#"{"version":1,"name":"t","pool_length_m":25,"blocks":[{"name":"b","sets":[
        {"reps":2,"distance_m":50,"goal_s":40,"rest":{"type":"fixed","seconds":15}},
        {"reps":2,"distance_m":50,"goal_s":40,"rest":{"type":"interval","seconds":60}}]}]}"#
            .into(),
    )
    .unwrap();
    // 10 s countdown + 55 + 55 + 60 + 40, the last rep having no rest after it.
    assert_eq!(estimated_duration_s(w.clone(), None), 240.0);
    let left = |s| estimated_remaining_s(w.clone(), s, None);
    assert_eq!(
        left(state(pos(0, 0), Phase::Resting, Some(10.0), None)),
        220.0
    );
    assert_eq!(
        left(state(pos(0, 0), Phase::Swimming, None, Some(10.0))),
        200.0
    );
    // Interval rep 50 s in: its 60 s cycle ends in 10 s.
    assert_eq!(
        left(state(pos(1, 0), Phase::Swimming, None, Some(50.0))),
        50.0
    );
    assert_eq!(
        left(state(pos(0, 1), Phase::Resting, Some(5.0), None)),
        160.0
    );
    assert_eq!(
        left(state(pos(1, 1), Phase::Swimming, None, Some(30.0))),
        10.0
    );
    assert_eq!(left(state(pos(1, 1), Phase::Done, None, None)), 0.0);
}
