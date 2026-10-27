use super::{Equipment, Intensity, Slot, Stroke, WorkoutSet};

/// Adds one rep to the main set furthest below its share, among non-sprint sets whose rep is at most `room` m
/// (sprint reps stay as the share and `SPRINT_REPS` cap set them). False if no set can take one.
pub(super) fn add_rep(main: &mut [Slot], main_m: f64, room: f64) -> bool {
    let short = |s: &Slot| {
        s.share
            .mul_add(main_m, -f64::from(s.set.reps * s.set.distance_m))
    };
    let pick = (0..main.len())
        .filter(|&i| {
            main[i].set.intensity != Intensity::Sprint && f64::from(main[i].set.distance_m) <= room
        })
        .max_by(|&a, &b| short(&main[a]).total_cmp(&short(&main[b])));
    if let Some(i) = pick {
        main[i].set.reps += 1;
    }
    pick.is_some()
}

/// Cuts a skill set from 4 reps to 2, then drops it.
pub(super) fn cut(skill: &mut Option<WorkoutSet>) {
    if let Some(s) = skill.as_mut().filter(|s| s.reps > 2) {
        s.reps = 2;
    } else {
        *skill = None;
    }
}

/// Gives each selected piece of gear that no main set uses yet to the fitting main set with the least gear.
pub(super) fn spread_gear(sets: &mut [WorkoutSet], have: &[Equipment]) {
    let fits = |s: &WorkoutSet, e: Equipment| match e {
        Equipment::Paddles | Equipment::PullBuoy | Equipment::Snorkel => {
            s.stroke == Stroke::Freestyle && s.intensity != Intensity::Sprint
        }
        Equipment::Fins => s.stroke != Stroke::Breaststroke,
        Equipment::Kickboard => s.stroke == Stroke::Kickboard,
    };
    for &e in have {
        if sets
            .iter()
            .any(|s| s.drill.is_none() && s.equipment.contains(&e))
        {
            continue;
        }
        if let Some(s) = sets
            .iter_mut()
            .filter(|s| s.drill.is_none() && fits(s, e))
            .min_by_key(|s| s.equipment.len())
        {
            s.equipment.push(e);
        }
    }
}
