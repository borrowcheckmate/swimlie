use super::*;

const EXAMPLE: &str = include_str!("../../../schema/example.json");
const PLAN: &str = include_str!("../../../schema/plan-example.json");
const SCHEMA: &str = include_str!("../../../schema/workout.schema.json");
const PLAN_SCHEMA: &str = include_str!("../../../schema/plan.schema.json");

#[test]
fn description_is_optional_and_capped() {
    let w = parse_workout(EXAMPLE.into()).unwrap();
    assert_eq!(w.description, None);
    let with = EXAMPLE.replacen(
        '{',
        &format!("{{\"description\": \"  {}  \",", "x".repeat(500)),
        1,
    );
    let d = parse_workout(with).unwrap().description.unwrap();
    assert_eq!(d.chars().count(), DESCRIPTION_MAX_CHARS);
    assert_eq!(
        parse_workout(EXAMPLE.replacen('{', "{\"description\": \"Easy swim.\",", 1))
            .unwrap()
            .description
            .as_deref(),
        Some("Easy swim.")
    );
}

#[test]
fn example_parses() {
    let w = parse_workout(EXAMPLE.into()).unwrap();
    assert_eq!(w.total_distance_m(), 400 + 2 * (4 * 100 + 200));
    assert_eq!(parse_workout(workout_to_json(w.clone())).unwrap(), w);
}

/// The workout swum for the spike 0b recording; its labelled laps are in the same folder.
#[test]
fn swim_a_workout_parses() {
    let json = include_str!("../../tests/data/recorded-swim-a/workout.json");
    assert_eq!(parse_workout(json.into()).unwrap().total_distance_m(), 1900);
}

#[test]
fn rejects_bad_input() {
    let bad = EXAMPLE.replace("\"distance_m\": 400", "\"distance_m\": 410");
    assert!(matches!(
        parse_workout(bad),
        Err(WorkoutError::Invalid { .. })
    ));
    let bad = EXAMPLE.replace("\"version\": 1", "\"version\": 2");
    assert!(matches!(
        parse_workout(bad),
        Err(WorkoutError::Invalid { .. })
    ));
    let bad = EXAMPLE.replace("\"stroke\": \"any\"", "\"stroke\": \"doggy\"");
    assert!(matches!(
        parse_workout(bad),
        Err(WorkoutError::Invalid { .. })
    ));
    assert!(matches!(
        parse_workout(r#"{"foo":1}"#.into()),
        Err(WorkoutError::Invalid { .. })
    ));
    assert!(matches!(
        parse_workout("{".into()),
        Err(WorkoutError::Json { .. })
    ));
}

#[test]
fn drill_ids_are_checked_on_import_only() {
    let with = |id| {
        EXAMPLE.replacen(
            "\"distance_m\": 400",
            &format!("\"distance_m\": 400, \"drill\": \"{id}\""),
            1,
        )
    };
    assert!(parse_import(with("catch-up")).is_ok());
    assert!(
        matches!(parse_import(with("moonwalk")), Err(WorkoutError::Invalid { msg }) if msg.contains("unknown drill 'moonwalk'"))
    );
    // Stored and synced workouts parse and validate, so a drill from a newer catalogue is kept.
    let w = parse_workout(with("moonwalk")).unwrap();
    assert_eq!(w.blocks[0].sets[0].drill.as_deref(), Some("moonwalk"));
    assert!(w.validate().is_ok());
    assert_eq!(parse_workout(workout_to_json(w.clone())).unwrap(), w);
}

#[test]
fn plan_example_parses_and_round_trips() {
    let p = parse_plan(PLAN.into()).unwrap();
    assert_eq!(p.weeks.len(), 2);
    assert_eq!(parse_plan(plan_to_json(p.clone())).unwrap(), p);
}

#[test]
fn parse_import_picks_plan_or_workout() {
    assert_eq!(
        parse_import(PLAN.into()).unwrap(),
        Import::Plan {
            plan: parse_plan(PLAN.into()).unwrap()
        }
    );
    assert_eq!(
        parse_import(EXAMPLE.into()).unwrap(),
        Import::Workout {
            workout: parse_workout(EXAMPLE.into()).unwrap()
        }
    );
    assert!(matches!(
        parse_import("{".into()),
        Err(WorkoutError::Json { .. })
    ));
}

#[test]
fn rejects_bad_plans() {
    let mut p = parse_plan(PLAN.into()).unwrap();
    p.weeks.clear();
    assert!(matches!(p.validate(), Err(WorkoutError::Invalid { .. })));
    let mut p = parse_plan(PLAN.into()).unwrap();
    p.weeks[0].workouts.clear();
    assert!(matches!(p.validate(), Err(WorkoutError::Invalid { msg }) if msg.contains("week 1")));
    let mut p = parse_plan(PLAN.into()).unwrap();
    p.weeks[0].workouts[0].blocks[0].sets[0].distance_m = 110;
    assert!(
        matches!(p.validate(), Err(WorkoutError::Invalid { msg }) if msg.contains("week 1, workout 1"))
    );
    assert!(matches!(
        parse_plan(r#"{"version":1,"name":"x"}"#.into()),
        Err(WorkoutError::Invalid { .. })
    ));
}

/// Keeps a published schema in sync with its Rust type.
/// Regenerate with: `UPDATE_SCHEMA=1` cargo test
fn check_schema<T: JsonSchema>(file: &str, published: &str) {
    let generated = serde_json::to_string_pretty(&schemars::schema_for!(T)).unwrap() + "\n";
    if std::env::var_os("UPDATE_SCHEMA").is_some() {
        std::fs::write(
            format!("{}/../schema/{file}", env!("CARGO_MANIFEST_DIR")),
            &generated,
        )
        .unwrap();
    } else {
        assert_eq!(
            published, generated,
            "schema/{file} is stale, run UPDATE_SCHEMA=1 cargo test"
        );
    }
}

#[test]
fn schema_up_to_date() {
    check_schema::<Workout>("workout.schema.json", SCHEMA);
}

#[test]
fn plan_schema_up_to_date() {
    check_schema::<Plan>("plan.schema.json", PLAN_SCHEMA);
}
