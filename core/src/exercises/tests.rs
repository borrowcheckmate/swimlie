use super::*;

#[test]
fn ids_are_unique_kebab_case() {
    let mut ids: Vec<&str> = exercise_ids().collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count);
    for id in exercise_ids() {
        assert!(
            !id.is_empty() && !id.starts_with('-') && !id.ends_with('-') && !id.contains("--"),
            "{id}"
        );
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{id}"
        );
    }
}

#[test]
fn every_kind_has_an_entry() {
    for kind in [
        ExerciseKind::Drill,
        ExerciseKind::Wall,
        ExerciseKind::Start,
        ExerciseKind::Fun,
    ] {
        assert!(exercises().iter().any(|e| e.kind == kind), "{kind:?}");
    }
}

#[test]
fn lookup_by_id() {
    assert_eq!(
        exercise("catch-up".into()).map(|e| e.name),
        Some("Catch-up drill".into())
    );
    assert_eq!(exercise("no-such-drill".into()), None);
}

#[test]
fn video_urls_are_search_links_without_spaces() {
    for e in exercises() {
        assert!(e.video_url.starts_with(SEARCH_URL), "{}", e.id);
        assert!(!e.video_url.contains(' '), "{}", e.id);
    }
    for e in CATALOGUE {
        assert!(
            e.search
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == ' '),
            "{}",
            e.id
        );
    }
}

#[test]
fn breath_holds_have_safety() {
    for id in ["touch-the-floor", "breathe-every-5th"] {
        assert_eq!(
            exercise(id.into()).and_then(|e| e.safety),
            Some(BREATH_HOLD.into()),
            "{id}"
        );
    }
}

#[test]
fn dive_has_safety_and_deep_cue() {
    let e = exercise("dive-and-swim".into()).expect("dive-and-swim");
    assert!(e.safety.is_some());
    assert!(e.cue.contains("Deep"));
}

#[test]
fn cues_fit_the_watch() {
    for e in exercises() {
        assert!(e.cue.chars().count() <= 40, "{}", e.id);
    }
}

#[test]
fn rep_distances_are_positive() {
    assert!(exercises().iter().all(|e| e.rep_m > 0));
}

#[test]
fn prompt_lists_every_id() {
    let prompt = include_str!("../../../schema/prompt.md");
    for id in exercise_ids() {
        assert!(prompt.contains(&format!("`{id}`")), "{id}");
    }
}
