//! Workout format: the public JSON contract for imports, AI apps, MCP and the coach.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct Workout {
    pub version: u32,
    pub name: String,
    pub pool_length_m: u32,
    pub blocks: Vec<Block>,
    /// Short coach note about the swim, e.g. from the AI creator. Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Longest coach note kept; longer text is cut at a character boundary.
pub const DESCRIPTION_MAX_CHARS: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub name: String,
    #[serde(default = "one")]
    pub repeat: u32,
    pub sets: Vec<WorkoutSet>,
}

/// Named `WorkoutSet` because `Set` clashes with Swift's `Set`; JSON is unaffected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct WorkoutSet {
    #[serde(default = "one")]
    pub reps: u32,
    pub distance_m: u32,
    #[serde(default)]
    pub stroke: Stroke,
    #[serde(default)]
    pub intensity: Intensity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest: Option<Rest>,
    /// Target time per rep in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_s: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub equipment: Vec<Equipment>,
    /// Exercise id from the catalogue (`exercises()`), e.g. "catch-up".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct Rest {
    #[serde(rename = "type")]
    pub kind: RestKind,
    pub seconds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum RestKind {
    /// Start the next rep every `seconds`.
    Interval,
    /// Rest `seconds` after finishing the rep.
    Fixed,
}

/// Matches `HealthKit`'s swimming stroke styles, plus `Any`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Enum,
)]
#[serde(rename_all = "snake_case")]
pub enum Stroke {
    #[default]
    Any,
    Freestyle,
    Backstroke,
    Breaststroke,
    Butterfly,
    Mixed,
    Kickboard,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Enum,
)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    #[default]
    Easy,
    Moderate,
    Threshold,
    Hard,
    Sprint,
}

/// Swim toys a set uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum Equipment {
    Fins,
    Paddles,
    PullBuoy,
    Kickboard,
    Snorkel,
}

/// One `HealthKit` lap (normally one length). Times are seconds since the session started.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Lap {
    pub start_s: f64,
    pub end_s: f64,
    pub stroke: Stroke,
    /// `HealthKit`'s `swimmingStrokeCount`; `None` when it gives none (e.g. kickboard).
    pub strokes: Option<u32>,
}

/// Where a lap or the swimmer is in the plan. All indices are 0-based; `round` counts block repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Position {
    pub block: u32,
    pub round: u32,
    pub set: u32,
    pub rep: u32,
}

/// One heart-rate reading.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct HrSample {
    /// Seconds since `started_at_ms`.
    pub t_s: f64,
    pub bpm: f64,
}

/// Where the swim started: one fix at the start, no route.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
    pub accuracy_m: f64,
}

/// A swum workout: the plan, the laps and the rep each lap was assigned to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Session {
    pub workout: Workout,
    /// Unix ms wall clock of the `HKWorkoutSession` start; lap times are relative to it.
    pub started_at_ms: i64,
    /// Id of the stored workout this was swum from; `workout` stays the source of truth.
    pub workout_id: Option<String>,
    pub laps: Vec<Lap>,
    /// Parallel to `laps`.
    pub lap_positions: Vec<Position>,
    /// Lengths per lap, parallel to `laps`: 1, or 2 for a merged lap.
    pub lap_lengths: Vec<u32>,
    /// Heart-rate samples in time order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hr: Vec<HrSample>,
    /// Swimmer's age on the day, for HR zones; from `HealthKit`'s date of birth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_years: Option<u32>,
    /// Where the swim started, if the swimmer shared it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum WorkoutError {
    #[error("invalid JSON: {msg}")]
    Json { msg: String },
    #[error("invalid workout: {msg}")]
    Invalid { msg: String },
}

const fn one() -> u32 {
    1
}

fn invalid(msg: impl Into<String>) -> WorkoutError {
    WorkoutError::Invalid { msg: msg.into() }
}

fn from_serde(e: serde_json::Error) -> WorkoutError {
    match e.classify() {
        serde_json::error::Category::Data => invalid(e.to_string()), // well-formed JSON with missing or wrong fields
        _ => WorkoutError::Json { msg: e.to_string() },
    }
}

/// Parse and validate workout JSON (trust boundary: input comes from users and AI apps).
#[uniffi::export]
pub fn parse_workout(json: String) -> Result<Workout, WorkoutError> {
    let mut w: Workout = serde_json::from_str(&json).map_err(from_serde)?;
    w.validate()?;
    w.description = w
        .description
        .map(|d| {
            d.trim()
                .chars()
                .take(DESCRIPTION_MAX_CHARS)
                .collect::<String>()
        })
        .filter(|d| !d.is_empty());
    Ok(w)
}

#[uniffi::export]
#[must_use]
#[expect(clippy::expect_used, reason = "serializing plain data cannot fail")]
pub fn workout_to_json(workout: Workout) -> String {
    serde_json::to_string_pretty(&workout).expect("workout always serializes")
}

impl Workout {
    pub fn validate(&self) -> Result<(), WorkoutError> {
        if self.version != FORMAT_VERSION {
            return Err(invalid(format!(
                "unsupported version {}, expected {FORMAT_VERSION}",
                self.version
            )));
        }
        if self.pool_length_m == 0 {
            return Err(invalid("pool_length_m must be > 0"));
        }
        if self.blocks.is_empty() {
            return Err(invalid("workout has no blocks"));
        }
        for b in &self.blocks {
            if b.repeat == 0 || b.sets.is_empty() {
                return Err(invalid(format!(
                    "block '{}' needs repeat >= 1 and at least one set",
                    b.name
                )));
            }
            for s in &b.sets {
                if s.reps == 0 || s.distance_m == 0 {
                    return Err(invalid(format!(
                        "block '{}': reps and distance_m must be > 0",
                        b.name
                    )));
                }
                if s.goal_s == Some(0) {
                    return Err(invalid(format!("block '{}': goal_s must be > 0", b.name)));
                }
                // The engine counts whole lengths, so a rep must end at a wall.
                if s.distance_m % self.pool_length_m != 0 {
                    return Err(invalid(format!(
                        "block '{}': {} m is not a multiple of the {} m pool",
                        b.name, s.distance_m, self.pool_length_m
                    )));
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn total_distance_m(&self) -> u32 {
        self.blocks
            .iter()
            .map(|b| b.repeat * b.sets.iter().map(|s| s.reps * s.distance_m).sum::<u32>())
            .sum()
    }
}

/// A multi-week training plan; each workout is a full workout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub version: u32,
    pub name: String,
    pub weeks: Vec<PlanWeek>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct PlanWeek {
    /// What the week works on, e.g. "Build aerobic base".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    pub workouts: Vec<Workout>,
}

impl Plan {
    pub fn validate(&self) -> Result<(), WorkoutError> {
        if self.version != FORMAT_VERSION {
            return Err(invalid(format!(
                "unsupported version {}, expected {FORMAT_VERSION}",
                self.version
            )));
        }
        if self.weeks.is_empty() {
            return Err(invalid("plan has no weeks"));
        }
        for (w, week) in self.weeks.iter().enumerate() {
            if week.workouts.is_empty() {
                return Err(invalid(format!("week {}: no workouts", w + 1)));
            }
            for (i, workout) in week.workouts.iter().enumerate() {
                workout.validate().map_err(|e| match e {
                    WorkoutError::Invalid { msg } => {
                        invalid(format!("week {}, workout {}: {msg}", w + 1, i + 1))
                    }
                    other @ WorkoutError::Json { .. } => other,
                })?;
            }
        }
        Ok(())
    }
}

#[uniffi::export]
pub fn parse_plan(json: String) -> Result<Plan, WorkoutError> {
    let p: Plan = serde_json::from_str(&json).map_err(from_serde)?;
    p.validate()?;
    Ok(p)
}

#[uniffi::export]
#[must_use]
#[expect(clippy::expect_used, reason = "serializing plain data cannot fail")]
pub fn plan_to_json(plan: Plan) -> String {
    serde_json::to_string_pretty(&plan).expect("plan always serializes")
}

/// What a pasted or imported JSON file holds: a plan if it has a top-level "weeks", else a workout.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Import {
    Workout { workout: Workout },
    Plan { plan: Plan },
}

/// Drill ids must be in this catalogue. Checked on import only: a phone and a watch can run different catalogues,
/// so stored and synced workouts keep their ids.
fn check_drills(w: &Workout) -> Result<(), WorkoutError> {
    for b in &w.blocks {
        for s in &b.sets {
            if let Some(id) = s
                .drill
                .as_ref()
                .filter(|id| !crate::exercises::exercise_ids().any(|known| known == id.as_str()))
            {
                return Err(invalid(format!("block '{}': unknown drill '{id}'", b.name)));
            }
        }
    }
    Ok(())
}

#[uniffi::export]
pub fn parse_import(json: String) -> Result<Import, WorkoutError> {
    let v: serde_json::Value = serde_json::from_str(&json).map_err(from_serde)?;
    Ok(if v.get("weeks").is_some() {
        let plan = parse_plan(json)?;
        plan.weeks
            .iter()
            .flat_map(|wk| &wk.workouts)
            .try_for_each(check_drills)?;
        Import::Plan { plan }
    } else {
        let workout = parse_workout(json)?;
        check_drills(&workout)?;
        Import::Workout { workout }
    })
}

/// The fixed id a plan's workout is sent to the watch under, so the swum Session's `workout_id` links back to the plan.
#[uniffi::export]
#[must_use]
pub fn plan_workout_id(plan_id: String, week: u32, index: u32) -> String {
    format!("{plan_id}/{week}/{index}")
}

#[cfg(test)]
mod tests;
