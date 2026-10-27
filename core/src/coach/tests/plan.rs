use super::*;

pub(super) fn planned(
    kind: &str,
    pool: u32,
    equipment: Vec<Equipment>,
    weeks: u32,
    swims: u32,
    weekly: f64,
    css: Option<f64>,
) -> Plan {
    let inputs = CreatorInputs {
        plan_weeks: Some(weeks),
        swims_per_week: Some(swims),
        ..input(kind, pool, equipment, Some(1500), None)
    };
    generate_plan(
        inputs,
        SwimmerProfile {
            weekly_m_28d: weekly,
            ..swimmer(css)
        },
    )
}

#[test]
fn plan_grid_follows_the_rules() {
    const KINDS: [&str; 6] = [
        "endurance",
        "threshold",
        "speed",
        "technique",
        "mixed",
        "easy recovery",
    ];
    const POOLS: [u32; 2] = [25, 50];
    const WEEKS: [u32; 3] = [1, 4, 9];
    const SWIMS: [u32; 4] = [1, 2, 3, 5];
    const WEEKLY: [f64; 3] = [0.0, 3000.0, 9000.0];
    const CSSES: [Option<f64>; 2] = [None, Some(95.0)];
    let gears = [
        vec![],
        vec![
            Equipment::Fins,
            Equipment::Paddles,
            Equipment::PullBuoy,
            Equipment::Kickboard,
            Equipment::Snorkel,
        ],
    ];
    let mut failures = vec![];
    let mut case = 0;
    for kind in KINDS {
        for pool in POOLS {
            for weeks in WEEKS {
                for swims in SWIMS {
                    for weekly in WEEKLY {
                        for css in CSSES {
                            for equipment in &gears {
                                // Alternate the per-swim target between a distance and a time.
                                let (distance, minutes) = if case % 2 == 0 {
                                    (Some(1500), None)
                                } else {
                                    (None, Some(45))
                                };
                                case += 1;
                                let inputs = CreatorInputs {
                                    plan_weeks: Some(weeks),
                                    swims_per_week: Some(swims),
                                    ..input(kind, pool, equipment.clone(), distance, minutes)
                                };
                                let plan = generate_plan(
                                    inputs,
                                    SwimmerProfile {
                                        weekly_m_28d: weekly,
                                        ..swimmer(css)
                                    },
                                );
                                let mut bad = vec![];
                                if let Err(e) = plan.validate() {
                                    bad.push(format!("invalid: {e}"));
                                }
                                if plan.weeks.len() != weeks as usize
                                    || plan
                                        .weeks
                                        .iter()
                                        .any(|wk| wk.workouts.len() != swims as usize)
                                {
                                    bad.push("week or swim count is wrong".into());
                                }
                                if plan.weeks.iter().flat_map(|wk| &wk.workouts).any(|w| {
                                    !sets(w)
                                        .all(|s| s.equipment.iter().all(|e| equipment.contains(e)))
                                }) {
                                    bad.push("a set uses gear that is not in the inputs".into());
                                }
                                for v in bad {
                                    failures.push(format!("kind {kind:?}, pool {pool}, weeks {weeks}, swims {swims}, weekly {weekly}, css {css:?}, equipment {equipment:?}: {v}"));
                                }
                            }
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
fn plan_progression() {
    let plan = planned("endurance", 25, vec![], 12, 3, 6000.0, Some(95.0));
    let totals: Vec<f64> = plan
        .weeks
        .iter()
        .map(|wk| {
            f64::from(
                wk.workouts
                    .iter()
                    .map(Workout::total_distance_m)
                    .sum::<u32>(),
            )
        })
        .collect();
    assert!(
        (totals[0] - 6000.0).abs() <= 600.0,
        "week 1 total {}",
        totals[0]
    );
    // Each build week against the previous build week; 7% until the 1.5x cap binds at week 9, then flat.
    let builds: Vec<usize> = (1..=12).filter(|w| w % 4 != 0).collect();
    for pair in builds.windows(2) {
        let growth = totals[pair[1] - 1] / totals[pair[0] - 1];
        let floor = if pair[1] <= 7 { 1.04 } else { 0.99 };
        assert!(
            (floor..=1.11).contains(&growth),
            "week {} vs {}: growth {growth:.3}",
            pair[1],
            pair[0]
        );
    }
    for w in [4, 8, 12] {
        let ratio = totals[w - 1] / totals[w - 2];
        assert!(
            (0.65..=0.75).contains(&ratio),
            "lighter week {w}: ratio {ratio:.3}"
        );
    }
    let ratio = totals[4] / totals[2];
    assert!(
        (1.03..=1.11).contains(&ratio),
        "week 5 vs week 3: ratio {ratio:.3}"
    );
}

#[test]
fn plan_growth_is_capped() {
    let plan = planned("endurance", 25, vec![], 16, 3, 4000.0, Some(95.0));
    let max = f64::from(
        plan.weeks
            .iter()
            .map(|wk| {
                wk.workouts
                    .iter()
                    .map(Workout::total_distance_m)
                    .sum::<u32>()
            })
            .max()
            .unwrap(),
    );
    assert!(max <= 1.5 * 4000.0 * 1.02, "largest week {max}");
    assert!(max >= 1.4 * 4000.0, "largest week {max} shows no growth");
}

#[test]
fn plan_time_target_sets_each_swim_time() {
    let inputs = CreatorInputs {
        plan_weeks: Some(8),
        swims_per_week: Some(5),
        ..input("endurance", 25, vec![], None, Some(45))
    };
    let plan = generate_plan(inputs, swimmer(Some(100.0)));
    let mut factor: Vec<f64> = vec![];
    for w in 1..=8usize {
        let f = if w % 4 == 0 {
            0.7 * factor[w - 2]
        } else {
            1.07f64
                .powi((1..w).filter(|&u| u % 4 != 0).count() as i32)
                .min(1.5)
        };
        factor.push(f);
        for swim in &plan.weeks[w - 1].workouts {
            let secs = metrics::estimated_duration_s(swim.clone(), Some(100.0));
            let want = 45.0 * 60.0 * f;
            assert!(
                (secs - want).abs() <= 0.1 * want,
                "week {w} {}: {secs:.0} s, want {want:.0} s",
                swim.name
            );
        }
    }
}

#[test]
fn plan_rotates_every_type_across_weeks() {
    let plan = planned("endurance", 25, vec![], 3, 3, 4000.0, Some(95.0));
    let names: Vec<&str> = plan
        .weeks
        .iter()
        .flat_map(|wk| wk.workouts.iter().map(|w| w.name.as_str()))
        .collect();
    for t in [
        "Easy recovery",
        "Endurance",
        "Threshold",
        "Speed",
        "Technique",
        "Mixed",
    ] {
        assert!(
            names.iter().any(|n| n.starts_with(t)),
            "{t} missing in {names:?}"
        );
    }
}

#[test]
fn plan_types_vary() {
    const TYPES: [&str; 6] = [
        "Easy recovery",
        "Endurance",
        "Threshold",
        "Speed",
        "Technique",
        "Mixed",
    ];
    for kind in ["threshold", "easy recovery", "speed", "something else"] {
        for swims in [2, 3, 5, 7] {
            let plan = planned(kind, 25, vec![], 8, swims, 4000.0, Some(95.0));
            let requested = SwimType::parse(kind).name("en");
            assert!(
                plan.weeks[0].workouts[0].name.starts_with(requested),
                "{kind}: first swim is {}",
                plan.weeks[0].workouts[0].name
            );
            for (n, week) in plan.weeks.iter().enumerate() {
                let types: Vec<&str> = week
                    .workouts
                    .iter()
                    .map(|w| {
                        TYPES
                            .iter()
                            .copied()
                            .find(|t| w.name.starts_with(t))
                            .unwrap()
                    })
                    .collect();
                assert!(
                    types.windows(2).all(|p| p[0] != p[1]),
                    "{kind}, week {}: neighbours repeat {types:?}",
                    n + 1
                );
                if types.len() >= 3 {
                    let distinct = types.iter().collect::<std::collections::HashSet<_>>().len();
                    assert!(
                        distinct >= 3,
                        "{kind}, week {}: only {distinct} types {types:?}",
                        n + 1
                    );
                }
            }
        }
    }
}

#[test]
fn plan_without_history_uses_the_target() {
    for swims in [1, 2, 3, 5] {
        let plan = planned("endurance", 25, vec![], 4, swims, 0.0, Some(95.0));
        let total = f64::from(
            plan.weeks[0]
                .workouts
                .iter()
                .map(Workout::total_distance_m)
                .sum::<u32>(),
        );
        let want = f64::from(swims * 1500);
        assert!(
            (total - want).abs() <= 0.1 * want,
            "{swims} swims: total {total}, want {want}"
        );
    }
}

#[test]
fn same_inputs_same_plan() {
    let a = planned(
        "speed",
        25,
        vec![Equipment::Fins],
        6,
        3,
        4000.0,
        Some(110.0),
    );
    let b = planned(
        "speed",
        25,
        vec![Equipment::Fins],
        6,
        3,
        4000.0,
        Some(110.0),
    );
    assert_eq!(a, b);
}

#[test]
fn every_selected_gear_is_used() {
    let kinds = [
        SwimType::EasyRecovery,
        SwimType::Endurance,
        SwimType::Threshold,
        SwimType::Speed,
        SwimType::Technique,
        SwimType::Mixed,
    ];
    for kind in kinds {
        for gear in [Equipment::Paddles, Equipment::Fins] {
            for pool in [25, 50] {
                let w = build(kind, pool, &[gear], None, 2000, 0, 0, false);
                let main = &w.blocks[1].sets;
                assert!(
                    main.iter().any(|s| s.equipment.contains(&gear)),
                    "{kind:?} pool {pool} has no {gear:?} set"
                );
                if gear == Equipment::Paddles {
                    assert!(
                        main.iter()
                            .filter(|s| s.equipment.contains(&gear))
                            .all(|s| s.stroke == Stroke::Freestyle
                                && s.intensity != Intensity::Sprint),
                        "{kind:?} pool {pool} paddles on a non-fitting set"
                    );
                }
            }
        }
    }
}

#[test]
fn no_gear_no_equipment_in_main() {
    for kind in [SwimType::Endurance, SwimType::Speed, SwimType::Mixed] {
        let w = build(kind, 25, &[], None, 2000, 0, 0, false);
        assert!(
            sets(&w).all(|s| s.equipment.is_empty()),
            "{kind:?} got gear it was not given"
        );
    }
}

#[test]
fn paddles_without_pull_buoy() {
    for kind in [SwimType::Endurance, SwimType::EasyRecovery] {
        let w = build(kind, 25, &[Equipment::Paddles], None, 2000, 0, 0, false);
        let used = w
            .blocks
            .iter()
            .flat_map(|b| &b.sets)
            .any(|s| s.equipment == vec![Equipment::Paddles]);
        assert!(used, "{kind:?} has no paddle set");
    }
}

#[test]
fn kick_set_only_when_kickboard_selected() {
    for kind in [SwimType::Endurance, SwimType::Threshold, SwimType::Speed] {
        let with = build(
            kind,
            25,
            &[Equipment::Kickboard],
            Some(100.0),
            2000,
            0,
            0,
            false,
        );
        assert!(
            sets(&with)
                .any(|s| s.stroke == Stroke::Kickboard
                    && s.equipment.contains(&Equipment::Kickboard)),
            "{kind:?} has no kick set"
        );
        let without = build(kind, 25, &[], Some(100.0), 2000, 0, 0, false);
        assert!(
            !sets(&without).any(|s| s.stroke == Stroke::Kickboard),
            "{kind:?} has a kick set without a kickboard"
        );
    }
}
