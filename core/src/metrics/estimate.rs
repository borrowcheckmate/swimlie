use super::{Phase, Position, RestKind, START_COUNTDOWN_S, State, Workout, WorkoutSet};

/// Total planned distance of a workout, for Swift.
#[uniffi::export]
#[must_use]
pub fn total_distance_m(workout: Workout) -> u32 {
    workout.total_distance_m()
}

#[must_use]
pub fn css_pace(css_s_per_100m: Option<f64>) -> f64 {
    css_s_per_100m
        .filter(|c| c.is_finite() && *c > 0.0)
        .unwrap_or(120.0)
}

/// One rep's swim time and the rest after it. An interval rest is the cycle length, so the rest
/// after the swim is what is left of it.
pub(super) fn rep_estimate_s(set: &WorkoutSet, pace: f64) -> (f64, f64) {
    let swim = set
        .goal_s
        .map_or_else(|| f64::from(set.distance_m) / 100.0 * pace, f64::from);
    let rest = match &set.rest {
        None => 0.0,
        Some(r) if r.kind == RestKind::Interval => (f64::from(r.seconds) - swim).max(0.0),
        Some(r) => f64::from(r.seconds),
    };
    (swim, rest)
}

/// Planned duration: per rep the goal time (else distance at CSS, default 120 s/100 m) plus rest,
/// an interval rest being at least the swim time; plus the start countdown once.
#[uniffi::export]
#[must_use]
pub fn estimated_duration_s(workout: Workout, css_s_per_100m: Option<f64>) -> f64 {
    let pace = css_pace(css_s_per_100m);
    let mut total = START_COUNTDOWN_S;
    for b in &workout.blocks {
        for s in &b.sets {
            let (swim, rest) = rep_estimate_s(s, pace);
            total = f64::mul_add(f64::from(b.repeat * s.reps), swim + rest, total);
        }
    }
    total
}

/// Estimated seconds until the workout ends, from the live state: what is left of the current rep
/// (or of the rest before it) plus every later rep, timed like `estimated_duration_s`.
#[uniffi::export]
#[must_use]
pub fn estimated_remaining_s(workout: Workout, state: State, css_s_per_100m: Option<f64>) -> f64 {
    if state.phase == Phase::Done {
        return 0.0;
    }
    let pace = css_pace(css_s_per_100m);
    let mut plan: Vec<(Position, &WorkoutSet)> = vec![];
    for (b, block) in workout.blocks.iter().enumerate() {
        for round in 0..block.repeat {
            for (s, set) in block.sets.iter().enumerate() {
                for rep in 0..set.reps {
                    plan.push((
                        Position {
                            block: b as u32,
                            round,
                            set: s as u32,
                            rep,
                        },
                        set,
                    ));
                }
            }
        }
    }
    // A position outside this workout (e.g. a stale state) must not panic across FFI.
    let Some(i) = plan.iter().position(|(p, _)| *p == state.pos) else {
        return 0.0;
    };
    // The rest after the very last rep is not part of the workout.
    let rest_of = |k: usize| {
        if k + 1 == plan.len() {
            0.0
        } else {
            rep_estimate_s(plan[k].1, pace).1
        }
    };
    let (swim, _) = rep_estimate_s(plan[i].1, pace);
    let elapsed = state.rep_elapsed_s.unwrap_or(0.0);
    let interval = plan[i]
        .1
        .rest
        .as_ref()
        .is_some_and(|r| r.kind == RestKind::Interval);
    let current = match state.phase {
        Phase::Resting => state.rest_remaining_s.unwrap_or(0.0) + swim + rest_of(i),
        // An interval rep ends at its cycle end, which already includes the rest.
        _ if interval => (swim + rest_of(i) - elapsed).max(0.0),
        _ => (swim - elapsed).max(0.0) + rest_of(i),
    };
    let later: f64 = (i + 1..plan.len())
        .map(|k| rep_estimate_s(plan[k].1, pace).0 + rest_of(k))
        .sum();
    current + later
}
