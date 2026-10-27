use super::{BTreeMap, Equipment, Position, StoredSession, Stroke, reps, session_summary};

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PersonalRecord {
    pub distance_m: u32,
    pub stroke: Stroke,
    /// Swim toys of the set, sorted (see `canonical_equipment`). A record only compares with the same gear.
    pub equipment: Vec<Equipment>,
    pub time_s: f64,
    pub session_id: String,
    pub pos: Position,
}

/// Equipment sorted and deduplicated, so the same gear in any order is the same record key.
#[must_use]
pub fn canonical_equipment(equipment: &[Equipment]) -> Vec<Equipment> {
    let mut v = equipment.to_vec();
    v.sort_by_key(|e| *e as u32);
    v.dedup();
    v
}

/// Best time per (distance, stroke, equipment) over all sessions, sorted by distance, stroke, then equipment.
///
/// Only complete reps with no merged lap count: a merged lap makes the distance a guess. Exercise reps (drills,
/// skills, games) never count.
#[must_use]
pub fn personal_records(sessions: &[StoredSession]) -> Vec<PersonalRecord> {
    let mut best: Vec<PersonalRecord> = vec![];
    for st in sessions {
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
            if set.drill.is_some() {
                continue;
            }
            let idx: Vec<usize> = (0..s.laps.len())
                .filter(|&i| s.lap_positions[i] == r.pos)
                .collect();
            if r.lengths != set.distance_m / s.workout.pool_length_m
                || idx.iter().any(|&i| s.lap_lengths[i] != 1)
            {
                continue;
            }
            let stroke = if set.stroke != Stroke::Any {
                set.stroke
            } else if idx
                .iter()
                .all(|&i| s.laps[i].stroke == s.laps[idx[0]].stroke)
            {
                s.laps[idx[0]].stroke
            } else {
                continue;
            };
            let time_s = s.laps[idx[idx.len() - 1]].end_s - s.laps[idx[0]].start_s;
            let equipment = canonical_equipment(&set.equipment);
            let same = |p: &PersonalRecord| {
                p.distance_m == set.distance_m && p.stroke == stroke && p.equipment == equipment
            };
            match best.iter_mut().find(|p| same(p)) {
                Some(p) if p.time_s <= time_s => {}
                Some(p) => {
                    *p = PersonalRecord {
                        distance_m: set.distance_m,
                        stroke,
                        equipment,
                        time_s,
                        session_id: st.id.clone(),
                        pos: r.pos,
                    }
                }
                None => best.push(PersonalRecord {
                    distance_m: set.distance_m,
                    stroke,
                    equipment,
                    time_s,
                    session_id: st.id.clone(),
                    pos: r.pos,
                }),
            }
        }
    }
    best.sort_by_key(|p| {
        (
            p.distance_m,
            p.stroke as u32,
            p.equipment.iter().map(|e| *e as u32).collect::<Vec<_>>(),
        )
    });
    best
}

/// Distance and swim time of one week of sessions.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct WeekTotal {
    pub week_start_ms: i64,
    pub distance_m: u32,
    pub swim_s: f64,
    pub sessions: u32,
}

/// Totals per week (Monday 00:00 local time), oldest first; only weeks with sessions. `utc_offset_s` = local time minus UTC, so weeks follow the swimmer's time zone.
#[uniffi::export]
#[must_use]
pub fn weekly_totals(sessions: Vec<StoredSession>, utc_offset_s: i32) -> Vec<WeekTotal> {
    const DAY_MS: i64 = 86_400_000;
    let offset_ms = i64::from(utc_offset_s) * 1000;
    let mut weeks: BTreeMap<i64, WeekTotal> = BTreeMap::new();
    for st in &sessions {
        let local_days = (st.session.started_at_ms + offset_ms).div_euclid(DAY_MS);
        // 1970-01-01 was a Thursday, so (days + 3) % 7 is 0 on a Monday.
        let monday_local = (local_days - (local_days + 3).rem_euclid(7)) * DAY_MS;
        let week_start_ms = monday_local - offset_ms;
        let sum = session_summary(&st.session, None);
        let week = weeks.entry(week_start_ms).or_insert(WeekTotal {
            week_start_ms,
            distance_m: 0,
            swim_s: 0.0,
            sessions: 0,
        });
        week.distance_m += sum.distance_m;
        week.swim_s += sum.swim_s;
        week.sessions += 1;
    }
    weeks.into_values().collect()
}

/// Median swim time of the complete reps of one stroke and distance.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct StrokeTime {
    pub stroke: Stroke,
    pub distance_m: u32,
    pub median_s: f64,
    pub count: u32,
}
