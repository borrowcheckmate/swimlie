use super::{BTreeMap, Equipment, Intensity, StoredSession, Stroke, canonical_equipment, reps};

/// Comparable reps from the most recent session for each set configuration.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GoalHistory {
    pub stroke: Stroke,
    pub distance_m: u32,
    pub intensity: Intensity,
    pub equipment: Vec<Equipment>,
    pub median_s: f64,
}

type Key = (u32, u32, u32, Vec<u32>);
type Entry = (i64, GoalHistory, Vec<f64>);

pub(super) fn goal_history<'a>(
    sessions: impl Iterator<Item = &'a StoredSession>,
) -> Vec<GoalHistory> {
    let mut groups: BTreeMap<Key, Entry> = BTreeMap::new();
    for stored in sessions {
        let session = &stored.session;
        for rep in reps(session) {
            let Some(set) = session
                .workout
                .blocks
                .get(rep.pos.block as usize)
                .and_then(|b| b.sets.get(rep.pos.set as usize))
            else {
                continue;
            };
            let guessed = session
                .lap_lengths
                .iter()
                .zip(&session.lap_positions)
                .any(|(&n, &p)| p == rep.pos && n != 1);
            if set.goal_s.is_none()
                || set.drill.is_some()
                || guessed
                || set.stroke == Stroke::Any
                || rep.lengths != set.distance_m / session.workout.pool_length_m
                || !rep.swim_s.is_finite()
                || rep.swim_s <= 0.0
            {
                continue;
            }
            let gear = canonical_equipment(&set.equipment);
            let key = (
                set.stroke as u32,
                set.distance_m,
                set.intensity as u32,
                gear.iter().map(|e| *e as u32).collect(),
            );
            let group = groups.entry(key).or_insert_with(|| {
                (
                    session.started_at_ms,
                    GoalHistory {
                        stroke: set.stroke,
                        distance_m: set.distance_m,
                        intensity: set.intensity,
                        equipment: gear,
                        median_s: 0.0,
                    },
                    Vec::new(),
                )
            });
            if session.started_at_ms > group.0 {
                group.0 = session.started_at_ms;
                group.2.clear();
            }
            if session.started_at_ms == group.0 {
                group.2.push(rep.swim_s);
            }
        }
    }
    groups
        .into_values()
        .map(|(_, mut item, mut times)| {
            times.sort_by(f64::total_cmp);
            let n = times.len();
            item.median_s = if n % 2 == 0 {
                f64::midpoint(times[n / 2 - 1], times[n / 2])
            } else {
                times[n / 2]
            };
            item
        })
        .collect()
}
