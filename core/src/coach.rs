//! Rule-based workout generator: the same inputs always give the same workout (PLAN.md section 4b).

use crate::exercises::{Exercise, ExerciseKind, pick};
use crate::i18n::tr;
use crate::metrics::{self, CreatorInputs, SwimLevel, SwimmerProfile};
use crate::model::{
    Block, Equipment, FORMAT_VERSION, Intensity, Plan, PlanWeek, Rest, RestKind, Stroke, Workout,
    WorkoutSet,
};

mod main_set;
mod shuffle;
mod slots;
mod templates;

use main_set::{add_rep, cut, spread_gear};
use shuffle::build_shuffled;
use slots::{
    SPRINT_REPS, Slot, drill_slot, exercise_set, fixed, gear, new_set, pace, round_to_pool,
    swim_rest, timed, untimed, up5, with_kick,
};
use templates::{Template, main_slots};

/// Distance for a time target. The estimate steps when a rep count changes, so a proportional
/// rescale can oscillate across the target; bisect on the distance and keep the closest estimate.
/// Below 400 m is allowed here: sprint rests make a 15 min speed swim shorter than 400 m at CSS 150.
const NO_CSS_NOTE: &str = "No CSS yet: paces assume 2:00 /100 m. Swim the CSS test to set yours.";

/// The no-CSS note in English or Dutch (the validators accept either).
#[cfg(test)]
fn is_no_css_note(note: Option<&str>) -> bool {
    note.is_some_and(|n| n == NO_CSS_NOTE || n == tr("nl", NO_CSS_NOTE))
}

/// Workout types with a main-block template.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SwimType {
    EasyRecovery,
    Endurance,
    Threshold,
    Speed,
    Technique,
    Mixed,
}

impl SwimType {
    /// From the creator's free text (trimmed, any case); anything unknown is mixed.
    pub(crate) fn parse(kind: &str) -> Self {
        match kind.trim().to_lowercase().as_str() {
            "easy recovery" | "easy" | "recovery" => Self::EasyRecovery,
            "endurance" => Self::Endurance,
            "threshold" => Self::Threshold,
            "speed" | "sprint" => Self::Speed,
            "technique" | "drill" => Self::Technique,
            _ => Self::Mixed,
        }
    }

    fn name(self, lang: &str) -> &'static str {
        let en = match self {
            Self::EasyRecovery => "Easy recovery",
            Self::Endurance => "Endurance",
            Self::Threshold => "Threshold",
            Self::Speed => "Speed",
            Self::Technique => "Technique",
            Self::Mixed => "Mixed",
        };
        tr(lang, en)
    }
}

/// A main-block set template: its share of the main volume, and whether it has a goal time
/// (kick and drill sets do not).
fn distance_for_time(css: Option<f64>, minutes: u32, make: &dyn Fn(u32) -> Workout) -> u32 {
    let target = f64::from(minutes) * 60.0;
    let (mut lo, mut hi) = (100, 8000);
    let mut best = (f64::MAX, lo);
    while hi - lo > 1 {
        let mid = u32::midpoint(lo, hi);
        let est = metrics::estimated_duration_s(make(mid), css);
        if (est - target).abs() < best.0 {
            best = ((est - target).abs(), mid);
        }
        if est < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    best.1
}

/// The distance of one workout: the target distance, else the time target, else `default_m`.
fn target_distance(
    inputs: &CreatorInputs,
    css: Option<f64>,
    default_m: u32,
    make: &dyn Fn(u32) -> Workout,
) -> u32 {
    match (inputs.target_distance_m, inputs.target_time_min) {
        (Some(d), _) => d.clamp(400, 8000),
        (None, Some(t)) => distance_for_time(css, t, make),
        (None, None) => default_m,
    }
}

/// The coach: a workout for the creator's form and the swimmer's history (PLAN.md 4b).
#[uniffi::export]
#[must_use]
pub fn generate_workout(inputs: CreatorInputs, profile: SwimmerProfile) -> Workout {
    let kind = SwimType::parse(&inputs.kind);
    let css = profile.css_s_per_100m.filter(|c| c.is_finite() && *c > 0.0);
    let level = profile.level;
    let pace_css = css.or_else(|| Some(level.no_css_pace()));
    let pool = inputs.pool_length_m.max(1);
    // A fresh template variant and exercise seed per call: same goal and distance, shuffled sets.
    let r = uuid::Uuid::new_v4().as_u128() as u64;
    let make = |d: u32| {
        let mut w = build_shuffled(
            kind,
            pool,
            &inputs.equipment,
            css,
            level,
            d,
            (r % 12) as u32,
            ((r / 12) % 10) as u32,
            inputs.starting_blocks,
            Some((r >> 8, &inputs.strokes)),
            &inputs.language,
        );
        scale_rest(&mut w, inputs.rest_percent);
        w
    };
    make(target_distance(&inputs, pace_css, 2000, &make))
}

/// Scales the rest of every timed set by `percent` (-50 to 100), to 5 s. An interval is goal + rest, so only the rest part moves.
fn scale_rest(w: &mut Workout, percent: i32) {
    if percent == 0 {
        return;
    }
    let f = 1.0 + f64::from(percent.clamp(-50, 100)) / 100.0;
    let sets = w.blocks.iter_mut().flat_map(|b| b.sets.iter_mut());
    for s in sets {
        let goal = if s
            .rest
            .as_ref()
            .is_some_and(|r| r.kind == RestKind::Interval)
        {
            s.goal_s.unwrap_or(0)
        } else {
            0
        };
        if let Some(r) = s.rest.as_mut() {
            r.seconds = goal + up5(f64::from(r.seconds.saturating_sub(goal)) * f);
        }
    }
}

const ROTATION: [SwimType; 6] = [
    SwimType::Endurance,
    SwimType::Technique,
    SwimType::Threshold,
    SwimType::Speed,
    SwimType::Mixed,
    SwimType::EasyRecovery,
];
const LIGHT_WEEK: [SwimType; 4] = [
    SwimType::EasyRecovery,
    SwimType::Technique,
    SwimType::Endurance,
    SwimType::Mixed,
];

/// The multi-week plan (PLAN.md 4b): build weeks grow 7% per earlier build week (capped at 1.5x), every 4th week is lighter.
#[uniffi::export]
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "one linear pipeline whose steps share local state; splitting it is pending"
)]
#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "`{weeks}`-style placeholders are locale keys replaced by hand"
)]
pub fn generate_plan(inputs: CreatorInputs, profile: SwimmerProfile) -> Plan {
    let kind = SwimType::parse(&inputs.kind);
    let css = profile.css_s_per_100m.filter(|c| c.is_finite() && *c > 0.0);
    let level = profile.level;
    let pace_css = css.or_else(|| Some(level.no_css_pace()));
    let pool = inputs.pool_length_m.max(1);
    let weeks = inputs.plan_weeks.unwrap_or(4).clamp(1, 16);
    let swims = inputs.swims_per_week.unwrap_or(3).clamp(1, 7);
    let build_at = |t: SwimType, d: u32, variant: u32, seed: u32| {
        build_shuffled(
            t,
            pool,
            &inputs.equipment,
            css,
            level,
            d,
            variant,
            seed,
            inputs.starting_blocks,
            None,
            &inputs.language,
        )
    };
    let per_swim = target_distance(&inputs, pace_css, 1500, &|d| build_at(kind, d, 0, 0));
    let history = profile.weekly_m_28d >= 500.0;
    let v1 = if history {
        profile.weekly_m_28d
    } else {
        f64::from(swims * per_swim) * level.volume_x()
    };
    // Time target without history: each swim takes T x V/V1 minutes, so the distance depends on its type.
    let minutes = inputs
        .target_time_min
        .filter(|_| !history && inputs.target_distance_m.is_none());
    // Each build week leads with the requested type; the others continue the rotation across weeks.
    let others: Vec<SwimType> = ROTATION.into_iter().filter(|&t| t != kind).collect();
    let mut next_other = 0;
    let mut volumes: Vec<f64> = vec![];
    let mut out = vec![];
    for w in 1..=weeks {
        let lighter = w % 4 == 0;
        let v = if lighter {
            0.7 * volumes[w as usize - 2]
        } else {
            v1 * 1.07f64
                .powi((1..w).filter(|&u| u % 4 != 0).count() as i32)
                .min(1.5)
        };
        volumes.push(v);
        let types: Vec<SwimType> = if lighter {
            LIGHT_WEEK.to_vec()
        } else {
            let rest = (0..swims as usize - 1).map(|j| others[(next_other + j) % others.len()]);
            let t = std::iter::once(kind).chain(rest).collect();
            next_other += swims as usize - 1;
            t
        };
        let distance = (v / f64::from(swims)).round().clamp(400.0, 8000.0) as u32;
        let workouts = (0..swims as usize)
            .map(|j| {
                let t = types[j % types.len()];
                let seed = (w - 1) * swims + j as u32;
                let make = |d: u32| build_at(t, d, w - 1, seed);
                make(minutes.map_or(distance, |m| {
                    distance_for_time(pace_css, (f64::from(m) * v / v1).round() as u32, &make)
                }))
            })
            .collect();
        let focus = match w {
            1 => "Base",
            _ if lighter => "Lighter week",
            _ => "Build",
        };
        out.push(PlanWeek {
            focus: Some(tr(&inputs.language, focus).into()),
            workouts,
        });
    }
    let default = if css.is_none() {
        tr(&inputs.language, " (default pace)")
    } else {
        ""
    };
    let name = tr(&inputs.language, "{weeks}-week {kind} plan{default}")
        .replace("{weeks}", &weeks.to_string())
        .replace("{kindlc}", &kind.name(&inputs.language).to_lowercase())
        .replace("{kind}", kind.name(&inputs.language))
        .replace("{default}", default);
    Plan {
        version: FORMAT_VERSION,
        name,
        weeks: out,
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod rest_tests;
