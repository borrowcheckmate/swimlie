use super::*;

#[test]
fn shares_fill_the_main_block() {
    let kinds = [
        SwimType::EasyRecovery,
        SwimType::Endurance,
        SwimType::Threshold,
        SwimType::Speed,
        SwimType::Technique,
        SwimType::Mixed,
    ];
    let total = |b: &Block| f64::from(b.sets.iter().map(|s| s.reps * s.distance_m).sum::<u32>());
    // Drill and skill sets (PLAN.md 4b) are not swimming warm-up, so they are left out of the warm-up share.
    let swim_warm = |b: &Block| {
        f64::from(
            b.sets
                .iter()
                .filter(|s| s.drill.is_none())
                .map(|s| s.reps * s.distance_m)
                .sum::<u32>(),
        )
    };
    // Exception the rules cannot meet: a 400 m rep leaves under two lengths each for warm-up and cool-down
    // (endurance, 50 m pool, 500 m).
    let exempt = |kind: SwimType, pool: u32, d: u32| {
        matches!((kind, pool, d), (SwimType::Endurance, 50, 500))
    };
    let mut failures = vec![];
    for kind in kinds {
        for pool in [25, 50] {
            for d in (1000..=8000).step_by(100) {
                let w = build(kind, pool, &[], Some(100.0), d, 0, 0, false);
                let (warm, main) = (swim_warm(&w.blocks[0]), total(&w.blocks[1]));
                let main_min = if d < 2000 { 0.5 } else { 0.58 };
                if warm > 0.27 * f64::from(d) && !exempt(kind, pool, d) {
                    failures.push(format!("{kind:?} pool {pool} D {d}: warm-up {warm}"));
                }
                if main < main_min * f64::from(d) && !exempt(kind, pool, d) {
                    failures.push(format!("{kind:?} pool {pool} D {d}: main {main}"));
                }
            }
            for d in (400..=1000).step_by(100) {
                let w = build(kind, pool, &[], Some(100.0), d, 0, 0, false);
                let main = total(&w.blocks[1]);
                if main < 0.5 * f64::from(d) && !exempt(kind, pool, d) {
                    failures.push(format!("{kind:?} pool {pool} D {d}: main {main}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn warm_up_floor_and_main_share() {
    let kinds = [
        SwimType::EasyRecovery,
        SwimType::Endurance,
        SwimType::Threshold,
        SwimType::Speed,
        SwimType::Technique,
        SwimType::Mixed,
    ];
    let total = |b: &Block| f64::from(b.sets.iter().map(|s| s.reps * s.distance_m).sum::<u32>());
    let mut failures = vec![];
    for kind in kinds {
        for pool in [25, 33, 50] {
            for d in (1000..=4000).step_by(250) {
                for variant in 0..4 {
                    for blocks in [false, true] {
                        let w = build(kind, pool, &[], Some(100.0), d, variant, variant, blocks);
                        // The first warm-up set is the easy swim: at least the floor, rounded down to the pool, less one length.
                        let warm = w.blocks[0].sets[0].distance_m;
                        let floor = ((0.10 * f64::from(d)).max(2.0 * f64::from(pool))
                            / f64::from(pool))
                        .floor() as u32
                            * pool;
                        if warm + pool < floor {
                            failures.push(format!("{kind:?} pool {pool} D {d} variant {variant} blocks {blocks}: warm-up {warm} m, floor {floor} m"));
                        }
                        let main = total(&w.blocks[1]);
                        if main < 0.5 * f64::from(d) {
                            failures.push(format!("{kind:?} pool {pool} D {d} variant {variant} blocks {blocks}: main {main} m"));
                        }
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn speed_sprints_are_capped() {
    let mut failures = vec![];
    for pool in [25, 50] {
        for d in (400..=8000).step_by(100) {
            let w = build(SwimType::Speed, pool, &[], Some(100.0), d, 0, 0, false);
            let sprint: u32 = sets(&w)
                .filter(|s| s.intensity == Intensity::Sprint)
                .map(|s| s.reps * s.distance_m)
                .sum();
            let limit = if d <= 2000 {
                400.0
            } else {
                0.25 * f64::from(d)
            };
            // Pool 25 keeps its 25 m sprint (up to 200 m more), so the limit is checked from pool 50, where that slot is dropped.
            if pool >= 50 && f64::from(sprint) > limit {
                failures.push(format!("pool {pool} D {d}: {sprint} m of sprint"));
            }
            if sets(&w).any(|s| s.intensity == Intensity::Sprint && s.reps > SPRINT_REPS) {
                failures.push(format!(
                    "pool {pool} D {d}: a sprint set has more than {SPRINT_REPS} reps"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn speed_is_not_much_slower_than_endurance() {
    let mut failures = vec![];
    for pool in [25, 50] {
        for css in [None, Some(100.0)] {
            for d in (1000..=5000).step_by(100) {
                let speed = metrics::estimated_duration_s(
                    build(SwimType::Speed, pool, &[], css, d, 0, 0, false),
                    css,
                );
                let endurance = metrics::estimated_duration_s(
                    build(SwimType::Endurance, pool, &[], css, d, 0, 0, false),
                    css,
                );
                if speed > 1.3 * endurance {
                    failures.push(format!("pool {pool} css {css:?} D {d}: speed {speed:.0} s, endurance {endurance:.0} s"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn no_css_note_iff_no_usable_css() {
    for css in [None, Some(0.0), Some(f64::NAN), Some(100.0)] {
        let usable = css.is_some_and(|c| c.is_finite() && c > 0.0);
        let w = generate_workout(
            input("endurance", 25, vec![], Some(2000), None),
            swimmer(css),
        );
        assert_eq!(
            is_no_css_note(w.blocks[0].sets[0].notes.as_deref()),
            !usable,
            "workout, css {css:?}"
        );
        assert!(
            !w.name.contains("default pace"),
            "workout name, css {css:?}"
        );
        let plan = planned("endurance", 25, vec![], 2, 2, 0.0, css);
        assert_eq!(
            plan.name.ends_with(" (default pace)"),
            !usable,
            "plan name, css {css:?}"
        );
        assert_eq!(
            is_no_css_note(plan.weeks[0].workouts[0].blocks[0].sets[0].notes.as_deref()),
            !usable,
            "plan, css {css:?}"
        );
    }
}

#[test]
fn equipment_is_used() {
    let uses = |kind: &str, gear: Equipment| {
        // Generation is random, so some of 30 workouts must use the gear.
        (0..30).any(|_| {
            let w = generate_workout(
                input(kind, 25, vec![gear], Some(2000), None),
                swimmer(Some(100.0)),
            );
            sets(&w).any(|s| s.equipment.contains(&gear))
        })
    };
    assert!(uses("endurance", Equipment::PullBuoy));
    assert!(uses("technique", Equipment::Snorkel));
    assert!(uses("speed", Equipment::Fins));
    assert!(uses("easy recovery", Equipment::PullBuoy));
    let w = generate_workout(
        input(
            "technique",
            25,
            vec![Equipment::Kickboard],
            Some(2000),
            None,
        ),
        swimmer(Some(100.0)),
    );
    assert!(
        sets(&w)
            .any(|s| s.stroke == Stroke::Kickboard && s.equipment.contains(&Equipment::Kickboard))
    );
}

#[test]
fn technique_plan_rotates_drills() {
    let drill_ids = |week: &PlanWeek| -> std::collections::HashSet<String> {
        week.workouts
            .iter()
            .flat_map(sets)
            .filter_map(|s| s.drill.clone())
            .collect()
    };
    let plan = planned("technique", 25, vec![], 8, 3, 0.0, Some(95.0));
    let weeks: Vec<_> = plan.weeks.iter().map(drill_ids).collect();
    assert_ne!(weeks[0], weeks[1], "week 1 and week 2 use the same drills");
    let all: std::collections::HashSet<&String> = weeks.iter().flatten().collect();
    assert!(
        all.len() >= 6,
        "{} distinct drills over 8 weeks: {all:?}",
        all.len()
    );
}

#[test]
fn warm_skill_rotates_within_a_week() {
    // Week 1 of a 4-week, 4-swim plan: the warm-up skills that are there differ between swims (a skill can give
    // way to the warm-up floor, so not every swim has one).
    let plan = planned("endurance", 25, vec![], 4, 4, 0.0, Some(95.0));
    let skills: Vec<String> = plan.weeks[0]
        .workouts
        .iter()
        .filter_map(|w| w.blocks[0].sets.iter().find_map(|s| s.drill.clone()))
        .collect();
    let distinct = skills
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert!(
        skills.len() >= 2 && distinct >= 2,
        "week 1 warm-up skills: {skills:?}"
    );
}

#[test]
fn easy_recovery_ends_with_a_game() {
    for blocks in [false, true] {
        for seed in 0..10 {
            let w = build(
                SwimType::EasyRecovery,
                25,
                &[],
                Some(100.0),
                2000,
                0,
                seed,
                blocks,
            );
            let end = w.blocks[1]
                .sets
                .last()
                .and_then(|s| s.drill.clone())
                .expect("end skill");
            assert_eq!(
                exercise(end.clone()).unwrap().kind,
                ExerciseKind::Fun,
                "{end} with blocks {blocks}, seed {seed}"
            );
        }
    }
}

#[test]
fn snorkel_is_not_added_to_head_up() {
    let mut seen = false;
    for seed in 0..10 {
        let w = build(
            SwimType::Technique,
            25,
            &[Equipment::Snorkel],
            Some(100.0),
            2000,
            0,
            seed,
            false,
        );
        for s in sets(&w).filter(|s| s.drill.as_deref() == Some("head-up")) {
            seen = true;
            assert!(!s.equipment.contains(&Equipment::Snorkel), "seed {seed}");
        }
    }
    assert!(seen, "head-up is never picked, so the test checks nothing");
}

#[test]
fn starts_only_with_blocks() {
    let starts = |w: &Workout| {
        sets(w)
            .filter_map(|s| s.drill.clone())
            .filter(|id| exercise(id.clone()).unwrap().kind == ExerciseKind::Start)
            .count()
    };
    // A start rep is at least two lengths: the dive ends back at the block end.
    for (pool, dive_m) in [(25, 50), (50, 100)] {
        let with = CreatorInputs {
            starting_blocks: true,
            ..input("speed", pool, vec![], Some(2000), None)
        };
        let w = generate_workout(with, swimmer(Some(100.0)));
        let dive = sets(&w)
            .find(|s| s.drill.as_deref() == Some("dive-and-swim"))
            .unwrap_or_else(|| panic!("pool {pool}: no dive set"));
        assert_eq!((dive.distance_m, dive.reps), (dive_m, 4), "pool {pool}");
        for kind in [
            "speed",
            "mixed",
            "endurance",
            "threshold",
            "easy recovery",
            "technique",
        ] {
            let w = generate_workout(
                input(kind, pool, vec![], Some(2000), None),
                swimmer(Some(100.0)),
            );
            assert_eq!(starts(&w), 0, "{kind} without blocks, pool {pool}");
        }
    }
}

#[test]
fn drills_need_their_gear() {
    let scull = |equipment: &[Equipment]| {
        (0..10).any(|seed| {
            sets(&build(
                SwimType::Technique,
                25,
                equipment,
                Some(100.0),
                2000,
                0,
                seed,
                false,
            ))
            .any(|s| s.drill.as_deref() == Some("front-scull"))
        })
    };
    assert!(!scull(&[]), "front-scull without a pull buoy");
    assert!(
        scull(&[Equipment::PullBuoy]),
        "no seed 0..10 has front-scull with a pull buoy"
    );
}

#[test]
fn same_inputs_vary_but_keep_the_distance() {
    let make = || {
        generate_workout(
            input(
                "endurance",
                25,
                vec![Equipment::Kickboard],
                Some(2000),
                None,
            ),
            swimmer(Some(110.0)),
        )
    };
    let first = make();
    assert!(
        (0..50).any(|_| make() != first),
        "50 generated workouts were all identical"
    );
    for _ in 0..20 {
        assert_eq!(make().total_distance_m(), first.total_distance_m());
    }
}
