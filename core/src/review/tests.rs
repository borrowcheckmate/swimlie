use super::*;
use crate::metrics::SwimLevel;
use crate::model::{Block, Lap, Position, Rest, Workout};

fn set(distance_m: u32, rest: Option<Rest>) -> WorkoutSet {
    WorkoutSet {
        reps: 2,
        distance_m,
        stroke: Stroke::Freestyle,
        intensity: crate::model::Intensity::default(),
        rest,
        goal_s: None,
        equipment: vec![],
        drill: None,
        notes: None,
    }
}

/// Two sets of two 50 m reps (25 m pool, two lengths each). A length is 28 s with 2 s turns, so a rep swims 58 s
/// (1:56 /100 m). The rest after a rep is 20 s, except after the last rep.
fn stored(started_at_ms: i64) -> StoredSession {
    let workout = Workout {
        version: 1,
        name: "Main set".into(),
        pool_length_m: 25,
        blocks: vec![Block {
            name: "Main".into(),
            repeat: 1,
            sets: vec![
                set(
                    50,
                    Some(Rest {
                        kind: RestKind::Fixed,
                        seconds: 20,
                    }),
                ),
                set(50, None),
            ],
        }],
        description: None,
    };
    let (mut laps, mut positions, mut lengths) = (vec![], vec![], vec![]);
    let mut t = 0.0;
    for rep in 0..4u32 {
        let (set_idx, rep_idx) = (rep / 2, rep % 2);
        for _ in 0..2 {
            laps.push(Lap {
                start_s: t,
                end_s: t + 28.0,
                stroke: Stroke::Freestyle,
                strokes: None,
            });
            positions.push(Position {
                block: 0,
                round: 0,
                set: set_idx,
                rep: rep_idx,
            });
            lengths.push(1);
            t += 30.0; // 28 s length plus 2 s turn
        }
        if rep < 3 {
            t += 18.0; // back from the turn after the last length, then 20 s rest
        }
    }
    let session = Session {
        workout,
        started_at_ms,
        workout_id: None,
        laps,
        lap_positions: positions,
        lap_lengths: lengths,
        hr: vec![],
        age_years: None,
        location: None,
    };
    StoredSession {
        id: started_at_ms.to_string(),
        created_at_ms: 0,
        updated_at_ms: 0,
        session,
    }
}

/// One freestyle rep of `lengths` 25 m lengths, each `secs` long, in a fixed one-rep set.
fn swim(started_at_ms: i64, lengths: u32, secs: f64) -> StoredSession {
    let workout = Workout {
        version: 1,
        name: "Rep".into(),
        pool_length_m: 25,
        blocks: vec![Block {
            name: "Main".into(),
            repeat: 1,
            sets: vec![WorkoutSet {
                reps: 1,
                ..set(lengths * 25, None)
            }],
        }],
        description: None,
    };
    let (mut laps, mut positions, mut lens) = (vec![], vec![], vec![]);
    for i in 0..lengths {
        let t = f64::from(i) * secs;
        laps.push(Lap {
            start_s: t,
            end_s: t + secs,
            stroke: Stroke::Freestyle,
            strokes: None,
        });
        positions.push(Position {
            block: 0,
            round: 0,
            set: 0,
            rep: 0,
        });
        lens.push(1);
    }
    let session = Session {
        workout,
        started_at_ms,
        workout_id: None,
        laps,
        lap_positions: positions,
        lap_lengths: lens,
        hr: vec![],
        age_years: None,
        location: None,
    };
    StoredSession {
        id: started_at_ms.to_string(),
        created_at_ms: 0,
        updated_at_ms: 0,
        session,
    }
}

fn profile(css: Option<f64>) -> SwimmerProfile {
    SwimmerProfile {
        css_s_per_100m: css,
        sessions_28d: 0,
        weekly_m_28d: 0.0,
        rep_times: vec![],
        age_years: None,
        max_hr_bpm: None,
        level: SwimLevel::default(),
    }
}

#[test]
fn dates_are_utc_civil() {
    assert_eq!(date_utc(0), "1970-01-01");
    assert_eq!(date_utc(951_782_400_000), "2000-02-29");
    assert_eq!(date_utc(951_782_400_000 + 86_400_000), "2000-03-01");
}

#[test]
fn brief_is_compact_and_complete() {
    const DAY: i64 = 86_400_000;
    let now = 951_782_400_000 + DAY;
    let previous = vec![stored(now - 2 * DAY), stored(now - 4 * DAY)];
    let brief = session_review_brief(stored(now), profile(Some(120.0)), previous);
    assert!(brief.starts_with("Units: distances in metres"), "{brief}");
    assert!(brief.contains("Date: 2000-03-01, pool 25 m"), "{brief}");
    assert!(
        brief.contains("Planned workout: Main set, 200 m planned"),
        "{brief}"
    );
    assert!(
        brief.contains("Total: 200 m in 4:52 elapsed (3:52 swimming, 1:00 resting)"),
        "{brief}"
    );
    assert!(brief.contains("- Set 1: planned 2 x 50 m freestyle (easy, rest 20 s); done 2 x 50 m, pace 1:56 /100 m, rest avg 20 s"), "{brief}");
    assert!(brief.contains("- Set 2: planned 2 x 50 m freestyle (easy, no rest); done 2 x 50 m, pace 1:56 /100 m, rest avg 20 s"), "{brief}");
    assert!(!brief.contains("(cut short"), "{brief}");
    assert!(
        brief.contains("CSS 2:00 /100 m: average pace 4 s/100 m faster than CSS"),
        "{brief}"
    );
    assert!(
        brief.contains("Progress:\n- Previous 2 swims: average 200 m, average pace 1:56 /100 m"),
        "{brief}"
    );
    assert!(
        brief.contains("- Pace vs that average: same as that average"),
        "{brief}"
    );
    assert!(
        brief.contains("- Distance vs that average: +0 m"),
        "{brief}"
    );
    assert!(
        brief.contains("- Same stroke (freestyle) vs last 2 such swims: same as that average"),
        "{brief}"
    );
    assert!(
        brief.contains("- Swims in the last 7 days: 3, last 28 days: 3"),
        "{brief}"
    );
    assert!(brief.contains("Achievements:"), "{brief}");
    assert!(brief.len() < 4000, "{brief}");
}

#[test]
fn interval_and_cut_short_sets_say_what_happened() {
    const DAY: i64 = 86_400_000;
    let mut s = stored(951_782_400_000 + DAY);
    // Set 1: a send-off every 90 s, planned 4 reps but only 2 done.
    s.session.workout.blocks[0].sets[0].reps = 4;
    s.session.workout.blocks[0].sets[0].rest = Some(Rest {
        kind: RestKind::Interval,
        seconds: 90,
    });
    let brief = session_review_brief(s, profile(None), vec![]);
    assert!(brief.contains("- Set 1: planned 4 x 50 m freestyle (easy, send-off every 1:30, implied rest about 32 s); done 2 x 50 m (cut short: 2 of 4 reps), pace 1:56 /100 m"), "{brief}");
    assert!(!brief.contains("rest 90 s"), "{brief}");
}

#[test]
fn brief_without_history_says_so() {
    let brief = session_review_brief(stored(0), profile(None), vec![]);
    assert!(brief.contains("Previous swims: none to compare"), "{brief}");
    assert!(!brief.contains("Progress:"), "{brief}");
    assert!(!brief.contains("CSS"), "{brief}");
    assert!(brief.contains("Achievements:"), "{brief}");
}

#[test]
fn personal_best_only_when_true() {
    const DAY: i64 = 86_400_000;
    let now = 951_782_400_000 + DAY;
    // Longer and faster than the one earlier swim.
    let brief = session_review_brief(
        swim(now, 8, 25.0),
        profile(None),
        vec![swim(now - DAY, 4, 30.0)],
    );
    assert!(
        brief.contains("- Personal best: longest swim so far, 200 m"),
        "{brief}"
    );
    assert!(
        brief.contains("- Personal best: fastest average pace so far, 1:40 /100 m"),
        "{brief}"
    );
    // Shorter and slower: no claim.
    let brief = session_review_brief(
        swim(now, 4, 30.0),
        profile(None),
        vec![swim(now - DAY, 8, 25.0)],
    );
    assert!(!brief.contains("Personal best"), "{brief}");
    // No earlier swims: nothing to beat.
    let brief = session_review_brief(swim(now, 8, 25.0), profile(None), vec![]);
    assert!(!brief.contains("Personal best"), "{brief}");
}

#[test]
fn achievements_list_only_true_facts() {
    const DAY: i64 = 86_400_000;
    let now = 951_782_400_000 + DAY;
    // Same plan, 25 s lengths against 30 s lengths: faster sets and a fastest-pace best, nothing else.
    let brief = session_review_brief(
        swim(now, 8, 25.0),
        profile(None),
        vec![swim(now - DAY, 8, 30.0)],
    );
    let achievements = brief.split("Achievements:\n").nth(1).unwrap_or("");
    assert!(
        achievements.starts_with("- 1 of 1 sets faster than your earlier average for that stroke"),
        "{brief}"
    );
    assert!(
        achievements.contains("- Personal best: fastest average pace so far, 1:40 /100 m"),
        "{brief}"
    );
    assert!(!achievements.contains("Negative split"), "{brief}");
    assert!(!achievements.contains("above the plan"), "{brief}");
    assert!(!achievements.contains("longest"), "{brief}");
    // Identical swims: nothing to claim.
    let brief = session_review_brief(
        swim(now, 8, 30.0),
        profile(None),
        vec![swim(now - DAY, 8, 30.0)],
    );
    assert!(
        brief.contains("Achievements:\n- None in these numbers"),
        "{brief}"
    );
}
