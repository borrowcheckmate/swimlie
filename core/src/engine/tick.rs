use super::{
    Engine, Equipment, Group, LAP_DELAY_MAX_S, Phase, REST_GAP_S, RepResult, RestKind,
    START_COUNTDOWN_S, State, Stroke, Verdict, canonical_equipment, mid,
};

impl Engine {
    #[must_use]
    pub fn tick(&self, now_s: f64) -> State {
        let groups = self.groups();
        let pool = self.workout.pool_length_m;
        let swum_m = self.lengths.iter().sum::<u32>() * pool;

        let closed = self.closed_rep(&groups);
        let pace_group = match (closed, groups.last()) {
            (Some(_), g) => g,
            (None, _) => groups.iter().rev().nth(1),
        };
        let last_rep_pace_s_per_100m =
            pace_group.map(|g| (g.end_s - g.start_s) / f64::from(g.lengths * pool) * 100.0);
        let last_rep = closed.and_then(|(c, _)| self.rep_result(&groups, c));

        let (rep, done, rest_end) = match closed {
            Some((c, _)) if c + 1 == self.reps.len() => {
                let p = &self.reps[c];
                return State {
                    pos: p.pos,
                    done_m: p.lengths * pool,
                    target_m: p.lengths * pool,
                    phase: Phase::Done,
                    rest_remaining_s: None,
                    last_rep_pace_s_per_100m,
                    rep_elapsed_s: None,
                    swum_m,
                    last_rep,
                };
            }
            Some((c, end)) => (c + 1, 0, self.rest_end_after(&groups, c, end)),
            None => (
                groups.last().map_or(0, |g| g.rep),
                groups.last().map_or(0, |g| g.lengths),
                groups.is_empty().then(|| {
                    self.go_s
                        .or_else(|| self.start_reset_s.map(|t| t + START_COUNTDOWN_S))
                        .unwrap_or(START_COUNTDOWN_S)
                }),
            ),
        };
        let p = &self.reps[rep];
        let paused = self.pauses.last().is_some_and(|p| p.1.is_none());
        let rest_remaining_s = rest_end.map(|e| e - now_s).filter(|&r| r > 0.0 && !paused);
        let phase = if rest_remaining_s.is_some() || paused || self.is_stopped(now_s, rest_end) {
            Phase::Resting
        } else {
            Phase::Swimming
        };
        // Rep start: its first lap, else when the rest ended (or the previous rep, with no rest).
        let start = match groups.last() {
            Some(g) if g.rep == rep => g.start_s,
            _ => rest_end.or_else(|| closed.map(|c| c.1)).unwrap_or(0.0),
        };
        // A manual restart of this rep's clock counts from its time, laps already swum stay counted.
        let start = self
            .rep_restarts
            .iter()
            .rev()
            .find(|x| x.1 == rep)
            .map_or(start, |x| x.0);
        State {
            pos: p.pos,
            done_m: done * pool,
            target_m: p.lengths * pool,
            phase,
            rest_remaining_s,
            last_rep_pace_s_per_100m,
            rep_elapsed_s: (phase == Phase::Swimming).then_some(now_s - start),
            swum_m,
            last_rep,
        }
    }

    /// The latest rep known to be finished, and when it finished.
    fn closed_rep(&self, groups: &[Group]) -> Option<(usize, f64)> {
        if let Some(&(t, rep)) = self.marks.last()
            && self.laps.last().is_none_or(|l| mid(l) <= t)
        {
            return Some((rep, t));
        }
        groups
            .last()
            .filter(|g| g.lengths >= self.reps[g.rep].lengths)
            .map(|g| (g.rep, g.end_s))
    }

    /// When the rest after rep `c` (finished at `end`) is over, with manual skips applied.
    fn rest_end_after(&self, groups: &[Group], c: usize, end: f64) -> Option<f64> {
        let start = groups
            .last()
            .filter(|g| g.rep == c)
            .map_or(end, |g| g.start_s);
        let rest = self.reps[c].rest.as_ref()?;
        let base = self.rest_resets.iter().rev().find(|x| x.1 == c).map_or(
            match rest.kind {
                RestKind::Fixed => end,
                RestKind::Interval => start,
            },
            |x| x.0,
        );
        // A manual "next" during this rest ends it at the skip time.
        Some(
            self.rest_skips
                .iter()
                .filter(|x| x.1 == c)
                .fold(base + f64::from(rest.seconds), |e, x| e.min(x.0)),
        )
    }

    /// Unplanned stop: the next lap is `REST_GAP_S` overdue, with no strokes or distance since either.
    fn is_stopped(&self, now_s: f64, rest_end: Option<f64>) -> bool {
        let active_since = self
            .laps
            .last()
            .map(|l| l.end_s)
            .into_iter()
            .chain(rest_end)
            .chain(self.active_s)
            .reduce(f64::max);
        active_since
            .zip(self.typical_length_s())
            .is_some_and(|(a, t)| {
                now_s - self.paused_s(a, now_s) > a + t + LAP_DELAY_MAX_S + REST_GAP_S
            })
    }

    /// The finished rep `c`, timed against its goal and the best known time.
    pub(super) fn rep_result(&self, groups: &[Group], c: usize) -> Option<RepResult> {
        let g = groups.iter().find(|g| g.rep == c)?;
        let pos = self.reps[c].pos;
        let swim_s = g.end_s - g.start_s;
        let goal = self.workout.blocks[pos.block as usize].sets[pos.set as usize].goal_s;
        let fastest = self.record_key(g).is_some_and(|key| {
            let earlier = groups
                .iter()
                .filter(|h| h.rep < c && self.record_key(h).as_ref() == Some(&key));
            let best = self
                .records
                .iter()
                .filter(|p| (p.distance_m, p.stroke, &p.equipment) == (key.0, key.1, &key.2))
                .map(|p| p.time_s)
                .chain(earlier.map(|h| h.end_s - h.start_s))
                .min_by(f64::total_cmp);
            best.is_some_and(|b| swim_s < b)
        });
        let verdict = if fastest {
            Some(Verdict::Fastest)
        } else {
            goal.map(|t| {
                if swim_s <= f64::from(t) {
                    Verdict::Faster
                } else {
                    Verdict::Slower
                }
            })
        };
        let (sum, n) = self
            .hr
            .iter()
            .filter(|h| (g.start_s..=g.end_s).contains(&h.t_s))
            .fold((0.0, 0), |(s, n), h| (s + h.bpm, n + 1));
        let avg_bpm = (n > 0).then(|| sum / f64::from(n));
        Some(RepResult {
            pos,
            swim_s,
            goal_diff_s: goal.map(|t| swim_s - f64::from(t)),
            verdict,
            avg_bpm,
        })
    }

    /// Distance, stroke and equipment of a rep that can set a record: complete, no merged lap, not an exercise.
    pub(super) fn record_key(&self, g: &Group) -> Option<(u32, Stroke, Vec<Equipment>)> {
        let planned = &self.reps[g.rep];
        let set = &self.workout.blocks[planned.pos.block as usize].sets[planned.pos.set as usize];
        if set.drill.is_some() {
            return None;
        }
        let distance_m = planned.lengths * self.workout.pool_length_m;
        let idx: Vec<usize> = (0..self.laps.len())
            .filter(|&k| self.assigned[k] == g.rep)
            .collect();
        let clean = g.lengths == planned.lengths && idx.iter().all(|&k| self.lengths[k] == 1);
        if !clean {
            return None;
        }
        let stroke = match planned.stroke {
            Stroke::Any => {
                let first = self.laps[idx[0]].stroke;
                if idx.iter().any(|&k| self.laps[k].stroke != first) {
                    return None;
                }
                first
            }
            s => s,
        };
        Some((distance_m, stroke, canonical_equipment(&set.equipment)))
    }
}
