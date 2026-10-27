use super::*;
use crate::exercises::exercise;

mod main_block;
mod plan;
mod rules;

use plan::*;

pub(super) fn input(
    kind: &str,
    pool: u32,
    equipment: Vec<Equipment>,
    distance: Option<u32>,
    minutes: Option<u32>,
) -> CreatorInputs {
    CreatorInputs {
        language: "en".into(),
        pool_length_m: pool,
        equipment,
        starting_blocks: false,
        target_distance_m: distance,
        target_time_min: minutes,
        kind: kind.into(),
        notes: String::new(),
        plan_weeks: None,
        swims_per_week: None,
        rest_percent: 0,
        strokes: vec![],
    }
}

pub(super) fn swimmer(css: Option<f64>) -> SwimmerProfile {
    SwimmerProfile {
        css_s_per_100m: css,
        sessions_28d: 0,
        weekly_m_28d: 0.0,
        rep_times: vec![],
        age_years: None,
        max_hr_bpm: None,
        level: SwimLevel::default(),
    }
}

pub(super) fn sets(w: &Workout) -> impl Iterator<Item = &WorkoutSet> {
    w.blocks.iter().flat_map(|b| &b.sets)
}

/// Rule violations of one generated workout, as messages naming the failing input.
pub(super) fn violations(
    kind: &str,
    pool: u32,
    equipment: &[Equipment],
    distance: Option<u32>,
    minutes: Option<u32>,
    css: Option<f64>,
    blocks: bool,
) -> Vec<String> {
    let inputs = CreatorInputs {
        starting_blocks: blocks,
        ..input(kind, pool, equipment.to_vec(), distance, minutes)
    };
    let w = generate_workout(inputs, swimmer(css));
    let mut v = vec![];
    if let Err(e) = w.validate() {
        v.push(format!("invalid: {e}"));
    }
    if w.blocks.len() != 3 || w.blocks[0].name != "Warm-up" || w.blocks[2].name != "Cool-down" {
        v.push("blocks are not warm-up, main, cool-down".into());
        return v;
    }
    if !sets(&w).all(|s| s.equipment.iter().all(|e| equipment.contains(e))) {
        v.push("a set uses gear that is not in the inputs".into());
    }
    for s in sets(&w) {
        let Some(id) = &s.drill else { continue };
        let Some(e) = exercise(id.clone()) else {
            v.push(format!("unknown drill {id}"));
            continue;
        };
        if s.stroke != Stroke::Any {
            v.push(format!("drill {id} set has stroke {:?}", s.stroke));
        }
        if matches!(e.kind, ExerciseKind::Wall | ExerciseKind::Start) && s.distance_m < 2 * pool {
            v.push(format!("{id} rep is {} m, under two lengths", s.distance_m));
        }
        if !e.equipment.iter().all(|g| equipment.contains(g)) {
            v.push(format!("drill {id} needs gear that is not in the inputs"));
        }
        if e.kind == ExerciseKind::Start && !blocks {
            v.push(format!("start {id} without starting blocks"));
        }
        if e.safety.as_deref() == Some(crate::exercises::BREATH_HOLD) {
            v.push(format!("breath-hold drill {id} was picked"));
        }
    }
    if !equipment.contains(&Equipment::Kickboard) && sets(&w).any(|s| s.stroke == Stroke::Kickboard)
    {
        v.push("kickboard stroke without a kickboard".into());
    }
    // The warm-up's build set is moderate by design, so only its first set and the cool-down are checked.
    if w.blocks[0].sets[0].intensity != Intensity::Easy {
        v.push("warm-up is not easy".into());
    }
    if !w.blocks[2]
        .sets
        .iter()
        .all(|s| s.intensity == Intensity::Easy)
    {
        v.push("cool-down is not easy".into());
    }
    let mut seen = vec![];
    for s in &w.blocks[1].sets {
        let key = (s.distance_m, s.stroke, s.intensity, s.drill.clone());
        if seen.contains(&key) {
            v.push(format!("two main sets share {key:?}"));
        }
        seen.push(key);
        if s.stroke == Stroke::Mixed && s.distance_m % (4 * pool) != 0 {
            v.push(format!(
                "IM {} m is not a multiple of 4 lengths",
                s.distance_m
            ));
        }
        let Some(goal) = s.goal_s else {
            if matches!(
                s.intensity,
                Intensity::Moderate | Intensity::Threshold | Intensity::Hard | Intensity::Sprint
            ) {
                v.push(format!(
                    "{} m {:?} main set has no goal_s",
                    s.distance_m, s.intensity
                ));
            }
            continue;
        };
        // PLAN.md 4b: interval = goal + rest rounded up to 5 s; 1-rep sets and sprints get a fixed rest.
        let per_100 = f64::from(s.distance_m) / 100.0;
        let rest_x = if css.is_none() {
            SwimLevel::default().no_css_rest_x()
        } else {
            1.0
        };
        let rest = rest_x
            * if s.intensity == Intensity::Hard {
                10.0f64.mul_add(per_100, 15.0)
            } else {
                5.0f64.mul_add(per_100, 10.0)
            };
        let want = match (s.intensity, s.reps) {
            (Intensity::Sprint, _) => Rest {
                kind: RestKind::Fixed,
                seconds: up5(f64::from(goal.clamp(20, 60))),
            },
            (_, 1) => Rest {
                kind: RestKind::Fixed,
                seconds: up5(rest),
            },
            _ => Rest {
                kind: RestKind::Interval,
                seconds: up5(f64::from(goal) + rest),
            },
        };
        if s.rest.as_ref() != Some(&want) {
            v.push(format!(
                "{} m x{} {:?} rest {:?}, want {want:?}",
                s.distance_m, s.reps, s.intensity, s.rest
            ));
        }
        if s.reps == 1
            && matches!(
                s.rest,
                Some(Rest {
                    kind: RestKind::Interval,
                    ..
                })
            )
        {
            v.push(format!("{} m 1-rep set has an interval rest", s.distance_m));
        }
    }
    if w.name.contains("default pace") || w.name.contains("standaardtempo") {
        v.push(format!("name {} mentions the default pace", w.name));
    }
    let usable = css.is_some_and(|c| c.is_finite() && c > 0.0);
    let has_note = is_no_css_note(w.blocks[0].sets[0].notes.as_deref());
    if has_note == usable {
        v.push(format!(
            "first set notes {:?} do not match the CSS state",
            w.blocks[0].sets[0].notes
        ));
    }
    match (distance, minutes) {
        (Some(d), _) => {
            let target = d.clamp(400, 8000);
            let total = w.total_distance_m();
            if total.abs_diff(target) * 2 > pool {
                v.push(format!("total {total} m, target {target} m"));
            }
        }
        (None, Some(t)) => {
            let target = f64::from(t) * 60.0;
            let est = metrics::estimated_duration_s(
                w,
                css.or_else(|| Some(SwimLevel::default().no_css_pace())),
            );
            if (est - target).abs() > 0.1 * target {
                v.push(format!("estimated {est:.0} s, target {target} s"));
            }
        }
        (None, None) => unreachable!(),
    }
    v
}

/// The workout of a type at a distance (PLAN.md 4b). `variant` picks the template variant, `seed` the exercises.
#[expect(
    clippy::too_many_arguments,
    reason = "test shorthand over `build_shuffled`"
)]
pub(super) fn build(
    kind: SwimType,
    pool: u32,
    equipment: &[Equipment],
    css: Option<f64>,
    distance_m: u32,
    variant: u32,
    seed: u32,
    blocks: bool,
) -> Workout {
    build_shuffled(
        kind,
        pool,
        equipment,
        css,
        SwimLevel::Intermediate,
        distance_m,
        variant,
        seed,
        blocks,
        None,
        "en",
    )
}
