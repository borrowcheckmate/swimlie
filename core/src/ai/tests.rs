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
