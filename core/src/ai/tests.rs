use crate::model::parse_import;

#[test]
fn review_uses_custom_prompt_or_default() {
    assert_eq!(
        review_body("m", "b", "Be brief.")["messages"][0]["content"],
        "Be brief."
    );
    assert_eq!(
        review_body("m", "b", "  ")["messages"][0]["content"],
        default_review_prompt()
    );
}

#[test]
fn review_text_from_parts_and_empty_reason() {
    let parts = r#"{"choices":[{"message":{"content":[{"type":"text","text":"Hi "},{"type":"text","text":"there"}]}}]}"#;
    assert_eq!(extract_review(parts).unwrap(), "Hi there");
    let empty = r#"{"choices":[{"message":{"content":""},"finish_reason":"length"}]}"#;
    assert!(
        extract_review(empty)
            .unwrap_err()
            .to_string()
            .contains("finish reason: length")
    );
}

use super::*;

#[test]
fn request_has_model_json_mode_and_current_form() {
    let body = request_body("mistral-small", "easy 1500", "isPlan false, distanceM 1500");
    assert_eq!(body["model"], "mistral-small");
    assert_eq!(body["response_format"]["type"], "json_object");
    assert_eq!(body["messages"][1]["content"], "easy 1500");
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("isPlan false, distanceM 1500"));
    assert!(!system.contains("<current>"));
}

#[test]
fn prompt_asks_for_coach_note() {
    let body = request_body("mistral-small", "easy 1500", "isPlan false");
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("coachNote"));
}

#[test]
fn extracts_object_from_reply() {
    let reply =
        r#"{"choices":[{"message":{"content":"{\"isPlan\": false, \"distanceM\": 1500}"}}]}"#;
    let text = extract_choices(reply).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["distanceM"], 1500);
}

#[test]
fn review_request_is_plain_text_with_brief() {
    let body = review_body("mistral-small", "Total: 200 m", "");
    assert_eq!(body["model"], "mistral-small");
    assert!(body.get("response_format").is_none());
    assert_eq!(body["messages"][1]["content"], "Total: 200 m");
    let system = body["messages"][0]["content"].as_str().unwrap();
    for part in [
        "Summary",
        "Achievements",
        "Progress",
        "What to improve",
        "Next time",
    ] {
        assert!(system.contains(part));
    }
}

#[test]
fn extracts_review_text() {
    let reply =
        r#"{"choices":[{"message":{"content":"  Summary: fine.\nNext time: rest more.  "}}]}"#;
    assert_eq!(
        extract_review(reply).unwrap(),
        "Summary: fine.\nNext time: rest more."
    );
    assert!(matches!(
        extract_review(r#"{"choices":[{"message":{"content":" "}}]}"#),
        Err(AiError::Reply { .. })
    ));
    assert!(matches!(
        extract_review("not json"),
        Err(AiError::Reply { .. })
    ));
}

#[test]
fn parses_sse_string_and_parts_deltas() {
    let string = r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#;
    assert_eq!(
        parse_sse_line(string).unwrap(),
        SseLine::Chunk {
            text: "Hi".into(),
            finish: None
        }
    );
    let parts = r#"data: {"choices":[{"delta":{"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]}}]}"#;
    assert_eq!(
        parse_sse_line(parts).unwrap(),
        SseLine::Chunk {
            text: "ab".into(),
            finish: None
        }
    );
}

#[test]
fn parses_sse_done_finish_and_skips_other_lines() {
    assert_eq!(parse_sse_line("data: [DONE]\r").unwrap(), SseLine::Done);
    assert_eq!(parse_sse_line("").unwrap(), SseLine::Skip);
    assert_eq!(parse_sse_line(": keep-alive").unwrap(), SseLine::Skip);
    assert_eq!(parse_sse_line("event: message").unwrap(), SseLine::Skip);
    let last = r#"data: {"choices":[{"delta":{},"finish_reason":"length"}]}"#;
    assert_eq!(
        parse_sse_line(last).unwrap(),
        SseLine::Chunk {
            text: String::new(),
            finish: Some("length".into())
        }
    );
    assert!(matches!(
        parse_sse_line("data: {oops"),
        Err(AiError::Reply { .. })
    ));
    let err = r#"data: {"error":{"message":"Invalid model"}}"#;
    assert!(matches!(parse_sse_line(err), Err(AiError::Http { msg }) if msg == "Invalid model"));
}

#[test]
fn rejects_bad_replies() {
    assert!(matches!(
        extract_choices("not json"),
        Err(AiError::Reply { .. })
    ));
    assert!(matches!(
        extract_choices(r#"{"choices":[]}"#),
        Err(AiError::Reply { .. })
    ));
    let not_object = r#"{"choices":[{"message":{"content":"[1,2]"}}]}"#;
    assert!(matches!(
        extract_choices(not_object),
        Err(AiError::Reply { .. })
    ));
}

const WORKOUT_NO_REST: &str = r#"{"version":1,"name":"Mixed","pool_length_m":25,"blocks":[{"name":"Main","sets":[
    {"reps":4,"distance_m":100,"rest":{"type":"interval","seconds":120}},
    {"distance_m":200},
    {"distance_m":50,"rest":{"type":"fixed","seconds":30}}]}]}"#;

const PLAN_NO_REST: &str = r#"{"version":1,"name":"Two weeks","weeks":[
    {"focus":"Base","workouts":[{"version":1,"name":"A","pool_length_m":25,"blocks":[{"name":"Main","sets":[{"distance_m":100},{"distance_m":50,"rest":{"type":"fixed","seconds":10}}]}]}]},
    {"workouts":[{"version":1,"name":"B","pool_length_m":25,"blocks":[{"name":"Main","sets":[{"distance_m":200}]}]}]}]}"#;

#[test]
fn fill_missing_rest_fills_only_sets_without_one_in_a_workout() {
    let parsed = parse_import(WORKOUT_NO_REST.into()).unwrap();
    let Import::Workout { workout } = fill_missing_rest(parsed) else {
        panic!("expected a workout");
    };
    let rests: Vec<_> = workout.blocks[0]
        .sets
        .iter()
        .map(|s| s.rest.clone())
        .collect();
    assert_eq!(
        rests,
        vec![
            Some(Rest {
                kind: RestKind::Interval,
                seconds: 120
            }),
            Some(Rest {
                kind: RestKind::Fixed,
                seconds: DEFAULT_REST_S
            }),
            Some(Rest {
                kind: RestKind::Fixed,
                seconds: 30
            }),
        ]
    );
}

#[test]
fn fill_missing_rest_fills_every_workout_in_a_plan() {
    let parsed = parse_import(PLAN_NO_REST.into()).unwrap();
    let Import::Plan { plan } = fill_missing_rest(parsed) else {
        panic!("expected a plan");
    };
    let fixed_default = Some(Rest {
        kind: RestKind::Fixed,
        seconds: DEFAULT_REST_S,
    });
    let first = &plan.weeks[0].workouts[0].blocks[0].sets;
    assert_eq!(first[0].rest, fixed_default);
    assert_eq!(first[1].rest.as_ref().map(|r| r.seconds), Some(10));
    let second = &plan.weeks[1].workouts[0].blocks[0].sets;
    assert_eq!(second[0].rest, fixed_default);
    assert!(plan.validate().is_ok());
}

#[test]
fn normal_import_keeps_missing_rest_as_written() {
    let Import::Workout { workout } = parse_import(WORKOUT_NO_REST.into()).unwrap() else {
        panic!("expected a workout");
    };
    assert_eq!(workout.blocks[0].sets[1].rest, None);
}

#[test]
fn prompt_says_every_set_needs_a_rest() {
    let prompt = include_str!("../../../schema/prompt.md");
    assert!(prompt.contains("Every set needs a `rest`"));
    assert!(prompt.contains("warm-up, cool-down, kick and drill sets get `fixed` 15–20 s"));
}
