use super::*;

#[test]
fn rep_result_is_faster_or_slower_than_goal() {
    let mut e = engine(r#"{"reps": 2, "distance_m": 50, "goal_s": 45}"#);
    assert_eq!(e.tick(0.0).last_rep, None);
    let end = swim(&mut e, 0.0, 2, 20.0);
    let pos = Position {
        block: 0,
        round: 0,
        set: 0,
        rep: 0,
    };
    assert_eq!(
        e.tick(end + 5.0).last_rep,
        Some(RepResult {
            pos,
            swim_s: 42.0,
            goal_diff_s: Some(-3.0),
            verdict: Some(Verdict::Faster),
            avg_bpm: None
        })
    );
    let mut e = engine(r#"{"reps": 2, "distance_m": 50, "goal_s": 40}"#);
    let end = swim(&mut e, 0.0, 2, 20.0);
    let s = e.tick(end + 5.0).last_rep.unwrap();
    assert_eq!(
        (s.goal_diff_s, s.verdict),
        (Some(2.0), Some(Verdict::Slower))
    );
}

#[test]
fn rep_avg_bpm_counts_only_samples_inside_the_rep() {
    let mut e = engine(r#"{"reps": 2, "distance_m": 50}"#);
    // The rep spans 0..42 s; the sample at 42 s is the rep's last second, inclusive.
    for (t, bpm) in [
        (-1.0, 90.0),
        (0.0, 150.0),
        (20.0, 160.0),
        (42.0, 170.0),
        (43.0, 200.0),
    ] {
        e.on_heart_rate(t, bpm);
    }
    e.on_heart_rate(10.0, f64::NAN);
    let end = swim(&mut e, 0.0, 2, 20.0);
    assert_eq!(e.tick(end + 5.0).last_rep.unwrap().avg_bpm, Some(160.0));
    // No samples in the span: no average.
    let mut e = engine(r#"{"reps": 1, "distance_m": 50}"#);
    e.on_heart_rate(100.0, 150.0);
    let end = swim(&mut e, 0.0, 2, 20.0);
    assert_eq!(e.tick(end + 5.0).last_rep.unwrap().avg_bpm, None);
}

#[test]
fn out_of_order_heart_rate_is_dropped() {
    let mut e = engine(r#"{"reps": 1, "distance_m": 50}"#);
    e.on_heart_rate(10.0, 120.0);
    e.on_heart_rate(5.0, 130.0);
    e.on_heart_rate(20.0, 0.0);
    e.on_heart_rate(20.0, f64::NAN);
    swim(&mut e, 0.0, 2, 20.0);
    let s = e.session();
    assert_eq!(
        s.hr,
        vec![HrSample {
            t_s: 10.0,
            bpm: 120.0
        }]
    );
    assert!(
        crate::store::Store::open(":memory:")
            .unwrap()
            .save_session(None, &s)
            .is_ok()
    );
}

#[test]
fn rep_result_fastest_beats_a_passed_in_record() {
    let record = PersonalRecord {
        distance_m: 50,
        stroke: Stroke::Freestyle,
        equipment: vec![],
        time_s: 43.0,
        session_id: "old".into(),
        pos: Position {
            block: 0,
            round: 0,
            set: 0,
            rep: 0,
        },
    };
    let mut e = engine_with(
        r#"{"reps": 1, "distance_m": 50, "stroke": "freestyle"}"#,
        vec![record],
    );
    let end = swim(&mut e, 0.0, 2, 20.0);
    assert_eq!(
        e.tick(end + 5.0).last_rep.unwrap().verdict,
        Some(Verdict::Fastest)
    );
}

#[test]
fn second_rep_faster_than_first_is_fastest_without_records() {
    let mut e = engine(r#"{"reps": 2, "distance_m": 50}"#);
    let end = swim(&mut e, 0.0, 2, 20.0);
    // The first clean time of a distance is no record.
    assert_eq!(e.tick(end + 5.0).last_rep.unwrap().verdict, None);
    let end = swim(&mut e, end + 2.0, 2, 18.0);
    assert_eq!(
        e.tick(end + 5.0).last_rep.unwrap().verdict,
        Some(Verdict::Fastest)
    );
}

#[test]
fn rep_without_gear_is_not_fastest_against_a_paddles_record() {
    let record = PersonalRecord {
        distance_m: 50,
        stroke: Stroke::Freestyle,
        equipment: vec![Equipment::Paddles],
        time_s: 43.0,
        session_id: "old".into(),
        pos: Position {
            block: 0,
            round: 0,
            set: 0,
            rep: 0,
        },
    };
    // 40 s without gear would beat the paddles record, but it is a different record.
    let mut e = engine_with(
        r#"{"reps": 1, "distance_m": 50, "stroke": "freestyle"}"#,
        vec![record.clone()],
    );
    let end = swim(&mut e, 0.0, 2, 20.0);
    assert_eq!(e.tick(end + 5.0).last_rep.unwrap().verdict, None);
    // The same time with paddles is the record to beat.
    let mut e = engine_with(
        r#"{"reps": 1, "distance_m": 50, "stroke": "freestyle", "equipment": ["paddles"]}"#,
        vec![record],
    );
    let end = swim(&mut e, 0.0, 2, 20.0);
    assert_eq!(
        e.tick(end + 5.0).last_rep.unwrap().verdict,
        Some(Verdict::Fastest)
    );
}

#[test]
fn exercise_rep_is_never_fastest() {
    let record = PersonalRecord {
        distance_m: 50,
        stroke: Stroke::Freestyle,
        equipment: vec![],
        time_s: 43.0,
        session_id: "old".into(),
        pos: Position {
            block: 0,
            round: 0,
            set: 0,
            rep: 0,
        },
    };
    // 40 s for a 50 would beat the record, but a drill rep is not a time trial.
    let mut e = engine_with(
        r#"{"reps": 1, "distance_m": 50, "stroke": "freestyle", "drill": "catch-up"}"#,
        vec![record],
    );
    let end = swim(&mut e, 0.0, 2, 20.0);
    assert_eq!(e.tick(end + 5.0).last_rep.unwrap().verdict, None);
}

#[test]
fn merged_rep_is_never_fastest() {
    let record = |distance_m| PersonalRecord {
        distance_m,
        stroke: Stroke::Freestyle,
        equipment: vec![],
        time_s: 200.0,
        session_id: "old".into(),
        pos: Position {
            block: 0,
            round: 0,
            set: 0,
            rep: 0,
        },
    };
    // 84 s for 100 m would beat the record, but the merged lap makes the distance a guess.
    let mut e = engine_with(r#"{"reps": 1, "distance_m": 100}"#, vec![record(100)]);
    swim(&mut e, 0.0, 2, 20.0);
    e.on_lap(
        Lap {
            start_s: 44.0,
            end_s: 84.0,
            stroke: Stroke::Freestyle,
            strokes: None,
        },
        89.0,
    );
    assert_eq!(e.tick(90.0).last_rep.unwrap().verdict, None);
}

#[test]
fn isolated_slow_lap_is_merged_but_a_run_of_slow_laps_is_not() {
    let slow = |e: &mut Engine, t: f64| {
        e.on_lap(
            Lap {
                start_s: t,
                end_s: t + 48.0,
                stroke: Stroke::Kickboard,
                strokes: None,
            },
            t + 53.0,
        )
    };
    let mut e = engine(r#"{"reps": 8, "distance_m": 25}, {"reps": 1, "distance_m": 100}"#);
    let end = swim(&mut e, 0.0, 8, 20.0);
    slow(&mut e, end + 2.0);
    assert_eq!(e.session().lap_lengths.last(), Some(&2));
    // The next lap is slow too: a slow stroke, one length each. The rep is now 2 of 4.
    let s = slow(&mut e, end + 52.0);
    assert_eq!(&e.session().lap_lengths[8..], &[1, 1]);
    assert_eq!(s.done_m, 50);
}

#[test]
fn lap_faster_than_a_human_is_ignored() {
    let mut e = engine(r#"{"reps": 1, "distance_m": 50}"#);
    let end = swim(&mut e, 0.0, 1, 20.0);
    e.on_lap(
        Lap {
            start_s: end + 30.0,
            end_s: end + 33.5,
            stroke: Stroke::Freestyle,
            strokes: None,
        },
        end + 100.0,
    );
    assert_eq!(e.session().laps.len(), 1);
    assert_eq!(e.tick(end + 100.0).done_m, 25);
}
