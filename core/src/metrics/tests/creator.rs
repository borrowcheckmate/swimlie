use super::*;

#[test]
fn profile_from_recent_sessions() {
    const DAY_MS: i64 = 86_400_000;
    const NOW: i64 = 1_000 * DAY_MS;
    // Recent: three 50 m freestyle reps swum in 62, 58 and 78 s; age 41; HR up to 182.
    let mut parts = rep(pos(0, 0), 0.0, 30.0, 2, None);
    parts.extend(rep(pos(0, 1), 100.0, 28.0, 2, None));
    parts.extend(rep(pos(0, 2), 200.0, 38.0, 2, None));
    let mut recent = stored("recent", parts);
    recent.session.started_at_ms = NOW - 10 * DAY_MS;
    recent.session.age_years = Some(41);
    recent.session.hr = hr(&[(10.0, 150.0), (120.0, 182.0)]);
    // 100 days ago: one more 50 m rep, plus an older age and HR that must be ignored.
    let mut old = stored("old", rep(pos(0, 0), 0.0, 20.0, 2, None));
    old.session.started_at_ms = NOW - 100 * DAY_MS;
    old.session.age_years = Some(40);
    old.session.hr = hr(&[(10.0, 190.0)]);
    let p = swimmer_profile(vec![old, recent], Some(95.0), NOW, SwimLevel::default());
    assert_eq!(p.css_s_per_100m, Some(95.0));
    assert_eq!(p.sessions_28d, 1);
    assert!((p.weekly_m_28d - 150.0 / 4.0).abs() < 1e-9);
    assert_eq!(
        p.rep_times,
        vec![StrokeTime {
            stroke: Stroke::Freestyle,
            distance_m: 50,
            median_s: 62.0,
            count: 3
        }]
    );
    assert_eq!(p.age_years, Some(41));
    assert_eq!(p.max_hr_bpm, Some(182.0));
}

#[test]
fn profile_without_sessions_is_empty() {
    assert_eq!(
        swimmer_profile(vec![], None, 0, SwimLevel::default()),
        SwimmerProfile {
            css_s_per_100m: None,
            sessions_28d: 0,
            weekly_m_28d: 0.0,
            rep_times: vec![],
            age_years: None,
            max_hr_bpm: None,
            level: SwimLevel::default()
        }
    );
}

#[test]
fn creator_brief_with_history() {
    let inputs = CreatorInputs {
        language: "en".into(),
        pool_length_m: 25,
        equipment: vec![Equipment::Paddles, Equipment::PullBuoy],
        starting_blocks: true,
        target_distance_m: Some(2000),
        target_time_min: None,
        kind: "threshold".into(),
        notes: "Hold 1:40 pace".into(),
        plan_weeks: None,
        swims_per_week: None,
        rest_percent: 0,
        strokes: vec![],
    };
    let profile = SwimmerProfile {
        css_s_per_100m: Some(95.0),
        sessions_28d: 3,
        weekly_m_28d: 2400.0,
        rep_times: vec![
            StrokeTime {
                stroke: Stroke::Freestyle,
                distance_m: 50,
                median_s: 45.0,
                count: 12,
            },
            StrokeTime {
                stroke: Stroke::Freestyle,
                distance_m: 100,
                median_s: 98.0,
                count: 6,
            },
        ],
        age_years: Some(41),
        max_hr_bpm: Some(182.0),
        level: SwimLevel::default(),
    };
    let brief = creator_brief(inputs, profile);
    for line in [
        "My request:",
        "- Pool: 25 m",
        "- Type: threshold",
        "- Target: 2000 m",
        "- Equipment: paddles, pull buoy",
        "- Only use this equipment; none means no equipment at all.",
        "- Starting blocks: yes, dives allowed",
        "- Notes: Hold 1:40 pace",
        "- CSS pace: 1:35 /100 m",
        "- Last 4 weeks: 3 swims, about 2400 m per week",
        "- Typical rep times: freestyle 50 m 0:45 (12 reps), freestyle 100 m 1:38 (6 reps)",
        "- Age 41, max heart rate seen 182 bpm",
    ] {
        assert!(
            brief.lines().any(|l| l == line),
            "missing {line:?} in:\n{brief}"
        );
    }
    assert!(!brief.contains("Target time") && !brief.contains("No swim history"));
}

#[test]
fn creator_brief_plan_lines() {
    let inputs = CreatorInputs {
        language: "en".into(),
        pool_length_m: 25,
        equipment: vec![],
        starting_blocks: false,
        target_distance_m: Some(1000),
        target_time_min: Some(30),
        kind: "base".into(),
        notes: String::new(),
        plan_weeks: Some(4),
        swims_per_week: None,
        rest_percent: 0,
        strokes: vec![],
    };
    let empty = SwimmerProfile {
        css_s_per_100m: None,
        sessions_28d: 0,
        weekly_m_28d: 0.0,
        rep_times: vec![],
        age_years: None,
        max_hr_bpm: None,
        level: SwimLevel::default(),
    };
    let brief = creator_brief(inputs, empty);
    let lines: Vec<&str> = brief.lines().collect();
    assert_eq!(lines[0], "My request:");
    assert_eq!(
        lines[1],
        "- Plan: 4 weeks, 3 swims per week. Reply with a plan (version, name, weeks; each week has a focus and its workouts), not a single workout."
    );
    assert!(
        lines.contains(&"- Target per swim: 1000 m")
            && lines.contains(&"- Target time per swim: 30 min")
    );
    assert!(!brief.contains("- Target: ") && !brief.contains("- Target time: "));
}

#[test]
fn creator_brief_without_history_or_targets() {
    let inputs = CreatorInputs {
        language: "en".into(),
        pool_length_m: 50,
        equipment: vec![],
        starting_blocks: false,
        target_distance_m: None,
        target_time_min: None,
        kind: "easy recovery".into(),
        notes: "  ".into(),
        plan_weeks: None,
        swims_per_week: None,
        rest_percent: 0,
        strokes: vec![],
    };
    let empty = SwimmerProfile {
        css_s_per_100m: None,
        sessions_28d: 0,
        weekly_m_28d: 0.0,
        rep_times: vec![],
        age_years: None,
        max_hr_bpm: None,
        level: SwimLevel::default(),
    };
    let brief = creator_brief(inputs, empty);
    assert!(brief.contains("- No swim history yet: base everything on my level."));
    assert!(brief.lines().any(|l| l == "- Equipment: none"));
    assert!(brief.lines().any(|l| l == "- Starting blocks: no dives"));
    assert!(
        brief
            .lines()
            .any(|l| l.starts_with("- Every set needs a rest"))
    );
    for absent in [
        "Target",
        "Notes",
        "CSS pace",
        "Last 4 weeks",
        "Typical rep",
        "Age ",
    ] {
        assert!(
            !brief.contains(absent),
            "unexpected {absent:?} in:\n{brief}"
        );
    }
}

pub(super) fn free_lap(start_s: f64, end_s: f64) -> Lap {
    Lap {
        start_s,
        end_s,
        stroke: Stroke::Freestyle,
        strokes: None,
    }
}

#[test]
fn free_sets_split_at_rest_gaps() {
    // 20 s laps, 4 s turns, one 30 s rest, then 2 laps.
    let laps = vec![
        free_lap(0.0, 20.0),
        free_lap(24.0, 44.0),
        free_lap(48.0, 68.0),
        free_lap(72.0, 92.0),
        free_lap(122.0, 142.0),
        free_lap(146.0, 166.0),
    ];
    let sets = free_sets(laps.clone(), 0.0);
    assert_eq!(sets.len(), 2);
    assert_eq!(
        sets[0],
        FreeSet {
            start_s: 0.0,
            end_s: 92.0,
            lengths: 4,
            done: true
        }
    );
    assert_eq!(
        sets[1],
        FreeSet {
            start_s: 122.0,
            end_s: 166.0,
            lengths: 2,
            done: false
        }
    );
    // Overdue at last end + median 20 + 9 + 20 = 215 s; done only after it.
    assert!(!free_sets(laps.clone(), 215.0)[1].done);
    assert!(free_sets(laps, 216.0)[1].done);
}

#[test]
fn free_sets_empty() {
    assert_eq!(free_sets(vec![], 100.0), Vec::<FreeSet>::new());
}
