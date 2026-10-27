use super::{
    COST_EXTRA, COST_MISSING, COST_NO_REST, COST_PAUSE, COST_STROKE, Engine, Lap, Position,
    REST_GAP_S, Stroke, TYPICAL_WINDOW,
};

/// Alignment table: cheapest cost and back pointer per state (laps placed, reps over).
pub(super) struct Table {
    width: usize,
    best: Vec<f64>,
    from: Vec<(usize, usize)>,
}

impl Table {
    fn new(laps: usize, reps: usize) -> Self {
        let width = reps + 1;
        let mut best = vec![f64::INFINITY; (laps + 1) * width];
        best[0] = 0.0;
        Self {
            width,
            best,
            from: vec![(0, 0); (laps + 1) * width],
        }
    }

    fn cost(&self, laps: usize, reps_over: usize) -> f64 {
        self.best[laps * self.width + reps_over]
    }

    /// Records `cost` for the state (`laps_placed`, one more rep over) if it beats the known one.
    fn relax(&mut self, (i, r): (usize, usize), laps_placed: usize, cost: f64) {
        let state = laps_placed * self.width + r + 1;
        if cost < self.best[state] {
            self.best[state] = cost;
            self.from[state] = (i, r);
        }
    }

    /// The rep index per lap on the cheapest path that places all `laps`.
    fn backtrack(&self, laps: usize) -> Vec<usize> {
        let mut r = (0..self.width)
            .min_by(|&a, &b| self.cost(laps, a).total_cmp(&self.cost(laps, b)))
            .unwrap_or(0);
        let mut i = laps;
        let mut out = vec![0; laps];
        while (i, r) != (0, 0) {
            let (pi, pr) = self.from[i * self.width + r];
            out[pi..i].fill(pr);
            (i, r) = (pi, pr);
        }
        out
    }
}

impl Engine {
    #[expect(clippy::expect_used, reason = "state positions come from the plan")]
    pub(super) fn index_of(&self, pos: Position) -> usize {
        self.reps
            .iter()
            .position(|r| r.pos == pos)
            .expect("state positions come from the plan")
    }

    /// Median duration of one length over the last few laps.
    pub(super) fn typical_length_s(&self) -> Option<f64> {
        let mut d: Vec<f64> = (self
            .laps
            .iter()
            .zip(&self.lengths)
            .rev()
            .take(TYPICAL_WINDOW))
        .map(|(l, &n)| (l.end_s - l.start_s) / f64::from(n))
        .collect();
        d.sort_by(f64::total_cmp);
        d.get(d.len() / 2).copied()
    }

    pub(super) fn gap_before(&self, lap: usize) -> f64 {
        self.laps[lap].start_s - self.laps[lap - 1].end_s
    }

    /// Manual marks are hard limits: laps before a mark belong to the closed rep or earlier, laps
    /// after it to a later rep (or count as extra on the last rep once the workout is done).
    pub(super) fn allowed(&self, lap: usize, rep: usize) -> bool {
        let m = mid(&self.laps[lap]);
        self.marks.iter().all(|&(t, closed)| {
            if m <= t {
                rep <= closed
            } else {
                rep > closed || closed + 1 == self.reps.len()
            }
        })
    }

    /// Cheapest split of all laps into consecutive reps. Returns the rep index per lap.
    pub(super) fn solve(&self) -> Vec<usize> {
        let (n, r_count) = (self.laps.len(), self.reps.len());
        let mut table = Table::new(n, r_count);
        for i in 0..=n {
            for r in 0..r_count {
                let base = table.cost(i, r);
                if base.is_infinite() {
                    continue;
                }
                // Rep r without any detected lap.
                let missing = COST_MISSING.mul_add(f64::from(self.reps[r].lengths), base);
                table.relax((i, r), i, missing);
                self.place_laps(&mut table, i, r, base);
            }
        }
        table.backtrack(n)
    }

    /// Relaxes every state reached by laps `i..=j` being rep `r`, starting from cost `base`.
    fn place_laps(&self, table: &mut Table, i: usize, r: usize, base: f64) {
        let (n, planned) = (self.laps.len(), self.reps[r].lengths);
        let mut cost = base;
        if i > 0 && i < n && self.gap_before(i) < REST_GAP_S / 2.0 {
            cost += COST_NO_REST;
        }
        let mut len = 0;
        for j in i..n {
            if !self.allowed(j, r) {
                break;
            }
            len += self.lengths[j];
            if j > i && self.gap_before(j) >= REST_GAP_S {
                cost += COST_PAUSE * self.gap_before(j) / REST_GAP_S;
            }
            if contradicts(self.laps[j].stroke, self.reps[r].stroke) {
                cost += COST_STROKE;
            }
            // The rep being swum may still get more lengths.
            let missing = if j + 1 == n {
                0
            } else {
                planned.saturating_sub(len)
            };
            let total = COST_MISSING.mul_add(
                f64::from(missing),
                COST_EXTRA.mul_add(f64::from(len.saturating_sub(planned)), cost),
            );
            table.relax((i, r), j + 1, total);
        }
    }

    pub(super) fn groups(&self) -> Vec<Group> {
        let mut groups: Vec<Group> = vec![];
        for (k, (lap, &rep)) in self.laps.iter().zip(&self.assigned).enumerate() {
            match groups.last_mut() {
                Some(g) if g.rep == rep => {
                    g.lengths += self.lengths[k];
                    g.end_s = lap.end_s;
                }
                _ => groups.push(Group {
                    rep,
                    lengths: self.lengths[k],
                    start_s: lap.start_s,
                    end_s: lap.end_s,
                }),
            }
        }
        groups
    }
}

/// Laps of one rep.
pub(super) struct Group {
    pub(super) rep: usize,
    pub(super) lengths: u32,
    pub(super) start_s: f64,
    pub(super) end_s: f64,
}

pub(super) const fn mid(lap: &Lap) -> f64 {
    f64::midpoint(lap.start_s, lap.end_s)
}

/// Only clear stroke errors count: drills read as `kickboard` and `any`/`mixed` match everything.
pub(super) fn contradicts(detected: Stroke, planned: Stroke) -> bool {
    use Stroke::{Backstroke, Breaststroke, Butterfly, Freestyle};
    let clear = |s| matches!(s, Freestyle | Backstroke | Breaststroke | Butterfly);
    clear(detected) && clear(planned) && detected != planned
}
