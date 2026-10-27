use super::*;

#[test]
fn workout_round_trip_update_delete() {
    let s = Store::open(":memory:").unwrap();
    let a = s.save_workout(None, &workout()).unwrap();
    assert_eq!(s.get_workout(&a.id).unwrap().unwrap(), a);
    let mut w2 = workout();
    w2.name = "Renamed".into();
    let b = s.save_workout(Some(a.id.clone()), &w2).unwrap();
    assert_eq!(b.created_at_ms, a.created_at_ms);
    assert!(b.updated_at_ms > a.updated_at_ms);
    assert_eq!(
        s.get_workout(&a.id).unwrap().unwrap().workout.name,
        "Renamed"
    );
    assert!(s.delete_workout(&a.id).unwrap());
    assert!(!s.delete_workout(&a.id).unwrap());
    assert!(s.get_workout(&a.id).unwrap().is_none());
    assert_eq!(s.list_workouts().unwrap(), Vec::<StoredWorkout>::new());
    // Tombstone stays with a newer updated_at, for sync.
    let (deleted, updated): (Option<i64>, i64) = s
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT deleted_at, updated_at FROM workouts WHERE id = ?1",
            [&a.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(updated > b.updated_at_ms && deleted == Some(updated));
    // Saving the id again revives it.
    let c = s.save_workout(Some(a.id.clone()), &workout()).unwrap();
    assert!(c.updated_at_ms > updated && c.created_at_ms == a.created_at_ms);
    assert_eq!(s.get_workout(&a.id).unwrap().unwrap(), c);
}

#[test]
fn seed_workout_inserts_once() {
    let s = Store::open(":memory:").unwrap();
    assert!(s.seed_workout("sample".into(), &workout()).unwrap());
    assert!(!s.seed_workout("sample".into(), &workout()).unwrap());
    assert_eq!(s.list_workouts().unwrap().len(), 1);
    // A deleted seed is a tombstone, so it is not re-seeded.
    assert!(s.delete_workout("sample").unwrap());
    assert!(!s.seed_workout("sample".into(), &workout()).unwrap());
    assert!(s.get_workout("sample").unwrap().is_none());
}

#[test]
fn seeded_workout_loses_to_real_save() {
    let seeded = Store::open(":memory:").unwrap();
    assert!(seeded.seed_workout("sample".into(), &workout()).unwrap());
    let edited = Store::open(":memory:").unwrap();
    let mut w = workout();
    w.name = "Edited".into();
    edited.save_workout(Some("sample".into()), &w).unwrap();
    assert_eq!(
        seeded
            .apply_changes(&edited.export_changes().unwrap())
            .unwrap(),
        1
    );
    assert_eq!(
        seeded.get_workout("sample").unwrap().unwrap().workout.name,
        "Edited"
    );
    assert_eq!(
        edited
            .apply_changes(&seeded.export_changes().unwrap())
            .unwrap(),
        0
    );
}

#[test]
fn list_orders_and_unknown_id_inserts() {
    let s = Store::open(":memory:").unwrap();
    let a = s.save_workout(Some("sync-id".into()), &workout()).unwrap();
    assert_eq!(a.id, "sync-id");
    let b = s.save_workout(None, &workout()).unwrap();
    // b and the re-saved a can share a millisecond; make b clearly older.
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE workouts SET updated_at = 0 WHERE id = ?1", [&b.id])
        .unwrap();
    s.save_workout(Some(a.id.clone()), &workout()).unwrap();
    let ids: Vec<_> = s
        .list_workouts()
        .unwrap()
        .into_iter()
        .map(|w| w.id)
        .collect();
    assert_eq!(ids, [a.id, b.id]);
    // Older created_at must sort last, whatever the insert order.
    let x = s.save_session(Some("x".into()), &session()).unwrap();
    let y = s.save_session(Some("y".into()), &session()).unwrap();
    s.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE sessions SET created_at = ?1 WHERE id = 'x'",
            [y.created_at_ms - 5],
        )
        .unwrap();
    let ids: Vec<_> = s
        .list_sessions()
        .unwrap()
        .into_iter()
        .map(|w| w.id)
        .collect();
    assert_eq!(ids, [y.id, x.id]);
}

#[test]
fn session_round_trip_replace_and_cascade() {
    let s = Store::open(":memory:").unwrap();
    let a = s.save_session(None, &session()).unwrap();
    assert_eq!(s.get_session(&a.id).unwrap().unwrap().session, session());
    let mut shorter = session();
    shorter.laps.truncate(1);
    shorter.lap_positions.truncate(1);
    shorter.lap_lengths.truncate(1);
    let b = s.save_session(Some(a.id.clone()), &shorter).unwrap();
    assert_eq!(b.created_at_ms, a.created_at_ms);
    assert!(b.updated_at_ms > a.updated_at_ms);
    assert_eq!(count(&s, "laps"), 1);
    assert!(s.delete_session(&a.id).unwrap());
    assert_eq!(count(&s, "laps"), 0);
    assert_eq!(count(&s, "sessions"), 1);
    assert!(s.get_session(&a.id).unwrap().is_none() && s.list_sessions().unwrap().is_empty());
    s.conn
        .lock()
        .unwrap()
        .execute("DELETE FROM sessions", [])
        .unwrap();
    let c = s.save_session(None, &session()).unwrap();
    s.conn
        .lock()
        .unwrap()
        .execute("DELETE FROM sessions WHERE id = ?1", [&c.id])
        .unwrap();
    assert_eq!(count(&s, "laps"), 0); // hard delete still cascades
}

#[test]
fn hr_and_age_round_trip_and_sync() {
    let a = Store::open(":memory:").unwrap();
    let mut s = session();
    s.hr = vec![
        HrSample {
            t_s: 0.0,
            bpm: 120.0,
        },
        HrSample {
            t_s: 5.0,
            bpm: 131.5,
        },
        HrSample {
            t_s: 5.0,
            bpm: 140.0,
        },
    ];
    s.age_years = Some(40);
    let saved = a.save_session(None, &s).unwrap();
    assert_eq!(a.get_session(&saved.id).unwrap().unwrap().session, s);
    let b = Store::open(":memory:").unwrap();
    assert_eq!(b.apply_changes(&a.export_changes().unwrap()).unwrap(), 1);
    assert_eq!(b.get_session(&saved.id).unwrap().unwrap().session, s);
    // Saving without HR or age clears both.
    s.hr.clear();
    s.age_years = None;
    a.save_session(Some(saved.id.clone()), &s).unwrap();
    assert_eq!(a.get_session(&saved.id).unwrap().unwrap().session, s);
    assert_eq!(count(&a, "sessions"), 1);
}

#[test]
fn location_round_trip_and_sync() {
    let a = Store::open(":memory:").unwrap();
    let mut s = session();
    s.location = Some(Location {
        lat: 52.37,
        lon: 4.89,
        accuracy_m: 12.5,
    });
    let saved = a.save_session(None, &s).unwrap();
    assert_eq!(a.get_session(&saved.id).unwrap().unwrap().session, s);
    let b = Store::open(":memory:").unwrap();
    assert_eq!(b.apply_changes(&a.export_changes().unwrap()).unwrap(), 1);
    assert_eq!(b.get_session(&saved.id).unwrap().unwrap().session, s);
    s.location = None;
    a.save_session(Some(saved.id.clone()), &s).unwrap();
    assert_eq!(
        a.get_session(&saved.id).unwrap().unwrap().session.location,
        None
    );
}

#[test]
fn rejects_invalid_location() {
    let s = Store::open(":memory:").unwrap();
    let ok = Location {
        lat: 0.0,
        lon: 0.0,
        accuracy_m: 0.0,
    };
    let bad_loc = [
        Location { lat: 90.5, ..ok },
        Location {
            lat: f64::NAN,
            ..ok
        },
        Location { lon: -180.5, ..ok },
        Location {
            accuracy_m: -1.0,
            ..ok
        },
        Location {
            accuracy_m: f64::INFINITY,
            ..ok
        },
    ];
    for location in bad_loc {
        let mut bad = session();
        bad.location = Some(location);
        assert!(matches!(
            s.save_session(None, &bad),
            Err(StoreError::Invalid { .. })
        ));
    }
    assert_eq!(count(&s, "sessions"), 0);
}

#[test]
fn rejects_invalid_hr() {
    let s = Store::open(":memory:").unwrap();
    let bad_hr = [
        vec![HrSample {
            t_s: 0.0,
            bpm: f64::NAN,
        }],
        vec![HrSample { t_s: 0.0, bpm: 0.0 }],
        vec![HrSample {
            t_s: f64::INFINITY,
            bpm: 120.0,
        }],
        vec![
            HrSample {
                t_s: 5.0,
                bpm: 120.0,
            },
            HrSample {
                t_s: 4.0,
                bpm: 121.0,
            },
        ],
    ];
    for hr in bad_hr {
        let mut bad = session();
        bad.hr = hr;
        assert!(matches!(
            s.save_session(None, &bad),
            Err(StoreError::Invalid { .. })
        ));
    }
    assert_eq!(count(&s, "sessions"), 0);
}

#[test]
fn migrates_v1_database() {
    let path = std::env::temp_dir().join(format!("swimlie-v1-{}.db", uuid::Uuid::new_v4()));
    let p = path.to_str().unwrap();
    {
        let conn = Connection::open(p).unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute(
            "INSERT INTO sessions (id, workout_json, started_at_ms, workout_id, created_at, updated_at) VALUES ('old', ?1, 0, NULL, 1, 1)",
            [workout_to_json(workout())],
        )
        .unwrap();
    }
    let s = Store::open(p).unwrap();
    let got = s.get_session("old").unwrap().unwrap().session;
    assert!(got.hr.is_empty() && got.age_years.is_none());
    let version: i64 = s
        .conn
        .lock()
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, MIGRATIONS.len() as i64);
    drop(s);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn rejects_invalid_saves() {
    let s = Store::open(":memory:").unwrap();
    let mut bad = session();
    bad.lap_lengths.pop();
    assert!(matches!(
        s.save_session(None, &bad),
        Err(StoreError::Invalid { .. })
    ));
    let mut bad = session();
    bad.lap_lengths[0] = 0;
    assert!(matches!(
        s.save_session(None, &bad),
        Err(StoreError::Invalid { .. })
    ));
    let mut bad = workout();
    bad.blocks.clear();
    assert!(matches!(
        s.save_workout(None, &bad),
        Err(StoreError::Invalid { .. })
    ));
    assert_eq!(count(&s, "sessions") + count(&s, "workouts"), 0);
}

#[test]
fn file_db_persists_and_newer_version_rejected() {
    let path = std::env::temp_dir().join(format!("swimlie-store-{}.db", uuid::Uuid::new_v4()));
    let p = path.to_str().unwrap();
    let id = Store::open(p)
        .unwrap()
        .save_workout(None, &workout())
        .unwrap()
        .id;
    let s = Store::open(p).unwrap();
    assert_eq!(s.get_workout(&id).unwrap().unwrap().workout, workout());
    s.conn
        .lock()
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    drop(s);
    assert!(matches!(
        Store::open(p),
        Err(StoreError::TooNew { found: 99, .. })
    ));
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn corrupt_rows_yield_corrupt() {
    let s = Store::open(":memory:").unwrap();
    let a = s.save_session(None, &session()).unwrap();
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE laps SET stroke = 'doggy' WHERE idx = 0", [])
        .unwrap();
    assert!(matches!(
        s.get_session(&a.id),
        Err(StoreError::Corrupt { .. })
    ));
    s.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE laps SET stroke = 'freestyle', block = -1 WHERE idx = 0",
            [],
        )
        .unwrap();
    assert!(matches!(
        s.get_session(&a.id),
        Err(StoreError::Corrupt { .. })
    ));
    let w = s.save_workout(None, &workout()).unwrap();
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE workouts SET json = '{}'", [])
        .unwrap();
    assert!(matches!(
        s.get_workout(&w.id),
        Err(StoreError::Corrupt { .. })
    ));
    assert!(matches!(s.list_workouts(), Err(StoreError::Corrupt { .. })));
}
