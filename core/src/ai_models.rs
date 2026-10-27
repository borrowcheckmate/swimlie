//! Lists the models a provider offers (GET {endpoint}/models), so the Settings picker shows real names.

use crate::ai::AiError;
use std::time::Duration;

/// Fetches the model ids from an OpenAI-compatible `/models` endpoint, sorted.
#[uniffi::export]
pub fn ai_models(endpoint: String, api_key: String) -> Result<Vec<String>, AiError> {
    let url = format!("{}/models", endpoint.trim_end_matches('/'));
    parse_models(&get(&url, &api_key)?)
}

/// Ids that name a non-chat model (embeddings, speech, images, moderation, ...). Case-insensitive.
const NON_CHAT: [&str; 11] = [
    "embed",
    "moderation",
    "whisper",
    "tts",
    "dall-e",
    "image",
    "audio",
    "transcribe",
    "ocr",
    "realtime",
    "guard",
];

/// True when the model id looks like a chat model. Pure, so it can be tested without a network.
#[must_use]
pub fn is_chat_model(id: &str) -> bool {
    let lower = id.to_lowercase();
    !NON_CHAT.iter().any(|bad| lower.contains(bad))
}

/// Reads `data[].id` from the reply, keeping chat models only. Pure, so it can be tested without a network.
pub fn parse_models(body: &str) -> Result<Vec<String>, AiError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AiError::Reply { msg: e.to_string() })?;
    let data = value["data"].as_array().ok_or_else(|| AiError::Reply {
        msg: "no data list".into(),
    })?;
    let mut ids: Vec<String> = data
        .iter()
        .filter_map(|m| {
            m["id"]
                .as_str()
                .filter(|id| is_chat_model(id))
                .map(str::to_string)
        })
        .collect();
    if ids.is_empty() {
        return Err(AiError::Reply {
            msg: "no models listed".into(),
        });
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[cfg(not(target_os = "watchos"))]
fn get(url: &str, api_key: &str) -> Result<String, AiError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent.get(url);
    // Some providers require the key; the public ones answer without it, so send it only when we have one.
    if !api_key.is_empty() {
        request = request.header("Authorization", &format!("Bearer {api_key}"));
    }
    let mut response = request
        .call()
        .map_err(|e| AiError::Network { msg: e.to_string() })?;
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| AiError::Network { msg: e.to_string() })?;
    if status >= 400 {
        // The provider's own message says what to fix; it never contains our key.
        let detail: String = text.chars().take(300).collect();
        return Err(AiError::Http {
            msg: format!("HTTP {status}: {detail}"),
        });
    }
    Ok(text)
}

#[cfg(target_os = "watchos")]
fn get(_url: &str, _api_key: &str) -> Result<String, AiError> {
    Err(AiError::Network {
        msg: "AI is not available on Apple Watch".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_sorts_model_ids() {
        let body = r#"{"object":"list","data":[{"id":"mistral-small-latest"},{"id":"mistral-large-latest"},{"id":"mistral-large-latest"}]}"#;
        assert_eq!(
            parse_models(body).unwrap(),
            vec!["mistral-large-latest", "mistral-small-latest"]
        );
    }

    #[test]
    fn drops_non_chat_models() {
        let body = r#"{"data":[{"id":"gpt-5"},{"id":"text-embedding-3-small"},{"id":"Omni-MODERATION-latest"},{"id":"whisper-1"},{"id":"dall-e-3"},{"id":"llama-guard-4"},{"id":"gpt-4o-realtime"},{"id":"tts-1"}]}"#;
        assert_eq!(parse_models(body).unwrap(), vec!["gpt-5"]);
    }

    #[test]
    fn rejects_bad_model_lists() {
        assert!(matches!(
            parse_models("not json"),
            Err(AiError::Reply { .. })
        ));
        assert!(matches!(
            parse_models(r#"{"error":"bad key"}"#),
            Err(AiError::Reply { .. })
        ));
        assert!(matches!(
            parse_models(r#"{"data":[]}"#),
            Err(AiError::Reply { .. })
        ));
    }
}
