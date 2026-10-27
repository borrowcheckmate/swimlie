//! Remote AI for the swim creator: one OpenAI-compatible chat call that fills the form as JSON.
//! The Swift side decodes the returned JSON into its choices struct and applies it to the form.

use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;
use std::time::Duration;

const SYSTEM_PROMPT: &str = "You fill in a swim workout form from the swimmer's request. Reply with one JSON object only, \
with exactly these keys: isPlan (bool, true for a multi-week plan), poolLength (25 or 50), equipment (array, only from: \
Fins, Paddles, Pull buoy, Kickboard, Snorkel), startingBlocks (bool), byTime (bool, true for time, false for distance), \
distanceM (500 to 5000 in steps of 100), minutes (15 to 120 in steps of 5), restPercent (-50 to 50 in steps of 10), \
strokes (array, only from: Freestyle, Backstroke, Breaststroke, Butterfly, IM; empty lets the coach choose), \
kind (one of: easy recovery, endurance, threshold, speed, technique, mixed), weeks (2 to 16), swimsPerWeek (1 to 6), \
notes (short phrase), coachNote (2 to 3 plain sentences: what this swim is, why it fits what the swimmer asked for or \
their state, and one tip; no medical advice; only about the swim being created). The form already has these values: \
<current>. Copy every value the request does not mention unchanged, and change only what the request asks for.";

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AiError {
    #[error("network error: {msg}")]
    Network { msg: String },
    #[error("the AI service answered with an error: {msg}")]
    Http { msg: String },
    #[error("unexpected AI reply: {msg}")]
    Reply { msg: String },
}

/// Builds the chat/completions request body. Pure, so it can be tested without a network.
#[must_use]
pub fn request_body(model: &str, text: &str, current: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "temperature": 0,
        "top_p": 1,
        "max_tokens": 6000,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT.replace("<current>", current) },
            { "role": "user", "content": text },
        ],
    })
}

/// The reply text of a chat/completions body. Some models answer with a list of parts instead of a string, and
/// reasoning models can spend the whole token budget thinking, so the error says why the text is missing.
fn message_text(value: &serde_json::Value) -> Result<String, AiError> {
    let text = match value.pointer("/choices/0/message/content") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    };
    if text.trim().is_empty() {
        let finish = value
            .pointer("/choices/0/finish_reason")
            .and_then(|f| f.as_str())
            .unwrap_or("unknown");
        return Err(AiError::Reply {
            msg: format!("no message content (finish reason: {finish})"),
        });
    }
    Ok(text)
}

/// Pulls the JSON object text out of a chat/completions response body.
pub fn extract_choices(body: &str) -> Result<String, AiError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AiError::Reply { msg: e.to_string() })?;
    let content = message_text(&value)?;
    let parsed: serde_json::Value =
        serde_json::from_str(content.trim()).map_err(|e| AiError::Reply { msg: e.to_string() })?;
    if !parsed.is_object() {
        return Err(AiError::Reply {
            msg: "reply is not a JSON object".into(),
        });
    }
    Ok(parsed.to_string())
}

/// Asks an OpenAI-compatible endpoint (`{endpoint}/chat/completions`) to fill the form. Returns the JSON object text.
///
/// The API key is only sent as the Bearer header and is never logged or returned in errors.
#[uniffi::export]
pub fn ai_choices(
    endpoint: String,
    api_key: String,
    model: String,
    text: String,
    current: String,
) -> Result<String, AiError> {
    let url = format!("{}/chat/completions", endpoint.trim().trim_end_matches('/'));
    let body = request_body(&model, &text, &current).to_string();
    extract_choices(&post(&url, &api_key, &body)?)
}

const REVIEW_PROMPT: &str = "You are a warm, encouraging swim coach reviewing one swim for the swimmer. You get a brief of the \
swim: the numbers only. Reply in plain text, no markdown symbols (no **, #, bullets or tables). Write 150 to 200 words \
in five sections, each starting with its label on its own line, in this order: Summary, Achievements, Progress, \
What to improve, Next time. Open the Summary with the strongest genuine achievement from the brief. Be specific and \
encouraging. Use ONLY numbers that appear in the brief. A set marked cut short was not completed: never call it \
complete. If the brief says there are no previous swims, write that Progress will show after more swims. If the \
brief has no achievements, say so plainly and do not invent any. End Next time with one concrete suggestion for the \
next session. Give no medical advice.";

/// The built-in review prompt, shown in Settings as the starting point for the swimmer's own.
#[uniffi::export]
#[must_use]
pub fn default_review_prompt() -> String {
    REVIEW_PROMPT.to_string()
}

/// Builds the review request body (plain text, no JSON mode). Pure, so it can be tested without a network.
#[must_use]
pub fn review_body(model: &str, brief: &str, prompt: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "temperature": 0,
        "top_p": 1,
        "max_tokens": 4000,
        "messages": [
            { "role": "system", "content": if prompt.trim().is_empty() { REVIEW_PROMPT } else { prompt } },
            { "role": "user", "content": brief },
        ],
    })
}

/// Pulls the message text out of a chat/completions response body.
pub fn extract_review(body: &str) -> Result<String, AiError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AiError::Reply { msg: e.to_string() })?;
    Ok(message_text(&value)?.trim().to_string())
}

/// Asks an OpenAI-compatible endpoint (`{endpoint}/chat/completions`) for a short review of a swim brief.
/// The API key is only sent as the Bearer header and is never logged or returned in errors.
#[uniffi::export]
pub fn ai_review(
    endpoint: String,
    api_key: String,
    model: String,
    brief: String,
    prompt: String,
) -> Result<String, AiError> {
    let url = format!("{}/chat/completions", endpoint.trim().trim_end_matches('/'));
    let body = review_body(&model, &brief, &prompt).to_string();
    extract_review(&post(&url, &api_key, &body)?)
}

/// Receives the review text as it grows, one call per chunk with everything so far.
#[uniffi::export(with_foreign)]
pub trait ReviewSink: Send + Sync {
    fn on_text(&self, text_so_far: String);
}

/// One line of a chat/completions SSE stream.
#[derive(Debug, PartialEq, Eq)]
pub enum SseLine {
    /// Blank, comment or other non-data line.
    Skip,
    /// `data: [DONE]`, the end of the stream.
    Done,
    /// A chunk: the text it adds (empty if none) and its finish reason, when it has one.
    Chunk {
        text: String,
        finish: Option<String>,
    },
}

/// Parses one SSE line. Pure, so it can be tested without a network.
pub fn parse_sse_line(line: &str) -> Result<SseLine, AiError> {
    let Some(data) = line.trim_end().strip_prefix("data:") else {
        return Ok(SseLine::Skip);
    };
    let data = data.trim_start();
    if data == "[DONE]" {
        return Ok(SseLine::Done);
    }
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| AiError::Reply { msg: e.to_string() })?;
    if let Some(err) = value.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .map_or_else(|| err.to_string(), String::from);
        return Err(AiError::Http {
            msg: msg.chars().take(300).collect(),
        });
    }
    let text = match value.pointer("/choices/0/delta/content") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect(),
        _ => String::new(),
    };
    let finish = value
        .pointer("/choices/0/finish_reason")
        .and_then(|f| f.as_str())
        .map(String::from);
    Ok(SseLine::Chunk { text, finish })
}

/// Streaming variant of `ai_review`: calls `sink` with the text so far after each chunk and returns the final text.
#[uniffi::export]
#[expect(
    clippy::too_many_arguments,
    reason = "UniFFI export: the signature is the Swift API"
)]
pub fn ai_review_stream(
    endpoint: String,
    api_key: String,
    model: String,
    brief: String,
    prompt: String,
    sink: Arc<dyn ReviewSink>,
) -> Result<String, AiError> {
    let url = format!("{}/chat/completions", endpoint.trim().trim_end_matches('/'));
    let mut body = review_body(&model, &brief, &prompt);
    body["stream"] = serde_json::Value::Bool(true);
    let mut lines = BufReader::new(open_stream(&url, &api_key, &body.to_string())?);
    let mut text = String::new();
    let mut finish: Option<String> = None;
    let mut line = String::new();
    loop {
        line.clear();
        if lines
            .read_line(&mut line)
            .map_err(|e| AiError::Network { msg: e.to_string() })?
            == 0
        {
            break;
        }
        match parse_sse_line(&line)? {
            SseLine::Skip => {}
            SseLine::Done => break,
            SseLine::Chunk {
                text: delta,
                finish: reason,
            } => {
                if reason.is_some() {
                    finish = reason;
                }
                if !delta.is_empty() {
                    text.push_str(&delta);
                    sink.on_text(text.clone());
                }
            }
        }
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(AiError::Reply {
            msg: format!(
                "no message content (finish reason: {})",
                finish.as_deref().unwrap_or("unknown")
            ),
        });
    }
    Ok(text)
}

#[cfg(not(target_os = "watchos"))]
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .http_status_as_error(false)
        .build()
        .into()
}

#[cfg(not(target_os = "watchos"))]
fn post(url: &str, api_key: &str, body: &str) -> Result<String, AiError> {
    let mut response = agent()
        .post(url)
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send(body)
        .map_err(|e| AiError::Network { msg: e.to_string() })?;
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| AiError::Network { msg: e.to_string() })?;
    if status >= 400 {
        // The provider's own message (e.g. "Invalid model") says what to fix; it never contains our key.
        let detail: String = text.chars().take(300).collect();
        return Err(AiError::Http {
            msg: format!("HTTP {status}: {detail}"),
        });
    }
    Ok(text)
}

/// Sends the request and returns the body as a reader, so the caller can read it as it arrives.
#[cfg(not(target_os = "watchos"))]
fn open_stream(url: &str, api_key: &str, body: &str) -> Result<Box<dyn Read>, AiError> {
    let mut response = agent()
        .post(url)
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send(body)
        .map_err(|e| AiError::Network { msg: e.to_string() })?;
    let status = response.status().as_u16();
    if status >= 400 {
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| AiError::Network { msg: e.to_string() })?;
        let detail: String = text.chars().take(300).collect();
        return Err(AiError::Http {
            msg: format!("HTTP {status}: {detail}"),
        });
    }
    Ok(Box::new(response.into_body().into_reader()))
}

/// The watch has no AI; this keeps the symbol in the shared bindings without pulling in TLS for watchOS.
#[cfg(target_os = "watchos")]
fn post(_url: &str, _api_key: &str, _body: &str) -> Result<String, AiError> {
    Err(AiError::Network {
        msg: "AI is not available on Apple Watch".into(),
    })
}

#[cfg(target_os = "watchos")]
fn open_stream(_url: &str, _api_key: &str, _body: &str) -> Result<Box<dyn Read>, AiError> {
    Err(AiError::Network {
        msg: "AI is not available on Apple Watch".into(),
    })
}

#[cfg(test)]
mod tests;
