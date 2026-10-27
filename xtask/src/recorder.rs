use std::fmt::Write as _;

use super::{
    Engine, Lap, StoredSession, Stroke, SwimLevel, SwimmerProfile, Value, Workout, fs,
    session_review_brief,
};

/// A number from a JSON value; numbers may also arrive as strings, anything else counts as 0.
pub(crate) fn num(v: &Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0.0)
}

/// The planned workout of a recorder "workout" line, one line per set, ahead of the measured laps.
pub(crate) fn planned_summary(e: &Value) -> String {
    let w = &e["workout"];
    let mut out = format!(
        "planned: {} (id {}, pool {} m)\n",
        w["name"].as_str().unwrap_or("?"),
        e["id"].as_str().unwrap_or(""),
        num(&w["pool_length_m"])
    );
    for b in w["blocks"].as_array().into_iter().flatten() {
        for s in b["sets"].as_array().into_iter().flatten() {
            let _ = writeln!(out, "  {s}");
        }
    }
    out + "measured:\n"
}

/// Formats the `HealthKit` laps and rests of a recorder .ndjson log. Lines mentioning "motion" are skipped.
pub(crate) fn swim_laps(text: &str) -> Result<String, String> {
    let mut t0 = 0.0;
    let mut laps = Vec::new();
    let mut other = Vec::new();
    let mut planned = String::new();
    for line in text.lines() {
        if line.contains("\"motion\"") {
            continue;
        }
        let e: Value =
            serde_json::from_str(line).map_err(|err| format!("bad line `{line}`: {err}"))?;
        match (e["kind"].as_str(), e["type_name"].as_str()) {
            (Some("clock"), _) => t0 = num(&e["wall"]),
            (Some("workout"), _) => planned = planned_summary(&e),
            (Some("event"), Some("lap")) => laps.push(e),
            (Some("event"), _) => other.push(e),
            _ => {}
        }
    }
    laps.sort_by(|a, b| num(&a["start"]).total_cmp(&num(&b["start"])));

    let mut out = planned;
    let mut prev = 0.0;
    for (i, e) in laps.iter().enumerate() {
        let (start, end) = (num(&e["start"]), num(&e["end"]));
        let gap = if prev == 0.0 { 0.0 } else { start - prev };
        if gap > 8.0 {
            let _ = writeln!(out, "   --- rest {gap:5.0}s");
        }
        let style: String = e["stroke_style_name"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(6)
            .collect();
        let swolf = num(&e["metadata"]["HKSWOLFScore"]);
        let delay = num(&e["received_at"]) - end;
        let _ = writeln!(
            out,
            "{:3} {:7.0} {:5.1}s {style:6} swolf {swolf:4.0} delay {delay:5.1}",
            i + 1,
            start - t0,
            end - start,
        );
        prev = end;
    }
    for e in other.iter().filter(|e| e["type_name"] != "segment") {
        let name = e["type_name"].as_str().unwrap_or("");
        // Python's round() rounds half to even, and so does round_ties_even.
        let at = (num(&e["start"]) - t0).round_ties_even() as i64;
        let _ = writeln!(out, "{name} {at}");
    }
    Ok(out)
}

pub(crate) fn laps(file: &str) -> Result<(), String> {
    let text = fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
    print!("{}", swim_laps(&text)?);
    Ok(())
}

/// The stroke of a `HealthKit` `stroke_style_name`.
pub(crate) fn stroke_of(name: &str) -> Stroke {
    match name.to_lowercase().as_str() {
        "freestyle" => Stroke::Freestyle,
        "backstroke" => Stroke::Backstroke,
        "breaststroke" => Stroke::Breaststroke,
        "butterfly" => Stroke::Butterfly,
        "mixed" => Stroke::Mixed,
        "kickboard" => Stroke::Kickboard,
        _ => Stroke::Any,
    }
}

/// The AI review brief of a recorder log. The session is rebuilt with the same engine the watch uses, so the
/// text is the exact model input for that swim. Without a CSS or earlier swims, the brief has no comparison.
pub(crate) fn review_brief(text: &str) -> Result<String, String> {
    let mut t0 = 0.0;
    let mut workout = None;
    let mut laps = Vec::new();
    for line in text.lines() {
        if line.contains("\"motion\"") {
            continue;
        }
        let e: Value =
            serde_json::from_str(line).map_err(|err| format!("bad line `{line}`: {err}"))?;
        match (e["kind"].as_str(), e["type_name"].as_str()) {
            (Some("clock"), _) => t0 = num(&e["wall"]),
            (Some("workout"), _) => workout = Some(e),
            (Some("event"), Some("lap")) => laps.push(e),
            _ => {}
        }
    }
    let w = workout.ok_or("no workout line in the log")?;
    let plan: Workout =
        serde_json::from_value(w["workout"].clone()).map_err(|e| format!("bad workout: {e}"))?;
    let id = w["id"].as_str().map(str::to_string);
    let mut engine =
        Engine::new(plan, id, (t0 * 1000.0) as i64, vec![]).map_err(|e| format!("{e:?}"))?;
    laps.sort_by(|a, b| num(&a["start"]).total_cmp(&num(&b["start"])));
    for e in &laps {
        let (start_s, end_s) = (num(&e["start"]) - t0, num(&e["end"]) - t0);
        let lap = Lap {
            start_s,
            end_s,
            stroke: stroke_of(e["stroke_style_name"].as_str().unwrap_or("")),
            strokes: None,
        };
        engine.on_lap(lap, (num(&e["received_at"]) - t0).max(end_s));
    }
    let stored = StoredSession {
        id: String::new(),
        created_at_ms: 0,
        updated_at_ms: 0,
        session: engine.session(),
    };
    let profile = SwimmerProfile {
        css_s_per_100m: None,
        sessions_28d: 0,
        weekly_m_28d: 0.0,
        rep_times: vec![],
        age_years: None,
        max_hr_bpm: None,
        level: SwimLevel::default(),
    };
    Ok(session_review_brief(stored, profile, vec![]))
}

pub(crate) fn brief(file: &str) -> Result<(), String> {
    let text = fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
    println!("{}", review_brief(&text)?);
    Ok(())
}
