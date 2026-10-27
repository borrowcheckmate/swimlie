//! SQLite store for workouts and swum sessions. Pure: the caller supplies the DB path.

use crate::metrics::{PersonalRecord, css_s_per_100m, css_test_times};
use crate::model::{
    HrSample, Lap, Location, Plan, Position, Session, Stroke, Workout, parse_plan, parse_workout,
    plan_to_json, workout_to_json,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

mod crud;
mod css;
mod decode_session;
mod rows;
mod sync;

use decode_session::to_session;
use rows::{
    Row, Stamps, head_row, incoming_wins, json_of, lap_row, mark_deleted, now_ms, parse_stroke,
    plan_rows, session_rows, soft_delete, timestamps, timestamps_of, to_plan, to_workout, u32_of,
    validate_session, workout_rows, write_plan, write_session, write_workout,
};
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum StoreError {
    #[error("database error: {msg}")]
    Db { msg: String },
    /// Data read from the database failed validation.
    #[error("corrupt data: {msg}")]
    Corrupt { msg: String },
    /// Data passed to a save call failed validation.
    #[error("invalid input: {msg}")]
    Invalid { msg: String },
    #[error("database schema version {found} is newer than supported {supported}")]
    TooNew { found: i64, supported: i64 },
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db { msg: e.to_string() }
    }
}

fn corrupt(msg: impl Into<String>) -> StoreError {
    StoreError::Corrupt { msg: msg.into() }
}

fn invalid(msg: impl Into<String>) -> StoreError {
    StoreError::Invalid { msg: msg.into() }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoredWorkout {
    pub id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub workout: Workout,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StoredPlan {
    pub id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct StoredSession {
    pub id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub session: Session,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredCss {
    pub id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub t400_s: f64,
    pub t200_s: f64,
}

/// Wire format of `export_changes`/`apply_changes`. Tombstones are rows with `deleted_at_ms` set.
#[derive(Serialize, Deserialize)]
struct SyncBatch {
    #[serde(default)]
    workouts: Vec<SyncRow<Workout>>,
    #[serde(default)]
    sessions: Vec<SyncRow<Session>>,
    #[serde(default)]
    plans: Vec<SyncRow<Plan>>,
}

/// Incoming batch with rows left raw, so one bad row can be skipped without failing the rest.
#[derive(Deserialize)]
struct RawBatch {
    #[serde(default)]
    workouts: Vec<serde_json::Value>,
    #[serde(default)]
    sessions: Vec<serde_json::Value>,
    #[serde(default)]
    plans: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct SyncRow<T> {
    id: String,
    created_at_ms: i64,
    updated_at_ms: i64,
    deleted_at_ms: Option<i64>,
    data: T,
}

#[derive(Debug, uniffi::Object)]
pub struct Store {
    // Mutex because Connection is !Sync and UniFFI objects are shared.
    conn: Mutex<Connection>,
}

impl Store {
    /// The connection. A poisoned lock is still safe to use: every write is one transaction.
    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Ordered migrations: entry N takes the schema from version N to N + 1. Append one per schema change.
/// Deletes are soft (`deleted_at` set, `updated_at` bumped) so a tombstone can win last-write-wins sync.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE workouts (
        id TEXT PRIMARY KEY, json TEXT NOT NULL,
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER);
    CREATE TABLE sessions (
        id TEXT PRIMARY KEY, workout_json TEXT NOT NULL, workout_id TEXT, started_at_ms INTEGER NOT NULL,
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER);
    CREATE TABLE laps (
        session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        idx INTEGER NOT NULL, start_s REAL NOT NULL, end_s REAL NOT NULL, stroke TEXT NOT NULL,
        strokes INTEGER, lengths INTEGER NOT NULL,
        block INTEGER NOT NULL, round INTEGER NOT NULL, set_idx INTEGER NOT NULL, rep INTEGER NOT NULL,
        PRIMARY KEY (session_id, idx));
    CREATE TABLE css_results (
        id TEXT PRIMARY KEY, t400_s REAL NOT NULL, t200_s REAL NOT NULL,
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER);
", "
    ALTER TABLE sessions ADD COLUMN hr_json TEXT;
    ALTER TABLE sessions ADD COLUMN age_years INTEGER;
", "
    ALTER TABLE sessions ADD COLUMN location_json TEXT;
", "
    CREATE TABLE plans (id TEXT PRIMARY KEY, json TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER);
"];

#[cfg(test)]
mod tests;
