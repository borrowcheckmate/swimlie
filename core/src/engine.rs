//! Live set engine: `HealthKit` laps in, "where am I" out.
//!
//! `HealthKit` lengths go missing, merge and arrive 5–9 s late (spike 0b, PLAN.md section 3), so the
//! engine never counts laps blindly. After every lap it re-aligns all laps so far with the planned
//! reps (a small dynamic program) using lap counts, lap times, rest gaps and strokes. One missed
//! length then costs one short rep instead of shifting every rep after it. All times are seconds
//! since the session started, on the same clock as the lap timestamps.

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::metrics::{PersonalRecord, canonical_equipment};
use crate::model::{
    Equipment, HrSample, Lap, Position, Rest, RestKind, Session, Stroke, Workout, WorkoutError,
};

mod align;
mod tick;

use align::{Group, mid};

/// A gap between laps this long is a rest, not a turn.
const REST_GAP_S: f64 = 20.0;
/// Slowest lap event seen in spike 0b.
const LAP_DELAY_MAX_S: f64 = 9.0;
/// A lap this many times the typical length is slow: two merged lengths, or a slow stroke.
const MERGED_FACTOR: f64 = 1.7;
/// Beyond this a long lap is one length plus a pause at the wall (e.g. rest), not two merged lengths.
const PAUSED_FACTOR: f64 = 2.6;
/// Laps used for the typical length duration.
const TYPICAL_WINDOW: usize = 8;
/// Faster than any human swims 100 m; a lap this fast is a `HealthKit` artefact.
const MIN_PACE_S_PER_100M: f64 = 40.0;

/// Planned rest before the first rep, so the swimmer can get ready at the wall.
pub const START_COUNTDOWN_S: f64 = 10.0;

// Alignment costs. Missing lengths are common, invented ones are not, and mid-rep pauses get
// less likely the longer they are.
const COST_MISSING: f64 = 1.0; // per planned length without a lap
const COST_EXTRA: f64 = 2.0; // per length beyond the plan
const COST_PAUSE: f64 = 1.0; // per REST_GAP_S of a rest-sized gap inside a rep
const COST_NO_REST: f64 = 0.5; // next rep starts without a rest
const COST_STROKE: f64 = 0.5; // per lap whose detected stroke contradicts the plan

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Phase {
    Swimming,
    Resting,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Verdict {
    Fastest,
    Faster,
    Slower,
}

/// The latest finished rep, shown during the rest after it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RepResult {
    pub pos: Position,
    /// First lap start to last lap end.
    pub swim_s: f64,
    /// `swim_s` minus the set's goal; negative is faster. None without a goal.
    pub goal_diff_s: Option<f64>,
    pub verdict: Option<Verdict>,
    /// Mean heart rate while swimming the rep; `None` without samples.
    pub avg_bpm: Option<f64>,
}

/// What the watch shows. While resting, `pos` is the rep that comes next.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct State {
    pub pos: Position,
    pub done_m: u32,
    pub target_m: u32,
    pub phase: Phase,
    /// Countdown of a planned rest; `None` for a rest the swimmer took on their own.
    pub rest_remaining_s: Option<f64>,
    pub last_rep_pace_s_per_100m: Option<f64>,
    /// Seconds since the current rep started; `None` while resting or done.
    pub rep_elapsed_s: Option<f64>,
    /// Metres swum so far (lengths x pool length).
    pub swum_m: u32,
    /// The latest finished rep; `None` before the first one or when it has no laps.
    pub last_rep: Option<RepResult>,
}

/// Seconds of prep left after a manual "next" skips a planned rest.
const SKIP_PREP_S: f64 = 1.0;
/// Back within this many seconds of a rep's start goes to the break again; later it restarts the rep clock.
const BACK_GRACE_S: f64 = 3.0;

/// Kind of manual action, in the order the swimmer made them; `manual_back` undoes the latest.
#[derive(Debug)]
enum Action {
    Mark,
    Skip,
    /// The rep being swum starts its clock over (`manual_back`).
    Restart,
    /// Not a swimmer action: the rest a `manual_back` landed in restarts in full.
    Reset,
}

#[derive(Debug)]
struct PlannedRep {
    pos: Position,
    lengths: u32,
    stroke: Stroke,
    rest: Option<Rest>,
}

#[derive(Debug)]
pub struct Engine {
    workout: Workout,
    started_at_ms: i64,
    workout_id: Option<String>,
    reps: Vec<PlannedRep>,
    laps: Vec<Lap>,
    /// Laps at least `MERGED_FACTOR` times the typical length when they arrived.
    slow: Vec<bool>,
    /// Lengths per lap (1, or 2 for a merged lap), re-decided with every lap.
    lengths: Vec<u32>,
    /// Manual "rep done": (time, rep index closed at that time).
    marks: Vec<(f64, usize)>,
    /// Manual "next" during a planned rest: (time, rep whose rest ended at that time).
    rest_skips: Vec<(f64, usize)>,
    /// Rest restarts after an undo: (time, rep whose rest counts from that time).
    rest_resets: Vec<(f64, usize)>,
    /// Rep clock restarts: (time, rep whose rep clock counts from that time).
    rep_restarts: Vec<(f64, usize)>,
    /// Order of all manual actions, for `manual_back`.
    log: Vec<Action>,
    /// Rep index per lap, re-solved after every lap or mark.
    assigned: Vec<usize>,
    /// When a manual "next" ended the start countdown early.
    go_s: Option<f64>,
    /// Back during the first rep's first seconds restarts the start countdown from here.
    start_reset_s: Option<f64>,
    /// Best known times from earlier sessions, for the rep verdicts.
    records: Vec<PersonalRecord>,
    /// Heart-rate samples in time order.
    hr: Vec<HrSample>,
    /// Latest sign of swimming other than a lap (stroke or distance samples); laps can arrive late.
    active_s: Option<f64>,
    /// Swimming again after an unplanned stop: (time, rep whose clock counts from then until its first lap arrives).
    resumed: Option<(f64, usize)>,
    /// Pause windows (start, end); `end` is `None` while paused.
    pauses: Vec<(f64, Option<f64>)>,
}

/// Every rep of the workout in swimming order: blocks, rounds, sets, reps.
fn planned_reps(workout: &Workout) -> Vec<PlannedRep> {
    let positions = workout.blocks.iter().enumerate().flat_map(|(b, block)| {
        (0..block.repeat).flat_map(move |round| {
            block
                .sets
                .iter()
                .enumerate()
                .map(move |(s, set)| (b, round, s, set))
        })
    });
    positions
        .flat_map(|(b, round, s, set)| {
            (0..set.reps).map(move |rep| PlannedRep {
                pos: Position {
                    block: b as u32,
                    round,
                    set: s as u32,
                    rep,
                },
                lengths: set.distance_m / workout.pool_length_m,
                stroke: set.stroke,
                rest: set.rest.clone(),
            })
        })
        .collect()
}

impl Engine {
    pub fn new(
        workout: Workout,
        workout_id: Option<String>,
        started_at_ms: i64,
        records: Vec<PersonalRecord>,
    ) -> Result<Self, WorkoutError> {
        workout.validate()?;
        let reps = planned_reps(&workout);
        Ok(Self {
            workout,
            started_at_ms,
            workout_id,
            reps,
            laps: vec![],
            slow: vec![],
            lengths: vec![],
            marks: vec![],
            rest_skips: vec![],
            rest_resets: vec![],
            rep_restarts: vec![],
            log: vec![],
            assigned: vec![],
            go_s: None,
            start_reset_s: None,
            records,
            hr: vec![],
            active_s: None,
            resumed: None,
            pauses: vec![],
        })
    }

    /// A heart-rate sample at `t_s`. Dropped if non-finite, bpm <= 0, or earlier than the last sample:
    /// a wall-clock step back would break the time order `save_session` checks.
    pub fn on_heart_rate(&mut self, t_s: f64, bpm: f64) {
        let in_order = self.hr.last().is_none_or(|h| t_s >= h.t_s);
        if t_s.is_finite() && bpm.is_finite() && bpm > 0.0 && in_order {
            self.hr.push(HrSample { t_s, bpm });
        }
    }

    /// Stroke or distance data arrived at `t_s`: the swimmer is moving even if no lap has been reported.
    pub fn on_activity(&mut self, t_s: f64) {
        if !t_s.is_finite() {
            return;
        }
        // Moving again after an unplanned stop: the rep started now, not when the rep before it ended.
        let s = self.tick(t_s);
        let paused = self.pauses.last().is_some_and(|p| p.1.is_none());
        if s.phase == Phase::Resting && s.rest_remaining_s.is_none() && !paused {
            self.resumed = Some((t_s, self.index_of(s.pos)));
        }
        self.active_s = Some(self.active_s.map_or(t_s, |a| a.max(t_s)));
    }

    /// The `HealthKit` session was paused or resumed at `now_s`. Paused time never counts as an unplanned stop.
    pub fn set_paused(&mut self, paused: bool, now_s: f64) {
        match (self.pauses.last_mut(), paused) {
            (Some(p), false) if p.1.is_none() => p.1 = Some(now_s),
            (Some(p), true) if p.1.is_none() => {}
            (_, true) => self.pauses.push((now_s, None)),
            _ => {}
        }
    }

    /// Seconds of pause between `from` and `to`.
    fn paused_s(&self, from: f64, to: f64) -> f64 {
        self.pauses
            .iter()
            .map(|&(s, e)| (e.unwrap_or(to).min(to) - s.max(from)).max(0.0))
            .sum()
    }

    /// A `HealthKit` lap event, received at `now_s`. Laps must arrive in order (`HealthKit` does).
    pub fn on_lap(&mut self, lap: Lap, now_s: f64) -> State {
        // Not a swum length: leave it out of the session.
        if lap.end_s - lap.start_s
            < MIN_PACE_S_PER_100M * f64::from(self.workout.pool_length_m) / 100.0
        {
            return self.tick(now_s);
        }
        // A length that ended while the planned rest was still counting down is a HealthKit ghost (the
        // swimmer hangs on the wall): left in, it starts the next rep and hides the rest.
        // shortcut: skipped once manual actions exist (a late lap of the closed rep ends inside its rest).
        if self.log.is_empty() && self.tick(lap.end_s).rest_remaining_s.is_some() {
            return self.tick(now_s);
        }
        let typical = self.typical_length_s();
        self.slow.push(typical.is_some_and(|t| {
            (MERGED_FACTOR * t..PAUSED_FACTOR * t).contains(&(lap.end_s - lap.start_s))
        }));
        self.laps.push(lap);
        // A slow lap between normal ones is two merged lengths; a run of slow laps is a slow stroke
        // (kick, drill) with one length each. The next lap can overturn the call, so redo them all.
        // shortcut: at most 2 lengths per lap (all spike 0b saw); a longer lap is a pause at the wall.
        let slow = |k: usize| self.slow.get(k).copied().unwrap_or(false);
        self.lengths = (0..self.laps.len())
            .map(|k| 1 + u32::from(slow(k) && !(k > 0 && slow(k - 1)) && !slow(k + 1)))
            .collect();
        self.assigned = self.solve();
        self.tick(now_s)
    }

    /// Manual "next" for when detection fails. Outside a planned rest it closes the rep on screen at
    /// `now_s`; during a planned rest it ends the rest `SKIP_PREP_S` later, and the rep it leads to stays.
    pub fn manual_next(&mut self, now_s: f64) -> State {
        let state = self.tick(now_s);
        // During the start countdown "next" means go, not "rep 1 done".
        if self.laps.is_empty()
            && self.marks.is_empty()
            && self.go_s.is_none()
            && state.rest_remaining_s.is_some()
        {
            self.go_s = Some(now_s);
            return self.tick(now_s);
        }
        if state.phase != Phase::Done {
            let idx = self.index_of(state.pos);
            if let Some(remaining) = state.rest_remaining_s
                && idx > 0
            {
                // A rest with less prep left than that stays as it is.
                if remaining > SKIP_PREP_S {
                    self.rest_skips.push((now_s + SKIP_PREP_S, idx - 1));
                    self.log.push(Action::Skip);
                }
            } else {
                self.marks.push((now_s, idx));
                self.log.push(Action::Mark);
                self.assigned = self.solve();
            }
        }
        self.tick(now_s)
    }

    /// Undoes the latest manual action; `None` when there is nothing to undo.
    /// Right after a rest skip, the rest counts in full again. While swimming, the rep clock restarts;
    /// a second back right after that undoes the action before it.
    pub fn manual_back(&mut self, now_s: f64) -> Option<State> {
        let state = self.tick(now_s);
        let swimming = state.phase == Phase::Swimming;
        let restart = |e: &mut Self| {
            e.rep_restarts.push((now_s, e.index_of(state.pos)));
            e.log.push(Action::Restart);
            Some(e.tick(now_s))
        };
        // Past the grace period of a rep: back resets its clock to 0:00.
        if swimming && state.rep_elapsed_s.is_some_and(|t| t > BACK_GRACE_S) {
            return restart(self);
        }
        if matches!(self.log.last(), Some(Action::Skip))
            && let Some(&(end, c)) = self.rest_skips.last()
            && self.laps.last().is_none_or(|l| l.end_s <= end)
        {
            self.log.pop();
            self.rest_skips.pop();
            self.rest_resets.push((now_s, c));
            self.log.push(Action::Reset);
            return Some(self.tick(now_s));
        }
        // A restart alone is nothing to go back to: the old clock must not come back.
        let undoable = self
            .log
            .iter()
            .any(|a| !matches!(a, Action::Reset | Action::Restart));
        if undoable {
            // Reset entries go with the action that follows them.
            let mut solve = false;
            loop {
                match self.log.pop() {
                    Some(Action::Reset) => {
                        self.rest_resets.pop();
                    }
                    Some(Action::Restart) => {
                        self.rep_restarts.pop();
                    }
                    Some(Action::Mark) => {
                        self.marks.pop();
                        solve = true;
                        break;
                    }
                    Some(Action::Skip) => {
                        self.rest_skips.pop();
                        break;
                    }
                    None => break,
                }
            }
            if solve {
                self.assigned = self.solve();
            }
            // Landing in a planned rest: it counts in full from now.
            let s = self.tick(now_s);
            if s.rest_remaining_s.is_some() && s.phase == Phase::Resting && self.index_of(s.pos) > 0
            {
                self.rest_resets.push((now_s, self.index_of(s.pos) - 1));
                self.log.push(Action::Reset);
            }
            return Some(self.tick(now_s));
        }
        if self.go_s.is_some() && self.laps.is_empty() {
            self.go_s = None;
            // Back at the start: earlier clock resets of the first rep must not count in the next go.
            self.rep_restarts.clear();
            self.log.clear();
            self.start_reset_s = Some(now_s);
            return Some(self.tick(now_s));
        }
        if swimming {
            return restart(self); // nothing to undo: just reset the clock
        }
        None
    }
}

impl Engine {
    #[must_use]
    pub fn session(&self) -> Session {
        Session {
            workout: self.workout.clone(),
            started_at_ms: self.started_at_ms,
            workout_id: self.workout_id.clone(),
            laps: self.laps.clone(),
            lap_positions: self.assigned.iter().map(|&r| self.reps[r].pos).collect(),
            lap_lengths: self.lengths.clone(),
            hr: self.hr.clone(),
            age_years: None,
            location: None,
        }
    }
}

/// Swift-facing engine: the pure `Engine` behind a lock, since `UniFFI` objects are shared.
#[derive(Debug, uniffi::Object)]
pub struct LiveEngine(Mutex<Engine>);

impl LiveEngine {
    /// The engine. A poisoned lock is still safe to use: every call leaves the engine consistent.
    fn engine(&self) -> MutexGuard<'_, Engine> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[uniffi::export]
impl LiveEngine {
    #[uniffi::constructor]
    pub fn new(
        workout: Workout,
        workout_id: Option<String>,
        started_at_ms: i64,
        records: Vec<PersonalRecord>,
    ) -> Result<Self, WorkoutError> {
        Ok(Self(Mutex::new(Engine::new(
            workout,
            workout_id,
            started_at_ms,
            records,
        )?)))
    }

    pub fn on_lap(&self, lap: Lap, now_s: f64) -> State {
        self.engine().on_lap(lap, now_s)
    }

    pub fn on_heart_rate(&self, t_s: f64, bpm: f64) {
        self.engine().on_heart_rate(t_s, bpm);
    }

    pub fn on_activity(&self, t_s: f64) {
        self.engine().on_activity(t_s);
    }

    pub fn set_paused(&self, paused: bool, now_s: f64) {
        self.engine().set_paused(paused, now_s);
    }

    pub fn manual_next(&self, now_s: f64) -> State {
        self.engine().manual_next(now_s)
    }

    pub fn manual_back(&self, now_s: f64) -> Option<State> {
        self.engine().manual_back(now_s)
    }

    pub fn tick(&self, now_s: f64) -> State {
        self.engine().tick(now_s)
    }

    pub fn session(&self) -> Session {
        self.engine().session()
    }
}

#[cfg(test)]
mod tests;
