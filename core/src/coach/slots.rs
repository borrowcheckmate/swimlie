use super::{
    Equipment, Exercise, ExerciseKind, Intensity, Rest, RestKind, Stroke, SwimLevel, WorkoutSet,
    metrics, pick,
};

pub(super) struct Slot {
    pub(super) share: f64,
    pub(super) goal: bool,
    pub(super) set: WorkoutSet,
}

pub(super) const fn timed(share: f64, set: WorkoutSet) -> Slot {
    Slot {
        share,
        goal: true,
        set,
    }
}

pub(super) const fn untimed(share: f64, set: WorkoutSet) -> Slot {
    Slot {
        share,
        goal: false,
        set,
    }
}

pub(super) fn new_set(
    distance_m: u32,
    stroke: Stroke,
    intensity: Intensity,
    equipment: Vec<Equipment>,
    notes: Option<&str>,
) -> WorkoutSet {
    WorkoutSet {
        reps: 1,
        distance_m,
        stroke,
        intensity,
        rest: None,
        goal_s: None,
        equipment,
        drill: None,
        notes: notes.map(String::from),
    }
}

/// Adds a short kick set when a kickboard is selected, taking 15% of the main volume.
/// Without a kickboard the templates are returned unchanged.
pub(super) fn with_kick(slots: Vec<Slot>, pool: u32, have: &[Equipment]) -> Vec<Slot> {
    if !have.contains(&Equipment::Kickboard) {
        return slots;
    }
    let mut out: Vec<Slot> = slots
        .into_iter()
        .map(|slot| Slot {
            share: slot.share * 0.85,
            ..slot
        })
        .collect();
    out.push(untimed(
        0.15,
        new_set(
            round_to_pool(50.0, pool),
            Stroke::Kickboard,
            Intensity::Easy,
            vec![Equipment::Kickboard],
            None,
        ),
    ));
    out
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "the result is stored in an `Option<Rest>` field"
)]
pub(super) const fn fixed(seconds: u32) -> Option<Rest> {
    Some(Rest {
        kind: RestKind::Fixed,
        seconds,
    })
}

/// Seconds per 100 m: CSS times an intensity and a stroke multiplier.
pub(super) fn pace(css: Option<f64>, stroke: Stroke, intensity: Intensity) -> f64 {
    let intensity_x = match intensity {
        Intensity::Easy => 1.15,
        Intensity::Moderate => 1.08,
        Intensity::Threshold => 1.02,
        Intensity::Hard => 0.96,
        Intensity::Sprint => 0.88,
    };
    let stroke_x = match stroke {
        Stroke::Backstroke => 1.10,
        Stroke::Butterfly | Stroke::Mixed => 1.12,
        Stroke::Breaststroke => 1.20,
        Stroke::Any | Stroke::Freestyle | Stroke::Kickboard => 1.0,
    };
    metrics::css_pace(css) * intensity_x * stroke_x
}

impl SwimLevel {
    /// Pace in s/100 m when there is no usable CSS (PLAN.md 4b).
    pub(super) const fn no_css_pace(self) -> f64 {
        match self {
            Self::Beginner => 265.0,
            Self::Intermediate => 170.0,
            Self::Advanced => 138.0,
        }
    }

    /// Scale of the rest after each rep when there is no usable CSS: beginners rest longer.
    pub(super) const fn no_css_rest_x(self) -> f64 {
        match self {
            Self::Beginner => 1.5,
            Self::Intermediate | Self::Advanced => 1.0,
        }
    }

    /// Scale of the starting volume when there is no history (PLAN.md 4b).
    pub(super) const fn volume_x(self) -> f64 {
        match self {
            Self::Beginner => 0.6,
            Self::Intermediate => 1.0,
            Self::Advanced => 1.2,
        }
    }
}

/// Sprint sets stop at this many reps; the metres they lose go to the moderate set.
pub(super) const SPRINT_REPS: u32 = 8;

/// Rest after each rep of a timed set (PLAN.md 4b): sprints get a fixed rest of goal, clamped to 20-60 s;
/// a 1-rep set gets a fixed rest; other sets an interval of goal + rest. `rest_x` scales the rest (no-CSS levels).
pub(super) fn swim_rest(set: &WorkoutSet, goal: u32, rest_x: f64) -> Rest {
    if set.intensity == Intensity::Sprint {
        return Rest {
            kind: RestKind::Fixed,
            seconds: up5(f64::from(goal.clamp(20, 60))),
        };
    }
    let per_100 = f64::from(set.distance_m) / 100.0;
    let rest = rest_x
        * if set.intensity == Intensity::Hard {
            10.0f64.mul_add(per_100, 15.0)
        } else {
            5.0f64.mul_add(per_100, 10.0)
        };
    if set.reps == 1 {
        Rest {
            kind: RestKind::Fixed,
            seconds: up5(rest),
        }
    } else {
        Rest {
            kind: RestKind::Interval,
            seconds: up5(f64::from(goal) + rest),
        }
    }
}

/// Rounds up to a multiple of 5 s.
pub(super) fn up5(s: f64) -> u32 {
    (s / 5.0).ceil() as u32 * 5
}

/// The nearest multiple of the pool, at least one length.
pub(super) fn round_to_pool(m: f64, pool: u32) -> u32 {
    (m / f64::from(pool)).round().max(1.0) as u32 * pool
}

/// The wanted equipment that the swimmer has, so no set uses gear that was not in the inputs.
pub(super) fn gear(have: &[Equipment], want: &[Equipment]) -> Vec<Equipment> {
    want.iter().copied().filter(|e| have.contains(e)).collect()
}

/// An exercise as a set: `reps` of its rep distance, its cue as the note, a fixed 20 s rest. Walls and starts
/// need two lengths (a turn, or a dive that ends back at the block end). The set is `Any`: the drill names the stroke.
pub(super) fn exercise_set(e: &Exercise, reps: u32, pool: u32) -> WorkoutSet {
    let min_m = if matches!(e.kind, ExerciseKind::Wall | ExerciseKind::Start) {
        2 * pool
    } else {
        0
    };
    WorkoutSet {
        reps,
        distance_m: round_to_pool(f64::from(e.rep_m), pool).max(min_m),
        stroke: Stroke::Any,
        intensity: Intensity::Easy,
        rest: fixed(20),
        goal_s: None,
        equipment: e.equipment.clone(),
        drill: Some(e.id.clone()),
        notes: Some(e.cue.clone()),
    }
}

/// A technique drill slot (0.15 of the main volume): the catalogue drill, with the snorkel on freestyle ones
/// (not head-up, which is about the head position). Catch-up is always a drill with no gear, so a pick always fits.
#[expect(
    clippy::expect_used,
    reason = "catch-up is a drill and needs no gear, so a pick always exists"
)]
pub(super) fn drill_slot(n: u32, have: &[Equipment], pool: u32, lang: &str) -> Slot {
    let e = pick(&[ExerciseKind::Drill], have, false, n, lang).expect("catch-up is always a drill");
    let mut set = exercise_set(&e, 1, pool);
    if e.stroke == Stroke::Freestyle && e.id != "head-up" {
        set.equipment.extend(gear(have, &[Equipment::Snorkel]));
    }
    untimed(0.15, set)
}
