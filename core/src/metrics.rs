//! Per-rep and per-set numbers from a swum session. Pure: uses the reps the engine already assigned.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::engine::{Phase, START_COUNTDOWN_S, State};
use crate::model::{
    Equipment, HrSample, Intensity, Lap, Position, RestKind, Session, Stroke, Workout, WorkoutSet,
};
use crate::store::StoredSession;

mod css;
mod estimate;
mod profile;
mod records;
mod reps;
mod summary;

pub use css::*;
pub use estimate::*;
pub use profile::*;
pub use records::*;
pub use reps::*;
pub use summary::*;
#[cfg(test)]
mod tests;
