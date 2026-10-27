//! Coach text in the creator's language. English is the in-code source and the fallback: the key is the
//! English text. Dutch lives in `core/locales/nl.json`, keyed the same way (drills: `drill.<id>.<field>`).

use std::collections::HashMap;
use std::sync::OnceLock;

const NL_JSON: &str = include_str!("../locales/nl.json");

/// True for Dutch (`nl`, any case, surrounding spaces ignored).
pub(crate) fn is_dutch(lang: &str) -> bool {
    lang.trim().eq_ignore_ascii_case("nl")
}

fn nl() -> &'static HashMap<String, String> {
    static NL: OnceLock<HashMap<String, String>> = OnceLock::new();
    NL.get_or_init(|| serde_json::from_str(NL_JSON).unwrap_or_default())
}

/// The text of an English `key` in `lang`; the key itself when there is no translation.
pub(crate) fn tr(lang: &str, key: &'static str) -> &'static str {
    if !is_dutch(lang) {
        return key;
    }
    nl().get(key).map_or(key, String::as_str)
}

/// Name, cue and how-to of a catalogue drill in `lang`, English when untranslated.
pub(crate) fn drill(
    id: &str,
    lang: &str,
    en: (&'static str, &'static str, &'static str),
) -> (&'static str, &'static str, &'static str) {
    if !is_dutch(lang) {
        return en;
    }
    let get = |field: &str, fallback: &'static str| {
        nl().get(&format!("drill.{id}.{field}"))
            .map_or(fallback, String::as_str)
    };
    let (name, cue, how_to) = en;
    let name = get("name", name);
    (name, get("cue", cue), get("how_to", how_to))
}

/// Every English key `nl.json` may translate (drill keys are built from the catalogue ids).
#[cfg(test)]
fn english_keys() -> Vec<String> {
    let mut keys: Vec<String> = [
        "Warm-up",
        "Main",
        "Cool-down",
        "Easy recovery",
        "Endurance",
        "Threshold",
        "Speed",
        "Technique",
        "Mixed",
        "Ladder",
        "Pyramid",
        "Descend",
        "Fast-slow-fast",
        "Long to short",
        "Stroke round",
        "Base",
        "Lighter week",
        "Build",
        "Build each 50",
        "Count strokes",
        "Kick on your back",
        " (default pace)",
        "No CSS yet: paces assume 2:00 /100 m. Swim the CSS test to set yours.",
        "{weeks}-week {kind} plan{default}",
    ]
    .map(String::from)
    .to_vec();
    for id in crate::exercises::exercise_ids() {
        for field in ["name", "cue", "how_to"] {
            keys.push(format!("drill.{id}.{field}"));
        }
    }
    keys.extend(
        crate::exercises::exercises()
            .into_iter()
            .filter_map(|e| e.safety),
    );
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_the_key() {
        assert_eq!(tr("en", "Warm-up"), "Warm-up");
        assert_eq!(tr("xx", "Main"), "Main");
        assert_eq!(drill("catch-up", "en", ("a", "b", "c")), ("a", "b", "c"));
    }

    #[test]
    fn dutch_translates_and_unknown_keys_fall_back() {
        assert_eq!(tr("nl", "Endurance"), "Duurtraining");
        assert_eq!(tr(" NL ", "Main"), "Hoofdset");
        assert_eq!(tr("nl", "Not a key"), "Not a key");
    }

    #[test]
    fn every_dutch_key_is_a_known_english_key() {
        let known = english_keys();
        for key in nl().keys() {
            assert!(
                known.contains(key),
                "nl.json key matches no English key: {key}"
            );
        }
    }

    #[test]
    fn dutch_drill_cues_fit_the_watch() {
        for e in crate::exercises::exercises() {
            let x = crate::exercises::exercise_in(e.id.clone(), "nl".into()).expect("exercise");
            assert!(!x.name.is_empty(), "{}", e.id);
            assert!(x.cue.chars().count() <= 40, "{}: {}", e.id, x.cue);
        }
    }
}
