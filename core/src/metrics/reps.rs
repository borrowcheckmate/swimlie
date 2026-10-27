use super::{HrSample, Lap, Position, Range, Session, Stroke};

/// Metrics of one rep (the consecutive laps that share a `Position`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RepMetrics {
    pub pos: Position,
    /// Sum of `lap_lengths` over the rep's laps.
    pub lengths: u32,
    /// `lengths * pool_length_m`.
    pub distance_m: u32,
    /// First lap start to last lap end: turns included, the rest after the rep not.
    pub swim_s: f64,
    /// `swim_s / distance_m * 100`.
    pub pace_s_per_100m: f64,
    /// `swim_s - goal_s` of the rep's set; `None` without a goal or if the rep's length count differs from the set's distance (incomplete or extra laps).
    pub goal_delta_s: Option<f64>,
    /// Next rep's first lap start minus this rep's last lap end; `None` for the last rep.
    pub rest_after_s: Option<f64>,
    /// Per lap: duration + strokes; `None` if no strokes or the lap is merged.
    pub swolf: Vec<Option<f64>>,
    /// Mean of the `Some` SWOLF values.
    pub swolf_avg: Option<f64>,
    /// Strokes / minutes over the laps with strokes (merged laps included).
    pub stroke_rate_per_min: Option<f64>,
    /// Mean heart rate from the rep's first lap start to its last lap end.
    pub avg_bpm: Option<f64>,
    pub max_bpm: Option<f64>,
    /// HR at the rep's end minus HR at the next rep's start (positive = recovering); `None` for the last rep or without samples.
    pub rest_hr_drop_bpm: Option<f64>,
    /// Metres per stroke over the laps with strokes (merged laps included).
    pub distance_per_stroke_m: Option<f64>,
}

/// Lap index range of each rep: consecutive laps with the same position.
pub(super) fn rep_groups(session: &Session) -> Vec<Range<usize>> {
    let mut groups: Vec<Range<usize>> = vec![];
    for i in 0..session.laps.len() {
        match groups.last_mut() {
            Some(g) if session.lap_positions[g.start] == session.lap_positions[i] => g.end = i + 1,
            _ => groups.push(i..i + 1),
        }
    }
    groups
}

/// `(strokes, metres, seconds)` of the laps in `range` that have a stroke count.
pub(super) fn counted(session: &Session, range: Range<usize>) -> Vec<(u32, u32, f64)> {
    let pool = session.workout.pool_length_m;
    range
        .filter_map(|i| {
            let lap = session.laps[i];
            lap.strokes
                .map(|s| (s, session.lap_lengths[i] * pool, lap.end_s - lap.start_s))
        })
        .collect()
}

/// Strokes per minute and metres per stroke of `counted` laps; each `None` when it has no basis.
pub(super) fn stroke_stats(counted: &[(u32, u32, f64)]) -> (Option<f64>, Option<f64>) {
    let strokes: u32 = counted.iter().map(|c| c.0).sum();
    let metres: u32 = counted.iter().map(|c| c.1).sum();
    let secs: f64 = counted.iter().map(|c| c.2).sum();
    (
        (!counted.is_empty()).then(|| f64::from(strokes) / secs * 60.0),
        (strokes > 0).then(|| f64::from(metres) / f64::from(strokes)),
    )
}

/// (mean, max) of the bpm values; `None`s if there are none.
pub(super) fn hr_stats(bpm: impl Iterator<Item = f64>) -> (Option<f64>, Option<f64>) {
    let v: Vec<f64> = bpm.collect();
    (
        (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64),
        v.iter().copied().reduce(f64::max),
    )
}

/// (mean, max) bpm of the samples with `start_s <= t_s <= end_s`.
pub(super) fn hr_in(hr: &[HrSample], start_s: f64, end_s: f64) -> (Option<f64>, Option<f64>) {
    hr_stats(
        hr.iter()
            .filter(|h| h.t_s >= start_s && h.t_s <= end_s)
            .map(|h| h.bpm),
    )
}

/// Last bpm at or before `t`; samples are in time order.
pub(super) fn bpm_at(hr: &[HrSample], t: f64) -> Option<f64> {
    hr.iter().rev().find(|h| h.t_s <= t).map(|h| h.bpm)
}

/// SWOLF of one lap: duration + strokes; `None` without strokes or if the lap is merged.
// shortcut: merged laps are skipped, not split into two lengths, since their strokes are ambiguous.
pub(super) fn lap_swolf(session: &Session, i: usize) -> Option<f64> {
    let lap = session.laps[i];
    if session.lap_lengths[i] > 1 {
        None
    } else {
        lap.strokes.map(|s| lap.end_s - lap.start_s + f64::from(s))
    }
}

/// One `RepMetrics` per rep with laps, in lap order.
#[uniffi::export]
#[must_use]
pub fn reps(session: &Session) -> Vec<RepMetrics> {
    let pool = session.workout.pool_length_m;
    let groups = rep_groups(session);

    let mut out = Vec::with_capacity(groups.len());
    for (k, g) in groups.iter().enumerate() {
        let lengths: u32 = session.lap_lengths[g.clone()].iter().sum();
        let distance_m = lengths * pool;
        let (start_s, end_s) = (session.laps[g.start].start_s, session.laps[g.end - 1].end_s);
        let swim_s = end_s - start_s;
        let swolf: Vec<Option<f64>> = g.clone().map(|i| lap_swolf(session, i)).collect();
        let known: Vec<f64> = swolf.iter().flatten().copied().collect();
        let swolf_avg = (!known.is_empty()).then(|| known.iter().sum::<f64>() / known.len() as f64);
        let (stroke_rate_per_min, distance_per_stroke_m) =
            stroke_stats(&counted(session, g.clone()));
        let (avg_bpm, max_bpm) = hr_in(&session.hr, start_s, end_s);
        let rest_after_s = groups
            .get(k + 1)
            .map(|next| session.laps[next.start].start_s - end_s);
        let rest_hr_drop_bpm = groups.get(k + 1).and_then(|next| {
            Some(
                bpm_at(&session.hr, end_s)?
                    - bpm_at(&session.hr, session.laps[next.start].start_s)?,
            )
        });
        let pos = session.lap_positions[g.start];
        let set = session
            .workout
            .blocks
            .get(pos.block as usize)
            .and_then(|b| b.sets.get(pos.set as usize));
        let goal_delta_s = set.and_then(|s| {
            let goal = f64::from(s.goal_s?);
            (lengths == s.distance_m / pool).then_some(swim_s - goal)
        });
        out.push(RepMetrics {
            pos,
            lengths,
            distance_m,
            swim_s,
            pace_s_per_100m: swim_s / f64::from(distance_m) * 100.0,
            goal_delta_s,
            rest_after_s,
            swolf,
            swolf_avg,
            stroke_rate_per_min,
            avg_bpm,
            max_bpm,
            rest_hr_drop_bpm,
            distance_per_stroke_m,
        });
    }
    out
}

/// Metrics of one lap.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LapMetrics {
    /// Index into `Session.laps`.
    pub index: u32,
    pub pos: Position,
    pub stroke: Stroke,
    /// 1, or 2 for a merged lap.
    pub lengths: u32,
    pub distance_m: u32,
    pub start_s: f64,
    pub swim_s: f64,
    pub pace_s_per_100m: f64,
    pub strokes: Option<u32>,
    /// `None` if no strokes or the lap is merged.
    pub swolf: Option<f64>,
    pub stroke_rate_per_min: Option<f64>,
    pub distance_per_stroke_m: Option<f64>,
    pub avg_bpm: Option<f64>,
}

/// One `LapMetrics` per lap, in lap order.
#[uniffi::export]
#[must_use]
pub fn laps(session: &Session) -> Vec<LapMetrics> {
    let pool = session.workout.pool_length_m;
    (0..session.laps.len())
        .map(|i| {
            let lap = session.laps[i];
            let lengths = session.lap_lengths[i];
            let distance_m = lengths * pool;
            let swim_s = lap.end_s - lap.start_s;
            let (stroke_rate_per_min, distance_per_stroke_m) =
                stroke_stats(&counted(session, i..i + 1));
            LapMetrics {
                index: i as u32,
                pos: session.lap_positions[i],
                stroke: lap.stroke,
                lengths,
                distance_m,
                start_s: lap.start_s,
                swim_s,
                pace_s_per_100m: swim_s / f64::from(distance_m) * 100.0,
                strokes: lap.strokes,
                swolf: lap_swolf(session, i),
                stroke_rate_per_min,
                distance_per_stroke_m,
                avg_bpm: hr_in(&session.hr, lap.start_s, lap.end_s).0,
            }
        })
        .collect()
}

/// Metrics of one set: the reps with the same block, round and set index.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SetMetrics {
    pub block: u32,
    pub round: u32,
    pub set: u32,
    pub reps: u32,
    pub distance_m: u32,
    pub swim_s: f64,
    /// Rest between the set's reps; the rest after its last rep is not included.
    pub rest_s: f64,
    pub pace_s_per_100m: f64,
    pub fastest_pace_s_per_100m: f64,
    pub slowest_pace_s_per_100m: f64,
    /// Last rep's pace minus first rep's pace; positive = slowing down.
    pub pace_drift_s_per_100m: f64,
    /// Over the samples inside the set's rep windows.
    pub avg_bpm: Option<f64>,
    pub max_bpm: Option<f64>,
    /// Reps with a `goal_delta_s`.
    pub goal_reps: u32,
    /// Reps with `goal_delta_s <= 0`.
    pub goal_hits: u32,
}

/// One `SetMetrics` per set with reps, in first-seen order.
#[uniffi::export]
#[must_use]
pub fn sets(session: &Session) -> Vec<SetMetrics> {
    let rs = reps(session);
    let groups = rep_groups(session);
    let key = |p: Position| (p.block, p.round, p.set);
    let mut keys: Vec<(u32, u32, u32)> = vec![];
    for r in &rs {
        if !keys.contains(&key(r.pos)) {
            keys.push(key(r.pos));
        }
    }
    keys.into_iter()
        .map(|k| {
            let idx: Vec<usize> = (0..rs.len()).filter(|&i| key(rs[i].pos) == k).collect();
            let mine: Vec<&RepMetrics> = idx.iter().map(|&i| &rs[i]).collect();
            let windows: Vec<(f64, f64)> = idx
                .iter()
                .map(|&i| {
                    (
                        session.laps[groups[i].start].start_s,
                        session.laps[groups[i].end - 1].end_s,
                    )
                })
                .collect();
            let (avg_bpm, max_bpm) = hr_stats(
                session
                    .hr
                    .iter()
                    .filter(|h| windows.iter().any(|w| h.t_s >= w.0 && h.t_s <= w.1))
                    .map(|h| h.bpm),
            );
            let swim_s: f64 = mine.iter().map(|r| r.swim_s).sum();
            let distance_m: u32 = mine.iter().map(|r| r.distance_m).sum();
            let paces = || mine.iter().map(|r| r.pace_s_per_100m);
            let last = mine.len() - 1;
            SetMetrics {
                block: k.0,
                round: k.1,
                set: k.2,
                reps: mine.len() as u32,
                distance_m,
                swim_s,
                rest_s: mine[..last]
                    .iter()
                    .filter_map(|r| r.rest_after_s)
                    .sum::<f64>(),
                pace_s_per_100m: swim_s / f64::from(distance_m) * 100.0,
                fastest_pace_s_per_100m: paces().fold(f64::INFINITY, f64::min),
                slowest_pace_s_per_100m: paces().fold(f64::NEG_INFINITY, f64::max),
                pace_drift_s_per_100m: mine[last].pace_s_per_100m - mine[0].pace_s_per_100m,
                avg_bpm,
                max_bpm,
                goal_reps: mine.iter().filter(|r| r.goal_delta_s.is_some()).count() as u32,
                goal_hits: mine
                    .iter()
                    .filter(|r| r.goal_delta_s.is_some_and(|d| d <= 0.0))
                    .count() as u32,
            }
        })
        .collect()
}

/// Total swim time over total distance of one set's reps, in s per 100 m; `None` if no reps.
#[must_use]
pub fn set_pace_s_per_100m(reps: &[RepMetrics], block: u32, round: u32, set: u32) -> Option<f64> {
    let (swim, distance) = reps
        .iter()
        .filter(|r| r.pos.block == block && r.pos.round == round && r.pos.set == set)
        .fold((0.0, 0u32), |(s, d), r| (s + r.swim_s, d + r.distance_m));
    (distance > 0).then(|| swim / f64::from(distance) * 100.0)
}

/// A run of laps in a free swim with no rest-sized gap between them.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FreeSet {
    pub start_s: f64,
    pub end_s: f64,
    pub lengths: u32,
    pub done: bool,
}

/// Gap that counts as rest. recorded-swim-b: turns 0-8 s, rests between reps 16-34 s; 12 s splits them.
pub(super) const REST_GAP_S: f64 = 12.0;

/// Splits laps (seconds since start) into sets at rest-sized gaps; the last set is done once its next lap is overdue at `now_s`.
#[uniffi::export]
pub fn free_sets(mut laps: Vec<Lap>, now_s: f64) -> Vec<FreeSet> {
    laps.sort_by(|a, b| a.end_s.total_cmp(&b.end_s));
    let mut durs: Vec<f64> = laps.iter().map(|l| l.end_s - l.start_s).collect();
    durs.sort_by(f64::total_cmp);
    let n = durs.len();
    // Lap time with no rest or turn: the median, so one stop or merged lap does not skew it.
    let typical = match n {
        0 => 30.0,
        _ if n % 2 == 1 => durs[n / 2],
        _ => f64::midpoint(durs[n / 2 - 1], durs[n / 2]),
    };
    let mut sets: Vec<FreeSet> = vec![];
    for (i, l) in laps.iter().enumerate() {
        // Gap to the previous lap: turns are a few seconds, so a new set means a rest.
        if i == 0 || l.start_s - laps[i - 1].end_s >= REST_GAP_S {
            sets.push(FreeSet {
                start_s: l.start_s,
                end_s: l.end_s,
                lengths: 0,
                done: true,
            });
        }
        if let Some(s) = sets.last_mut() {
            s.end_s = l.end_s;
            s.lengths += 1;
        }
    }
    // The last set is only over once the next lap is 20 s overdue (9 s delivery delay on top of a typical length).
    if let Some(last) = sets.last_mut() {
        last.done = now_s > last.end_s + typical + 9.0 + 20.0;
    }
    sets
}
