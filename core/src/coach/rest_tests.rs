use super::tests::{input, sets, swimmer};
use super::*;

#[test]
fn rest_percent_scales_rest_not_goal() {
    let base = generate_workout(
        CreatorInputs {
            rest_percent: 0,
            strokes: vec![Stroke::Freestyle],
            ..test_inputs()
        },
        test_swimmer(),
    );
    let more = generate_workout(
        CreatorInputs {
            rest_percent: 50,
            strokes: vec![Stroke::Freestyle],
            ..test_inputs()
        },
        test_swimmer(),
    );
    let rest_sum = |w: &Workout| {
        w.blocks
            .iter()
            .flat_map(|b| b.sets.iter().map(move |s| (b.repeat, s)))
            .filter_map(|(n, s)| {
                s.rest.as_ref().map(|r| {
                    n * r.seconds.saturating_sub(if r.kind == RestKind::Interval {
                        s.goal_s.unwrap_or(0)
                    } else {
                        0
                    })
                })
            })
            .sum::<u32>()
    };
    assert!(rest_sum(&more) > rest_sum(&base));
}

#[test]
fn single_stroke_leaves_no_other_swim_stroke() {
    for kind in [
        "easy recovery",
        "endurance",
        "threshold",
        "speed",
        "technique",
        "mixed",
    ] {
        let w = generate_workout(
            CreatorInputs {
                strokes: vec![Stroke::Breaststroke],
                kind: kind.into(),
                ..test_inputs()
            },
            test_swimmer(),
        );
        for s in w.blocks.iter().flat_map(|b| &b.sets) {
            assert!(
                !matches!(
                    s.stroke,
                    Stroke::Freestyle | Stroke::Backstroke | Stroke::Butterfly | Stroke::Mixed
                ),
                "{kind}: {:?}",
                s.stroke
            );
        }
    }
}

#[test]
fn several_strokes_make_a_repeated_round() {
    let w = generate_workout(
        CreatorInputs {
            strokes: vec![Stroke::Freestyle, Stroke::Breaststroke, Stroke::Backstroke],
            ..test_inputs()
        },
        test_swimmer(),
    );
    let main = &w.blocks[1];
    assert!(
        main.repeat >= 2 && (3..=5).contains(&main.sets.len()),
        "repeat {} sets {}",
        main.repeat,
        main.sets.len()
    );
    assert!(main.sets.iter().all(|s| s.reps >= 1));
    assert_eq!(w.total_distance_m(), 2000);
    w.validate().unwrap();
}

#[test]
fn rounds_come_in_several_shapes() {
    let names: std::collections::HashSet<String> = (0..80)
        .map(|_| {
            generate_workout(
                CreatorInputs {
                    strokes: vec![Stroke::Freestyle, Stroke::Breaststroke],
                    ..test_inputs()
                },
                test_swimmer(),
            )
            .blocks[1]
                .name
                .clone()
        })
        .collect();
    assert!(names.len() >= 4, "{names:?}");
}

#[test]
fn chosen_strokes_are_used_in_main() {
    for _ in 0..20 {
        let w = generate_workout(
            CreatorInputs {
                strokes: vec![Stroke::Backstroke, Stroke::Breaststroke],
                ..test_inputs()
            },
            test_swimmer(),
        );
        let main = w.blocks[1].sets.iter().filter(|s| s.goal_s.is_some());
        assert!(
            main.clone().count() > 0
                && main
                    .into_iter()
                    .all(|s| matches!(s.stroke, Stroke::Backstroke | Stroke::Breaststroke))
        );
    }
}

fn test_inputs() -> CreatorInputs {
    CreatorInputs {
        language: "en".into(),
        pool_length_m: 25,
        equipment: vec![],
        starting_blocks: false,
        target_distance_m: Some(2000),
        target_time_min: None,
        kind: "endurance".into(),
        notes: String::new(),
        plan_weeks: None,
        swims_per_week: None,
        rest_percent: 0,
        strokes: vec![],
    }
}

fn test_swimmer() -> SwimmerProfile {
    SwimmerProfile {
        css_s_per_100m: Some(110.0),
        sessions_28d: 0,
        weekly_m_28d: 0.0,
        rep_times: vec![],
        age_years: None,
        max_hr_bpm: None,
        level: SwimLevel::default(),
    }
}

#[test]
fn dutch_keeps_the_structure_and_translates_the_text() {
    let shape = |w: &Workout| {
        w.blocks
            .iter()
            .map(|b| {
                (
                    b.repeat,
                    b.sets
                        .iter()
                        .map(|s| (s.reps, s.distance_m, s.goal_s, s.drill.clone()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    for kind in [
        SwimType::Endurance,
        SwimType::Technique,
        SwimType::Speed,
        SwimType::Mixed,
        SwimType::EasyRecovery,
    ] {
        for css in [Some(100.0), None] {
            let build = |lang| {
                build_shuffled(
                    kind,
                    25,
                    &[Equipment::Kickboard],
                    css,
                    SwimLevel::Intermediate,
                    2000,
                    3,
                    4,
                    true,
                    None,
                    lang,
                )
            };
            let (en, nl) = (build("en"), build("nl"));
            assert_eq!(shape(&en), shape(&nl), "{kind:?} css {css:?}");
            assert_eq!(nl.blocks[0].name, "Warming-up");
            assert_eq!(nl.blocks[2].name, "Cooling-down");
            assert!(
                nl.name.starts_with(&format!("{} ", kind.name("nl"))),
                "{}",
                nl.name
            );
            assert!(
                !nl.name.contains("Endurance") && !nl.name.contains("Threshold"),
                "{}",
                nl.name
            );
            let note = nl.blocks[0].sets[0].notes.clone();
            assert_eq!(is_no_css_note(note.as_deref()), css.is_none());
            if css.is_none() {
                assert!(note.unwrap().starts_with("Nog geen CSS"));
            }
            for s in sets(&nl).filter(|s| s.drill.is_some()) {
                let id = s.drill.clone().unwrap();
                assert_eq!(
                    s.notes.as_deref(),
                    Some(
                        crate::exercises::exercise_in(id, "nl".into())
                            .unwrap()
                            .cue
                            .as_str()
                    )
                );
            }
        }
    }
}

#[test]
fn dutch_plan_name_and_focus() {
    let inputs = CreatorInputs {
        language: "nl".into(),
        plan_weeks: Some(6),
        ..input("endurance", 25, vec![], Some(2000), None)
    };
    let plan = generate_plan(inputs, swimmer(None));
    assert_eq!(plan.name, "6-weekse duurtraining-plan (standaardtempo)");
    assert_eq!(plan.weeks[0].focus.as_deref(), Some("Basis"));
    assert_eq!(plan.weeks[1].focus.as_deref(), Some("Opbouw"));
    assert_eq!(plan.weeks[3].focus.as_deref(), Some("Lichtere week"));
}
