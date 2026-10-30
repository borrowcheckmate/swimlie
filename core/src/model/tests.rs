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

fn gear_error(import: Import, allowed: Vec<Equipment>) -> String {
    match check_equipment(import, allowed) {
        Err(WorkoutError::Invalid { msg }) => msg,
        other => panic!("expected an invalid workout, got {other:?}"),
    }
}

#[test]
fn equipment_check_covers_sets_and_drills() {
    let w = parse_workout(EXAMPLE.into()).unwrap();
    let import = || Import::Workout { workout: w.clone() };
    // The Main set uses pull buoy and paddles.
    assert!(check_equipment(import(), vec![Equipment::PullBuoy, Equipment::Paddles]).is_ok());
    assert_eq!(
        gear_error(import(), vec![Equipment::Fins]),
        "block 'Main': uses pull buoy, paddles, allowed: fins"
    );
    assert_eq!(
        gear_error(import(), vec![]),
        "block 'Main': uses pull buoy, paddles, allowed: none"
    );

    // A drill's gear counts too: front-scull needs a pull buoy.
    let with_drill = EXAMPLE.replacen(
        "\"distance_m\": 400",
        "\"distance_m\": 400, \"drill\": \"front-scull\"",
        1,
    );
    let w = parse_workout(with_drill).unwrap();
    let import = || Import::Workout { workout: w.clone() };
    assert_eq!(
        gear_error(import(), vec![Equipment::Paddles]),
        "block 'Warm-up': uses pull buoy, allowed: paddles"
    );
    assert!(check_equipment(import(), vec![Equipment::PullBuoy, Equipment::Paddles]).is_ok());
    // A drill without gear is fine with no equipment at all.
    let no_gear = EXAMPLE.replacen(
        "\"distance_m\": 400",
        "\"distance_m\": 400, \"drill\": \"catch-up\"",
        1,
    );
    let mut w = parse_workout(no_gear).unwrap();
    w.blocks[1].sets[0].equipment.clear();
    assert!(check_equipment(Import::Workout { workout: w }, vec![]).is_ok());
}

#[test]
fn equipment_check_covers_every_plan_workout() {
    let mut p = parse_plan(PLAN.into()).unwrap();
    p.weeks[1].workouts[0].blocks[0].sets[0].equipment = vec![Equipment::Fins];
    let import = || Import::Plan { plan: p.clone() };
    // The plan already uses kickboard and pull buoy in week 1.
    let all = vec![Equipment::Fins, Equipment::Kickboard, Equipment::PullBuoy];
    assert!(check_equipment(import(), all).is_ok());
    assert_eq!(
        gear_error(import(), vec![Equipment::Kickboard, Equipment::PullBuoy]),
        "week 2, workout 1: block 'Warm-up': uses fins, allowed: kickboard, pull buoy"
    );

    // A drill in the first week's first workout is reported there.
    let mut p = parse_plan(PLAN.into()).unwrap();
    p.weeks[0].workouts[0].blocks[0].sets[0].drill = Some("kick-race".into());
    let msg = gear_error(Import::Plan { plan: p.clone() }, vec![Equipment::Fins]);
    assert!(msg.starts_with("week 1, workout 1: block"), "{msg}");
    assert!(msg.ends_with("uses kickboard, allowed: fins"), "{msg}");
}
