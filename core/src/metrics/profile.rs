use super::{
    BTreeMap, Equipment, StoredSession, Stroke, StrokeTime, hr_stats, reps, session_summary,
};

/// Rep paces per stroke and distance, keyed by `(stroke order, distance)` for the profile order.
type StrokeGroups = BTreeMap<(u32, u32), (Stroke, Vec<f64>)>;

/// How well the swimmer says they swim (self-reported in Settings). Used when there is no usable CSS (PLAN.md 4b).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
pub enum SwimLevel {
    /// Just learning.
    Beginner,
    /// Can swim 200 m non-stop.
    #[default]
    Intermediate,
    /// Regular swimmer.
    Advanced,
}

impl SwimLevel {
    /// What the level means for a workout, so the AI does not have to guess from the word.
    pub(super) const fn guidance(self) -> &'static str {
        match self {
            Self::Beginner => {
                "beginner: no CSS means about 4:25 /100 m freestyle. Keep the total distance modest (a 40 min swim is about 800 m), reps short (25 to 100 m), rests long (at least 30 s, about 1.5 times the swim time of a rep), mostly easy and moderate, and no hard or sprint main sets."
            }
            Self::Intermediate => {
                "intermediate: no CSS means about 2:50 /100 m freestyle. A 40 min swim is about 1200 m."
            }
            Self::Advanced => {
                "advanced: no CSS means about 2:18 /100 m freestyle. A 40 min swim is about 1400 m."
            }
        }
    }

    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Beginner => "beginner",
            Self::Intermediate => "intermediate",
            Self::Advanced => "advanced",
        }
    }
}

/// What the swimmer's history says about them, for the AI prompt. Derived from sessions, not entered.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SwimmerProfile {
    pub css_s_per_100m: Option<f64>,
    /// Sessions started in the 28 days up to now.
    pub sessions_28d: u32,
    /// Distance of those sessions / 4.
    pub weekly_m_28d: f64,
    /// Complete reps of the last 90 days, sorted by stroke then distance.
    pub rep_times: Vec<StrokeTime>,
    /// From the newest session that has one.
    pub age_years: Option<u32>,
    /// Highest heart-rate sample of the last 90 days.
    pub max_hr_bpm: Option<f64>,
    /// Self-reported level, from Settings.
    pub level: SwimLevel,
}

/// Profile of the swimmer from their stored sessions. Sessions after `now_ms` are ignored.
#[uniffi::export]
#[must_use]
pub fn swimmer_profile(
    sessions: Vec<StoredSession>,
    css_s_per_100m: Option<f64>,
    now_ms: i64,
    level: SwimLevel,
) -> SwimmerProfile {
    const DAY_MS: i64 = 86_400_000;
    let recent = |days: i64| {
        sessions
            .iter()
            .filter(move |st| (now_ms - days * DAY_MS..=now_ms).contains(&st.session.started_at_ms))
    };
    let last_28: Vec<&StoredSession> = recent(28).collect();
    let distance_28: u32 = last_28
        .iter()
        .map(|st| session_summary(&st.session, None).distance_m)
        .sum();

    // Keyed by (stroke, distance) so the map iterates in the order the profile needs.
    let mut groups: StrokeGroups = BTreeMap::new();
    for st in recent(90) {
        let s = &st.session;
        for r in reps(s) {
            let Some(set) = s
                .workout
                .blocks
                .get(r.pos.block as usize)
                .and_then(|b| b.sets.get(r.pos.set as usize))
            else {
                continue;
            };
            // Same notion of complete as `personal_records`: no merged lap, all lengths swum, a planned stroke.
            let merged = s
                .lap_lengths
                .iter()
                .zip(&s.lap_positions)
                .any(|(&n, &p)| p == r.pos && n != 1);
            if set.stroke == Stroke::Any
                || merged
                || r.lengths != set.distance_m / s.workout.pool_length_m
            {
                continue;
            }
            groups
                .entry((set.stroke as u32, set.distance_m))
                .or_insert_with(|| (set.stroke, vec![]))
                .1
                .push(r.swim_s);
        }
    }
    let rep_times = groups
        .into_iter()
        .map(|((_, distance_m), (stroke, mut times))| {
            times.sort_by(f64::total_cmp);
            let n = times.len();
            let median_s = if n % 2 == 1 {
                times[n / 2]
            } else {
                f64::midpoint(times[n / 2 - 1], times[n / 2])
            };
            StrokeTime {
                stroke,
                distance_m,
                median_s,
                count: n as u32,
            }
        })
        .collect();

    SwimmerProfile {
        css_s_per_100m,
        sessions_28d: last_28.len() as u32,
        weekly_m_28d: f64::from(distance_28) / 4.0,
        rep_times,
        age_years: sessions
            .iter()
            .filter(|st| st.session.age_years.is_some())
            .max_by_key(|st| st.session.started_at_ms)
            .and_then(|st| st.session.age_years),
        max_hr_bpm: hr_stats(recent(90).flat_map(|st| st.session.hr.iter().map(|h| h.bpm))).1,
        level,
    }
}

/// What the creator form asks for: the prompt's "My request" section.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CreatorInputs {
    pub pool_length_m: u32,
    pub equipment: Vec<Equipment>,
    /// Dives from the blocks are allowed (many public pools forbid them).
    pub starting_blocks: bool,
    pub target_distance_m: Option<u32>,
    pub target_time_min: Option<u32>,
    /// Free text such as "threshold" or "easy recovery".
    pub kind: String,
    pub notes: String,
    /// Set when the creator asks for a multi-week plan.
    pub plan_weeks: Option<u32>,
    pub swims_per_week: Option<u32>,
    /// Rest between reps, as a percentage change: 50 is half as long again, -50 half as long. Single workouts only.
    pub rest_percent: i32,
    /// Strokes the main sets are spread over; empty lets the coach choose. Single workouts only.
    pub strokes: Vec<Stroke>,
    /// Language of the coach's text: "en" (default) or "nl". Unknown values give English.
    pub language: String,
}

/// A schema name with underscores as spaces, e.g. `pull buoy` for `PullBuoy`.
pub fn schema_name(v: &impl serde::Serialize) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|j| j.as_str().map(|s| s.replace('_', " ")))
        .unwrap_or_default()
}

/// `m:ss`, rounded to the second.
#[must_use]
pub fn mmss(s: f64) -> String {
    let t = s.round() as u64;
    format!("{}:{:02}", t / 60, t % 60)
}

/// The "My request" and "About me" sections of the AI prompt.
#[uniffi::export]
#[expect(
    clippy::too_many_lines,
    reason = "one linear pipeline whose steps share local state; splitting it is pending"
)]
pub fn creator_brief(inputs: CreatorInputs, profile: SwimmerProfile) -> String {
    let gear = if inputs.equipment.is_empty() {
        "none".to_string()
    } else {
        inputs
            .equipment
            .iter()
            .map(schema_name)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut request = vec!["My request:".to_string()];
    if let Some(w) = inputs.plan_weeks {
        let n = inputs.swims_per_week.unwrap_or(3);
        request.push(format!(
            "- Plan: {w} weeks, {n} swims per week. Reply with a plan (version, name, weeks; each week has a focus and its workouts), not a single workout."
        ));
    }
    request.push(format!("- Pool: {} m", inputs.pool_length_m));
    request.push(format!("- Type: {}", inputs.kind));
    let (distance, time) = if inputs.plan_weeks.is_some() {
        ("Target per swim", "Target time per swim")
    } else {
        ("Target", "Target time")
    };
    if let Some(d) = inputs.target_distance_m {
        request.push(format!("- {distance}: {d} m"));
    }
    if let Some(t) = inputs.target_time_min {
        request.push(format!("- {time}: {t} min"));
    }
    request.push(format!("- Equipment: {gear}"));
    request.push("- Only use this equipment; none means no equipment at all.".to_string());
    let blocks = if inputs.starting_blocks {
        "yes, dives allowed"
    } else {
        "no dives"
    };
    request.push(format!("- Starting blocks: {blocks}"));
    if !inputs.notes.trim().is_empty() {
        request.push(format!("- Notes: {}", inputs.notes.trim()));
    }

    let p = &profile;
    let mut about = vec![
        "About me (from my swim history, use it to pick paces, goal_s and rests):".to_string(),
    ];
    about.push(format!("- Level {}", p.level.guidance()));
    if p.css_s_per_100m.is_none()
        && p.sessions_28d == 0
        && p.rep_times.is_empty()
        && p.age_years.is_none()
        && p.max_hr_bpm.is_none()
    {
        about.push("- No swim history yet: base everything on my level.".to_string());
    } else {
        if let Some(css) = p.css_s_per_100m {
            about.push(format!("- CSS pace: {} /100 m", mmss(css)));
        }
        let swims = if p.sessions_28d == 1 { "swim" } else { "swims" };
        about.push(format!(
            "- Last 4 weeks: {} {swims}, about {:.0} m per week",
            p.sessions_28d, p.weekly_m_28d
        ));
        if !p.rep_times.is_empty() {
            let typical: Vec<String> = p
                .rep_times
                .iter()
                .map(|r| {
                    format!(
                        "{} {} m {} ({} reps)",
                        schema_name(&r.stroke),
                        r.distance_m,
                        mmss(r.median_s),
                        r.count
                    )
                })
                .collect();
            about.push(format!("- Typical rep times: {}", typical.join(", ")));
        }
        let mut me = vec![];
        if let Some(age) = p.age_years {
            me.push(format!("Age {age}"));
        }
        if let Some(hr) = p.max_hr_bpm {
            me.push(format!("max heart rate seen {hr:.0} bpm"));
        }
        if !me.is_empty() {
            about.push(format!("- {}", me.join(", ")));
        }
    }
    format!("{}\n\n{}\n", request.join("\n"), about.join("\n"))
}
