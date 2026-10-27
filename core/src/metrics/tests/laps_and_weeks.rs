use super::*;

pub(super) fn hr(samples: &[(f64, f64)]) -> Vec<HrSample> {
    samples
        .iter()
        .map(|&(t_s, bpm)| HrSample { t_s, bpm })
        .collect()
}

#[test]
fn rep_heart_rate_drop_and_stroke_distance() {
    // Rep 0: laps 0..126, 10 strokes each. Rep 1: laps 150..268, no strokes.
    let mut parts = rep(pos(0, 0), 0.0, 30.0, 4, Some(10));
    parts.extend(rep(pos(0, 1), 150.0, 28.0, 4, None));
    let mut s = session(parts);
    // The sample at 140 (90 bpm) lies in the rest between the reps.
    s.hr = hr(&[
        (0.0, 100.0),
        (40.0, 120.0),
        (100.0, 160.0),
        (126.0, 130.0),
        (140.0, 90.0),
        (160.0, 110.0),
    ]);
    let r = reps(&s);
    // Rep 0 window 0..=126 holds 100, 120, 160, 130: mean 127.5, max 160.
    assert!((r[0].avg_bpm.unwrap() - 127.5).abs() < 1e-9);
    assert_eq!(r[0].max_bpm, Some(160.0));
    // Rep 1 window 150..=268 holds only 110.
    assert_eq!((r[1].avg_bpm, r[1].max_bpm), (Some(110.0), Some(110.0)));
    // 130 at the end of rep 0, 90 at the start of rep 1; the last rep has no drop.
    assert_eq!(r[0].rest_hr_drop_bpm, Some(40.0));
    assert_eq!(r[1].rest_hr_drop_bpm, None);
    // 4 x 25 m with 10 strokes each: 100 m / 40 strokes.
    assert!((r[0].distance_per_stroke_m.unwrap() - 2.5).abs() < 1e-9);
    assert_eq!(r[1].distance_per_stroke_m, None);
    s.hr.clear();
    let r = reps(&s);
    assert_eq!(
        (r[0].avg_bpm, r[0].max_bpm, r[0].rest_hr_drop_bpm),
        (None, None, None)
    );
}

#[test]
fn lap_metrics_normal_and_merged() {
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
                stroke: Stroke::Backstroke,
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
                strokes: None,
            },
            pos(0, 0),
            1,
        ),
    ];
    let mut s = session(parts);
    s.hr = hr(&[(20.0, 120.0), (50.0, 140.0), (100.0, 150.0)]);
    let l = laps(&s);
    // Normal lap: 25 m in 30 s, 12 strokes: SWOLF 42, 24 strokes/min, 25/12 m per stroke.
    assert_eq!((l[0].index, l[0].lengths, l[0].distance_m), (0, 1, 25));
    assert!((l[0].pace_s_per_100m - 120.0).abs() < 1e-9);
    assert_eq!(l[0].swolf, Some(42.0));
    assert!((l[0].stroke_rate_per_min.unwrap() - 24.0).abs() < 1e-9);
    assert!((l[0].distance_per_stroke_m.unwrap() - 25.0 / 12.0).abs() < 1e-9);
    assert_eq!(l[0].avg_bpm, Some(120.0));
    // Merged lap: 50 m in 60 s, 24 strokes: no SWOLF, 24 strokes/min, 50/24 m per stroke.
    assert_eq!(
        (l[1].lengths, l[1].distance_m, l[1].start_s, l[1].swim_s),
        (2, 50, 32.0, 60.0)
    );
    assert_eq!(l[1].stroke, Stroke::Backstroke);
    assert!((l[1].pace_s_per_100m - 120.0).abs() < 1e-9);
    assert_eq!(l[1].swolf, None);
    assert!((l[1].stroke_rate_per_min.unwrap() - 24.0).abs() < 1e-9);
    assert!((l[1].distance_per_stroke_m.unwrap() - 50.0 / 24.0).abs() < 1e-9);
    assert_eq!(l[1].avg_bpm, Some(140.0));
    assert_eq!(
        (
            l[2].swolf,
            l[2].stroke_rate_per_min,
            l[2].distance_per_stroke_m
        ),
        (None, None, None)
    );
    assert_eq!((l[2].strokes, l[2].avg_bpm), (None, Some(150.0)));
}

#[test]
fn set_metrics_drift_rest_and_goals() {
    // Set 0: 3 reps of 4 x 25 m, goal 110 s. Rep times 106, 114, 130 s with turns; rests 24 and 36 s between them.
    // Set 1: one rep of 126 s with turns, no goal.
    let w = |reps, goal_s| WorkoutSet {
        reps,
        distance_m: 100,
        stroke: Stroke::Any,
        intensity: Intensity::default(),
        goal_s,
        equipment: vec![],
        drill: None,
        rest: None,
        notes: None,
    };
    let wsets = vec![w(3, Some(110)), w(1, None)];
    let mut parts = rep(pos(0, 0), 0.0, 25.0, 4, None); // 0..106
    parts.extend(rep(pos(0, 1), 130.0, 27.0, 4, None)); // 130..244
    parts.extend(rep(pos(0, 2), 280.0, 31.0, 4, None)); // 280..410
    parts.extend(rep(pos(1, 0), 450.0, 30.0, 4, None)); // 450..526
    let mut s = session(parts);
    s.workout = Workout {
        version: 1,
        name: "t".into(),
        pool_length_m: 25,
        blocks: vec![crate::model::Block {
            name: "b".into(),
            repeat: 1,
            sets: wsets,
        }],
        description: None,
    };
    // 120 bpm sits in the rest after rep 0 and 999 bpm too; 420 is after set 0, 500 is in set 1.
    s.hr = hr(&[
        (10.0, 120.0),
        (120.0, 999.0),
        (200.0, 170.0),
        (300.0, 150.0),
        (420.0, 90.0),
        (500.0, 100.0),
    ]);
    let m = sets(&s);
    assert_eq!(m.len(), 2);
    let (a, b) = (&m[0], &m[1]);
    assert_eq!(
        (a.block, a.round, a.set, a.reps, a.distance_m),
        (0, 0, 0, 3, 300)
    );
    assert!((a.swim_s - 350.0).abs() < 1e-9);
    assert!((a.rest_s - 60.0).abs() < 1e-9); // 24 + 36, not the 40 s after the set
    assert!(f64::mul_add(350.0 / 300.0, -100.0, a.pace_s_per_100m).abs() < 1e-9);
    assert_eq!(
        (a.fastest_pace_s_per_100m, a.slowest_pace_s_per_100m),
        (106.0, 130.0)
    );
    assert!((a.pace_drift_s_per_100m - 24.0).abs() < 1e-9);
    // Samples inside the rep windows: 120, 170, 150.
    assert!((a.avg_bpm.unwrap() - 440.0 / 3.0).abs() < 1e-9);
    assert_eq!(a.max_bpm, Some(170.0));
    // Goal 110: only 106 hits.
    assert_eq!((a.goal_reps, a.goal_hits), (3, 1));
    assert_eq!((b.reps, b.rest_s, b.pace_drift_s_per_100m), (1, 0.0, 0.0));
    assert_eq!(
        (b.fastest_pace_s_per_100m, b.slowest_pace_s_per_100m),
        (126.0, 126.0)
    );
    assert_eq!((b.avg_bpm, b.max_bpm), (Some(100.0), Some(100.0)));
    assert_eq!((b.goal_reps, b.goal_hits), (0, 0));
}

#[test]
fn summary_zones_and_strokes() {
    // Freestyle: 2 x 25 m, 30 s each, 10 strokes each. Backstroke: 4 x 25 m, 28 s each, 12 strokes each.
    // Turns and the rest between the reps are 2 s each.
    let mut parts = rep(pos(0, 0), 0.0, 30.0, 2, Some(10));
    parts.extend(
        rep(pos(0, 1), 64.0, 28.0, 4, Some(12))
            .into_iter()
            .map(|(l, p, n)| {
                (
                    Lap {
                        stroke: Stroke::Backstroke,
                        ..l
                    },
                    p,
                    n,
                )
            }),
    );
    let mut s = session(parts);
    s.hr = hr(&[(0.0, 100.0), (10.0, 150.0), (50.0, 170.0)]);
    s.age_years = Some(40); // max HR 180
    let sum = session_summary(&s, None);
    assert_eq!((sum.distance_m, sum.laps), (150, 6));
    assert!((sum.swim_s - 180.0).abs() < 1e-9); // reps: 62 + 118
    assert!((sum.rest_s - 2.0).abs() < 1e-9);
    assert_eq!(sum.strokes, Some(68));
    assert!(f64::mul_add(68.0 / 172.0, -60.0, sum.stroke_rate_per_min.unwrap()).abs() < 1e-9);
    assert!((sum.distance_per_stroke_m.unwrap() - 150.0 / 68.0).abs() < 1e-9);
    assert!((sum.avg_bpm.unwrap() - 140.0).abs() < 1e-9);
    assert_eq!(sum.max_bpm, Some(170.0));
    // 100 bpm (56 % of max) is zone 1, 150 bpm (83 %) is zone 4. The 40 s gap after 150 counts 30 s; the last sample 0.
    assert_eq!(sum.zone_s, vec![10.0, 0.0, 0.0, 30.0, 0.0]);
    let by = &sum.by_stroke;
    assert_eq!(by.len(), 2);
    assert_eq!(
        (by[0].stroke, by[0].lengths, by[0].distance_m),
        (Stroke::Backstroke, 4, 100)
    );
    assert!((by[0].swim_s - 112.0).abs() < 1e-9);
    assert!((by[0].pace_s_per_100m - 112.0).abs() < 1e-9);
    assert_eq!(
        (by[1].stroke, by[1].lengths, by[1].distance_m),
        (Stroke::Freestyle, 2, 50)
    );
    assert!((by[1].pace_s_per_100m - 120.0).abs() < 1e-9);
    // A merged lap counts as two lengths.
    let merged = session(vec![(
        Lap {
            start_s: 0.0,
            end_s: 60.0,
            stroke: Stroke::Freestyle,
            strokes: None,
        },
        pos(0, 0),
        2,
    )]);
    assert_eq!(session_summary(&merged, None).by_stroke[0].lengths, 2);
    s.age_years = None;
    assert_eq!(session_summary(&s, None).zone_s, [] as [f64; 0]);
}

pub(super) fn week_session(started_at_ms: i64, parts: Vec<(Lap, Position, u32)>) -> StoredSession {
    let mut session = session(parts);
    session.started_at_ms = started_at_ms;
    StoredSession {
        id: started_at_ms.to_string(),
        created_at_ms: 0,
        updated_at_ms: 0,
        session,
    }
}

#[test]
fn weekly_totals_sum_and_sort() {
    const DAY: i64 = 86_400_000;
    let mon = 7_004 * DAY; // a Monday (days since epoch % 7 == 4 maps to Monday)
    let wed = mon + 2 * DAY + 10 * 3_600_000;
    let a = week_session(wed, rep(pos(0, 0), 0.0, 30.0, 2, None)); // 50 m, 62 s
    let b = week_session(wed + 3_600_000, rep(pos(0, 0), 0.0, 30.0, 4, None)); // 100 m, 126 s
    let next = week_session(mon + 8 * DAY, rep(pos(0, 0), 0.0, 30.0, 1, None)); // 25 m, 30 s
    assert_eq!(
        weekly_totals(vec![next, a, b], 0),
        vec![
            WeekTotal {
                week_start_ms: mon,
                distance_m: 150,
                swim_s: 188.0,
                sessions: 2
            },
            WeekTotal {
                week_start_ms: mon + 7 * DAY,
                distance_m: 25,
                swim_s: 30.0,
                sessions: 1
            },
        ]
    );
    assert_eq!(weekly_totals(vec![], 0), Vec::<WeekTotal>::new());
}

#[test]
fn weekly_totals_follow_local_time() {
    const DAY: i64 = 86_400_000;
    const H: i64 = 3_600_000;
    let mon = 7_004 * DAY;
    // UTC Sunday 21:30 and 22:30 are local Sunday 23:30 and Monday 00:30 at +2 h.
    let sun = week_session(
        mon + 6 * DAY + 21 * H + H / 2,
        rep(pos(0, 0), 0.0, 30.0, 2, None),
    );
    let mon_early = week_session(
        mon + 6 * DAY + 22 * H + H / 2,
        rep(pos(0, 0), 0.0, 30.0, 2, None),
    );
    let utc = weekly_totals(vec![sun.clone(), mon_early.clone()], 0);
    assert_eq!(utc.iter().map(|w| w.sessions).collect::<Vec<_>>(), vec![2]);
    // Local Monday 00:00 is UTC mon - 2 h.
    let local = weekly_totals(vec![sun, mon_early], 2 * 3600);
    assert_eq!(
        local
            .iter()
            .map(|w| (w.week_start_ms, w.sessions))
            .collect::<Vec<_>>(),
        vec![(mon - 2 * H, 1), (mon - 2 * H + 7 * DAY, 1)]
    );
}
