use super::*;

#[test]
fn pace_rest_and_set_pace() {
    assert_eq!(reps(&session(vec![])), Vec::<RepMetrics>::new());
    // Rep 0: 4 x 30 s with 2 s turns, 0..126. Rep 1: 4 x 28 s with turns, 150..268.
    let mut parts = rep(pos(0, 0), 0.0, 30.0, 4, None);
    parts.extend(rep(pos(0, 1), 150.0, 28.0, 4, None));
    // Set 1: 2 lengths = 50 m in 300..342, so 84 s/100 m.
    parts.extend(rep(pos(1, 0), 300.0, 20.0, 2, None));
    let r = reps(&session(parts));
    assert_eq!(r.len(), 3);
    assert_eq!((r[0].lengths, r[0].distance_m), (4, 100));
    assert!((r[0].pace_s_per_100m - 126.0).abs() < 1e-9);
    assert!((r[0].rest_after_s.unwrap() - 24.0).abs() < 1e-9); // 150 - 126
    assert!((r[1].pace_s_per_100m - 118.0).abs() < 1e-9);
    assert!((r[1].rest_after_s.unwrap() - 32.0).abs() < 1e-9); // 300 - 268
    assert_eq!(r[2].rest_after_s, None);
    // Set 0: (126 + 118) s over 200 m = 122 s/100 m.
    assert!((set_pace_s_per_100m(&r, 0, 0, 0).unwrap() - 122.0).abs() < 1e-9);
    assert!((set_pace_s_per_100m(&r, 0, 0, 1).unwrap() - 84.0).abs() < 1e-9);
    assert_eq!(set_pace_s_per_100m(&r, 0, 0, 2), None);
}

#[test]
fn swolf_and_stroke_rate() {
    // SWOLF: 30 + 12 = 42, 30 + 14 = 44, no strokes, 28 + 10 = 38. Average 124 / 3.
    // Stroke rate: 36 strokes over 88 s = 2160 / 88 strokes per min.
    let parts = vec![
        (
            Lap {
                start_s: 0.0,
                end_s: 30.0,
                stroke: Stroke::Freestyle,
                strokes: Some(12),
            },
            pos(0, 0),
            1,
        ),
        (
            Lap {
                start_s: 32.0,
                end_s: 62.0,
                stroke: Stroke::Freestyle,
                strokes: Some(14),
            },
            pos(0, 0),
            1,
        ),
        (
            Lap {
                start_s: 64.0,
                end_s: 96.0,
                stroke: Stroke::Freestyle,
                strokes: None,
            },
            pos(0, 0),
            1,
        ),
        (
            Lap {
                start_s: 98.0,
                end_s: 126.0,
                stroke: Stroke::Freestyle,
                strokes: Some(10),
            },
            pos(0, 0),
            1,
        ),
    ];
    let r = &reps(&session(parts))[0];
    assert_eq!(r.swolf, vec![Some(42.0), Some(44.0), None, Some(38.0)]);
    assert!((r.swolf_avg.unwrap() - 124.0 / 3.0).abs() < 1e-9);
    assert!((r.stroke_rate_per_min.unwrap() - 2160.0 / 88.0).abs() < 1e-9);
    let none = reps(&session(rep(pos(0, 0), 0.0, 30.0, 2, None)));
    assert_eq!(
        (none[0].swolf_avg, none[0].stroke_rate_per_min),
        (None, None)
    );
}

#[test]
fn merged_lap_counts_two_lengths_but_no_swolf() {
    // Lengths 1 + 2 + 1 = 4 (100 m), 0..124 s with turns. Merged lap: 60 s, 24 strokes, no SWOLF.
    // Pace: 124 s / 100 m * 100 = 124. Stroke rate includes it: 48 strokes / 2 min = 24.
    let parts = vec![
        (
            Lap {
                start_s: 0.0,
                end_s: 30.0,
                stroke: Stroke::Freestyle,
                strokes: Some(12),
            },
            pos(0, 0),
            1,
        ),
        (
            Lap {
                start_s: 32.0,
                end_s: 92.0,
                stroke: Stroke::Freestyle,
                strokes: Some(24),
            },
            pos(0, 0),
            2,
        ),
        (
            Lap {
                start_s: 94.0,
                end_s: 124.0,
                stroke: Stroke::Freestyle,
                strokes: Some(12),
            },
            pos(0, 0),
            1,
        ),
    ];
    let r = &reps(&session(parts))[0];
    assert_eq!((r.lengths, r.distance_m), (4, 100));
    assert!((r.pace_s_per_100m - 124.0).abs() < 1e-9);
    assert_eq!(r.swolf, vec![Some(42.0), None, Some(42.0)]);
    assert!((r.swolf_avg.unwrap() - 42.0).abs() < 1e-9);
    assert!((r.stroke_rate_per_min.unwrap() - 24.0).abs() < 1e-9);
}

#[test]
fn missing_length_uses_detected_laps() {
    // 3 detected lengths of a 100 m rep in a 25 m pool: 75 m in 0..94 s with turns, so 94/75 x 100 s/100 m.
    let r = &reps(&session(rep(pos(0, 0), 0.0, 30.0, 3, None)))[0];
    assert_eq!((r.lengths, r.distance_m), (3, 75));
    assert!((r.swim_s - 94.0).abs() < 1e-9);
    assert!(f64::mul_add(94.0 / 75.0, -100.0, r.pace_s_per_100m).abs() < 1e-9);
}

#[test]
fn rep_time_includes_turns_not_rests() {
    // Two lengths of one rep with a 3 s gap at the wall: the rep takes 0..43 s, not 40 s of laps.
    let s = session(vec![
        lap(0.0, 20.0, Stroke::Freestyle, 1, 0),
        lap(23.0, 43.0, Stroke::Freestyle, 1, 0),
    ]);
    let r = &reps(&s)[0];
    assert!((r.swim_s - 43.0).abs() < 1e-9);
    assert!((r.pace_s_per_100m - 86.0).abs() < 1e-9); // 43 s over 50 m
    let sum = session_summary(&s, None);
    assert!((sum.swim_s - 43.0).abs() < 1e-9 && sum.rest_s.abs() < 1e-9);
}

#[test]
fn css_from_400_and_200() {
    // (360 - 170) / 2 = 95.
    assert!((css_s_per_100m(360.0, 170.0).unwrap() - 95.0).abs() < 1e-9);
    assert_eq!(css_s_per_100m(170.0, 170.0), None);
    assert_eq!(css_s_per_100m(170.0, 360.0), None);
    assert_eq!(css_s_per_100m(360.0, 0.0), None);
    assert_eq!(css_s_per_100m(f64::NAN, 170.0), None);
    assert_eq!(css_s_per_100m(f64::INFINITY, 170.0), None);
}

#[test]
fn css_test_times_from_bundled_workout() {
    let s = css_session(360.0, 170.0);
    assert_eq!(s.workout.name, "CSS test");
    assert_eq!(
        css_test_times(&s),
        Some(CssTest {
            t400_s: 360.0,
            t200_s: 170.0
        })
    );
    // The 200 trial one length short.
    let mut short = css_session(360.0, 170.0);
    short.laps.pop();
    short.lap_positions.pop();
    short.lap_lengths.pop();
    assert_eq!(css_test_times(&short), None);
    // Four 50 m reps of a normal workout.
    let mut normal = session(rep(pos(0, 0), 0.0, 30.0, 4, None));
    normal.workout = crate::model::parse_workout(
        include_str!("../../../../apple/Watch/Workouts/quick-test.json").into(),
    )
    .unwrap();
    assert_eq!(css_test_times(&normal), None);
}

#[test]
fn css_test_skips_aborted_trial() {
    // Block 1 aborted after 8 of 16 lengths; a full 400 retry as a duplicate block 4 after the 200.
    let mut s = css_session(360.0, 170.0);
    s.laps.drain(8..16);
    s.lap_positions.drain(8..16);
    s.lap_lengths.drain(8..16);
    s.workout.blocks.push(s.workout.blocks[1].clone());
    let t = s.laps.last().unwrap().end_s + 300.0;
    for i in 0..16 {
        s.laps.push(Lap {
            start_s: 22.5f64.mul_add(f64::from(i), t),
            end_s: 22.5f64.mul_add(f64::from(i + 1), t),
            stroke: Stroke::Freestyle,
            strokes: None,
        });
        s.lap_positions.push(Position {
            block: 4,
            round: 0,
            set: 0,
            rep: 0,
        });
        s.lap_lengths.push(1);
    }
    assert_eq!(
        css_test_times(&s),
        Some(CssTest {
            t400_s: 360.0,
            t200_s: 170.0
        })
    );
}

#[test]
fn css_test_rejects_implausible_times() {
    // CSS 5 s/100 m; and t400 not above 2 x t200.
    assert_eq!(css_test_times(&css_session(300.0, 290.0)), None);
    assert_eq!(css_test_times(&css_session(250.0, 130.0)), None);
}

#[test]
fn tss_from_css_and_elapsed() {
    // 40 x 25 m = 1000 m in 1200 s: 0.8333 m/s vs CSS 1 m/s, IF 5/6, TSS = 1/3 h x 25/36 x 100 = 2500/108.
    let s = session(rep(pos(0, 0), 0.0, 28.05, 40, None));
    assert!((swim_tss(&s, Some(100.0)).unwrap() - 2500.0 / 108.0).abs() < 1e-9);
    assert_eq!(swim_tss(&s, None), None);
    assert_eq!(swim_tss(&s, Some(0.0)), None);
    assert_eq!(swim_tss(&session(vec![]), Some(100.0)), None);
}

#[test]
fn session_summary_totals() {
    // 4 x 30 s one-length laps with 10 strokes (SWOLF 40), then a merged 2-length lap of 30 s, all one rep.
    // 150 m in 180 s (turns in): pace 120. IF = 5/6 (0.833 m/s vs CSS 1 m/s), TSS = 0.05 h x 25/36 x 100 = 125/36.
    let mut parts = rep(pos(0, 0), 0.0, 30.0, 4, Some(10));
    parts.push(lap(150.0, 180.0, Stroke::Freestyle, 2, 0));
    let s = session(parts);
    let sum = session_summary(&s, Some(100.0));
    assert_eq!(sum.distance_m, 150);
    assert!((sum.swim_s - 180.0).abs() < 1e-9);
    assert!((sum.elapsed_s - 180.0).abs() < 1e-9);
    assert!((sum.pace_s_per_100m.unwrap() - 120.0).abs() < 1e-9);
    assert!((sum.swolf_avg.unwrap() - 40.0).abs() < 1e-9);
    assert!((sum.tss.unwrap() - 125.0 / 36.0).abs() < 1e-9);
    assert_eq!(session_summary(&s, None).tss, None);
    let empty = session_summary(&session(vec![]), Some(100.0));
    assert_eq!(
        (empty.distance_m, empty.swim_s, empty.elapsed_s),
        (0, 0.0, 0.0)
    );
    assert_eq!(
        (empty.pace_s_per_100m, empty.swolf_avg, empty.tss),
        (None, None, None)
    );
}

#[test]
fn goal_delta_needs_goal_and_complete_rep() {
    // Set 0: 4 lengths (100 m), goal 110 s, swum in 126 s with turns: +16. Set 1: no goal. Set 2: goal, but 3 of 4 lengths.
    let w = |goal_s| WorkoutSet {
        reps: 1,
        distance_m: 100,
        stroke: Stroke::Any,
        intensity: Intensity::default(),
        goal_s,
        equipment: vec![],
        drill: None,
        rest: None,
        notes: None,
    };
    let sets = vec![w(Some(110)), w(None), w(Some(110))];
    let mut parts = rep(pos(0, 0), 0.0, 30.0, 4, None);
    parts.extend(rep(pos(1, 0), 200.0, 30.0, 4, None));
    parts.extend(rep(pos(2, 0), 400.0, 30.0, 3, None));
    let mut s = session(parts);
    s.workout = Workout {
        version: 1,
        name: "t".into(),
        pool_length_m: 25,
        blocks: vec![crate::model::Block {
            name: "b".into(),
            repeat: 1,
            sets,
        }],
        description: None,
    };
    let deltas: Vec<Option<f64>> = reps(&s).iter().map(|r| r.goal_delta_s).collect();
    assert_eq!(deltas, vec![Some(16.0), None, None]);
}
