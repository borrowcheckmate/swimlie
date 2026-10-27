use super::*;

#[test]
fn grid_follows_the_rules() {
    const KINDS: [&str; 7] = [
        "easy recovery",
        "endurance",
        "threshold",
        "speed",
        "technique",
        "mixed",
        "something else",
    ];
    const POOLS: [u32; 4] = [20, 25, 33, 50];
    const TARGETS: [(Option<u32>, Option<u32>); 12] = [
        (Some(400), None),
        (Some(500), None),
        (Some(1000), None),
        (Some(2000), None),
        (Some(3500), None),
        (Some(5000), None),
        (Some(8000), None),
        (None, Some(15)),
        (None, Some(30)),
        (None, Some(45)),
        (None, Some(75)),
        (None, Some(120)),
    ];
    const CSSES: [Option<f64>; 4] = [None, Some(75.0), Some(100.0), Some(150.0)];
    let gears = [
        vec![],
        vec![
            Equipment::Fins,
            Equipment::Paddles,
            Equipment::PullBuoy,
            Equipment::Kickboard,
            Equipment::Snorkel,
        ],
        vec![Equipment::PullBuoy],
        vec![Equipment::Kickboard, Equipment::Fins],
        vec![Equipment::Snorkel, Equipment::Paddles],
    ];
    let mut failures = vec![];
    for kind in KINDS {
        for pool in POOLS {
            for (distance, minutes) in TARGETS {
                for css in CSSES {
                    for equipment in &gears {
                        for blocks in [false, true] {
                            for v in
                                violations(kind, pool, equipment, distance, minutes, css, blocks)
                            {
                                failures.push(format!("kind {kind:?}, pool {pool}, target {distance:?} m / {minutes:?} min, css {css:?}, equipment {equipment:?}, blocks {blocks}: {v}"));
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
fn beginner_without_css_paces_at_200_and_has_no_sprint_or_hard() {
    for kind in [
        SwimType::Speed,
        SwimType::Threshold,
        SwimType::Endurance,
        SwimType::Mixed,
    ] {
        for d in [800, 1500, 2500] {
            for variant in 0..12 {
                let w = build_shuffled(
                    kind,
                    25,
                    &[],
                    None,
                    SwimLevel::Beginner,
                    d,
                    variant,
                    variant,
                    false,
                    None,
                    "en",
                );
                assert!(
                    sets(&w).all(|s| !matches!(s.intensity, Intensity::Sprint | Intensity::Hard)),
                    "{kind:?} D {d} variant {variant}"
                );
                for s in sets(&w).filter(|s| {
                    s.stroke == Stroke::Freestyle
                        && s.intensity == Intensity::Threshold
                        && s.goal_s.is_some()
                }) {
                    assert_eq!(
                        s.goal_s,
                        Some((265.0_f64 * 1.02 * f64::from(s.distance_m) / 100.0).round() as u32),
                        "{kind:?} {} m",
                        s.distance_m
                    );
                }
            }
        }
    }
}

#[test]
fn forty_minutes_without_css_by_level() {
    for (level, want) in [
        (SwimLevel::Beginner, 800.0),
        (SwimLevel::Intermediate, 1200.0),
        (SwimLevel::Advanced, 1400.0),
    ] {
        for _ in 0..20 {
            let w = generate_workout(
                input("endurance", 25, vec![], None, Some(40)),
                SwimmerProfile {
                    level,
                    ..swimmer(None)
                },
            );
            let d = f64::from(w.total_distance_m());
            assert!(
                (d / want - 1.0).abs() <= 0.15,
                "{level:?}: {d} m, want about {want} m"
            );
        }
    }
}

#[test]
fn plan_volume_scales_by_level_without_history() {
    let inputs = CreatorInputs {
        plan_weeks: Some(1),
        swims_per_week: Some(3),
        ..input("endurance", 25, vec![], Some(2000), None)
    };
    let total = |level| {
        let plan = generate_plan(
            inputs.clone(),
            SwimmerProfile {
                level,
                ..swimmer(Some(100.0))
            },
        );
        f64::from(
            plan.weeks[0]
                .workouts
                .iter()
                .map(|w| sets(w).map(|s| s.reps * s.distance_m).sum::<u32>())
                .sum::<u32>(),
        )
    };
    let (b, i, a) = (
        total(SwimLevel::Beginner),
        total(SwimLevel::Intermediate),
        total(SwimLevel::Advanced),
    );
    assert!(
        (b / i - 0.6).abs() < 0.05,
        "beginner {b} vs intermediate {i}"
    );
    assert!(
        (a / i - 1.2).abs() < 0.05,
        "advanced {a} vs intermediate {i}"
    );
}

#[test]
fn goal_and_interval_from_css() {
    let w = generate_workout(
        CreatorInputs {
            strokes: vec![Stroke::Freestyle],
            ..input("threshold", 25, vec![], Some(2000), None)
        },
        swimmer(Some(100.0)),
    );
    let set = w.blocks[1]
        .sets
        .iter()
        .find(|s| s.distance_m == 100)
        .unwrap();
    assert_eq!(set.goal_s, Some(102));
    assert_eq!(
        set.rest,
        Some(Rest {
            kind: RestKind::Interval,
            seconds: 120
        })
    );
}

#[test]
fn paces_scale_css_by_intensity_and_stroke() {
    let same = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let intensities = [
        (Intensity::Easy, 1.15),
        (Intensity::Moderate, 1.08),
        (Intensity::Threshold, 1.02),
        (Intensity::Hard, 0.96),
        (Intensity::Sprint, 0.88),
    ];
    for (intensity, x) in intensities {
        assert!(
            same(pace(Some(75.0), Stroke::Freestyle, intensity), 75.0 * x),
            "CSS 75 {intensity:?}"
        );
        assert!(
            same(pace(Some(150.0), Stroke::Freestyle, intensity), 150.0 * x),
            "CSS 150 {intensity:?}"
        );
    }
    assert!(same(
        pace(Some(75.0), Stroke::Freestyle, Intensity::Easy),
        86.25
    ));
    assert!(same(
        pace(Some(75.0), Stroke::Freestyle, Intensity::Moderate),
        81.0
    ));
    assert!(same(
        pace(Some(75.0), Stroke::Freestyle, Intensity::Threshold),
        76.5
    ));
    assert!(same(
        pace(Some(75.0), Stroke::Freestyle, Intensity::Hard),
        72.0
    ));
    assert!(same(
        pace(Some(75.0), Stroke::Freestyle, Intensity::Sprint),
        66.0
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Freestyle, Intensity::Easy),
        172.5
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Freestyle, Intensity::Moderate),
        162.0
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Freestyle, Intensity::Threshold),
        153.0
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Freestyle, Intensity::Hard),
        144.0
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Freestyle, Intensity::Sprint),
        132.0
    ));
    let strokes = [
        (Stroke::Freestyle, 1.0),
        (Stroke::Any, 1.0),
        (Stroke::Kickboard, 1.0),
        (Stroke::Backstroke, 1.10),
        (Stroke::Butterfly, 1.12),
        (Stroke::Mixed, 1.12),
        (Stroke::Breaststroke, 1.20),
    ];
    for (stroke, x) in strokes {
        assert!(
            same(
                pace(Some(75.0), stroke, Intensity::Moderate),
                75.0 * 1.08 * x
            ),
            "CSS 75 {stroke:?}"
        );
        assert!(
            same(
                pace(Some(150.0), stroke, Intensity::Moderate),
                150.0 * 1.08 * x
            ),
            "CSS 150 {stroke:?}"
        );
    }
    assert!(same(
        pace(Some(75.0), Stroke::Backstroke, Intensity::Moderate),
        89.1
    ));
    assert!(same(
        pace(Some(75.0), Stroke::Butterfly, Intensity::Moderate),
        90.72
    ));
    assert!(same(
        pace(Some(75.0), Stroke::Mixed, Intensity::Moderate),
        90.72
    ));
    assert!(same(
        pace(Some(75.0), Stroke::Breaststroke, Intensity::Moderate),
        97.2
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Backstroke, Intensity::Moderate),
        178.2
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Butterfly, Intensity::Moderate),
        181.44
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Mixed, Intensity::Moderate),
        181.44
    ));
    assert!(same(
        pace(Some(150.0), Stroke::Breaststroke, Intensity::Moderate),
        194.4
    ));
}
