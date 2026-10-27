use super::*;

pub(super) fn workout_batch(id: &str, name: &str, updated_at_ms: i64) -> String {
    let mut data = workout();
    data.name = name.into();
    let row = SyncRow {
        id: id.into(),
        created_at_ms: 1,
        updated_at_ms,
        deleted_at_ms: None,
        data,
    };
    serde_json::to_string(&SyncBatch {
        workouts: vec![row],
        sessions: vec![],
        plans: vec![],
    })
    .unwrap()
}

#[test]
fn sync_round_trip_keeps_rows_and_tombstones() {
    let a = Store::open(":memory:").unwrap();
    let w1 = a.save_workout(None, &workout()).unwrap();
    let w2 = a.save_workout(None, &workout()).unwrap();
    a.delete_workout(&w2.id).unwrap();
    a.save_session(None, &session()).unwrap();
    let b = Store::open(":memory:").unwrap();
    assert_eq!(b.apply_changes(&a.export_changes().unwrap()).unwrap(), 3);
    assert_eq!(
        b.get_workout(&w1.id).unwrap(),
        a.get_workout(&w1.id).unwrap()
    );
    assert_eq!(b.list_workouts().unwrap(), a.list_workouts().unwrap());
    assert_eq!(b.list_sessions().unwrap(), a.list_sessions().unwrap());
    assert!(b.get_workout(&w2.id).unwrap().is_none());
    let batch: SyncBatch = serde_json::from_str(&b.export_changes().unwrap()).unwrap();
    assert!(
        batch
            .workouts
            .iter()
            .any(|r| r.id == w2.id && r.deleted_at_ms.is_some())
    );
}

#[test]
fn sync_reapplying_writes_nothing() {
    let a = Store::open(":memory:").unwrap();
    a.save_workout(None, &workout()).unwrap();
    a.save_session(None, &session()).unwrap();
    let json = a.export_changes().unwrap();
    let b = Store::open(":memory:").unwrap();
    assert_eq!(b.apply_changes(&json).unwrap(), 2);
    assert_eq!(b.apply_changes(&json).unwrap(), 0);
}

#[test]
fn sync_writes_only_newer_rows() {
    let b = Store::open(":memory:").unwrap();
    assert_eq!(
        b.apply_changes(&workout_batch("w", "first", 100)).unwrap(),
        1
    );
    assert_eq!(
        b.apply_changes(&workout_batch("w", "older", 99)).unwrap(),
        0
    );
    assert_eq!(b.get_workout("w").unwrap().unwrap().workout.name, "first");
    assert_eq!(
        b.apply_changes(&workout_batch("w", "newer", 101)).unwrap(),
        1
    );
    let got = b.get_workout("w").unwrap().unwrap();
    assert_eq!(
        (
            got.workout.name.as_str(),
            got.updated_at_ms,
            got.created_at_ms
        ),
        ("newer", 101, 1)
    );
}

#[test]
fn sync_equal_timestamps_converge() {
    let a = Store::open(":memory:").unwrap();
    let b = Store::open(":memory:").unwrap();
    a.apply_changes(&workout_batch("w", "x", 5)).unwrap();
    b.apply_changes(&workout_batch("w", "y", 5)).unwrap();
    let (ea, eb) = (a.export_changes().unwrap(), b.export_changes().unwrap());
    a.apply_changes(&eb).unwrap();
    b.apply_changes(&ea).unwrap();
    assert_eq!(a.get_workout("w").unwrap().unwrap().workout.name, "y");
    assert_eq!(a.get_workout("w").unwrap(), b.get_workout("w").unwrap());
    assert_eq!(a.export_changes().unwrap(), b.export_changes().unwrap());
}

pub(super) fn session_batch(
    id: &str,
    data: Session,
    updated_at_ms: i64,
    deleted_at_ms: Option<i64>,
) -> String {
    let row = SyncRow {
        id: id.into(),
        created_at_ms: 1,
        updated_at_ms,
        deleted_at_ms,
        data,
    };
    serde_json::to_string(&SyncBatch {
        workouts: vec![],
        sessions: vec![row],
        plans: vec![],
    })
    .unwrap()
}

#[test]
fn sync_equal_timestamps_keep_fuller_row() {
    let loc = Location {
        lat: 52.37,
        lon: 4.89,
        accuracy_m: 12.5,
    };
    let mut full = session();
    full.location = Some(loc);
    let (a, b) = (
        Store::open(":memory:").unwrap(),
        Store::open(":memory:").unwrap(),
    );
    a.apply_changes(&session_batch("s", full.clone(), 5, None))
        .unwrap();
    // Same stamp, no location: shorter JSON, so the local row stays.
    assert_eq!(
        a.apply_changes(&session_batch("s", session(), 5, None))
            .unwrap(),
        0
    );
    assert_eq!(a.get_session("s").unwrap().unwrap().session, full);
    // The other way round: the fuller row overwrites.
    b.apply_changes(&session_batch("s", session(), 5, None))
        .unwrap();
    assert_eq!(
        b.apply_changes(&session_batch("s", full.clone(), 5, None))
            .unwrap(),
        1
    );
    assert_eq!(b.get_session("s").unwrap().unwrap().session, full);
}

#[test]
fn sync_equal_timestamps_tombstone_wins() {
    let mut full = session();
    full.location = Some(Location {
        lat: 52.37,
        lon: 4.89,
        accuracy_m: 12.5,
    });
    let a = Store::open(":memory:").unwrap();
    // Live row is longer than the tombstone, yet the tombstone wins the tie.
    a.apply_changes(&session_batch("s", full.clone(), 5, None))
        .unwrap();
    assert_eq!(
        a.apply_changes(&session_batch("s", session(), 5, Some(5)))
            .unwrap(),
        1
    );
    assert!(a.get_session("s").unwrap().is_none());
    // And a live row with the same stamp does not revive it.
    assert_eq!(
        a.apply_changes(&session_batch("s", full, 5, None)).unwrap(),
        0
    );
    assert!(a.get_session("s").unwrap().is_none());
}

#[test]
fn sync_awkward_floats_export_identically() {
    // Values that default serde_json parsing reads back one ulp off.
    let mut s = session();
    s.laps[0].start_s = 0.1 + 0.2;
    s.laps[1].end_s = 187.821_265_928_172_58;
    s.hr = vec![
        HrSample {
            t_s: 0.1 + 0.2,
            bpm: 119.639_053_788_655_97,
        },
        HrSample {
            t_s: 187.821_265_928_172_58,
            bpm: 48.108_121_892_211_756,
        },
    ];
    let a = Store::open(":memory:").unwrap();
    a.save_session(None, &s).unwrap();
    let b = Store::open(":memory:").unwrap();
    assert_eq!(b.apply_changes(&a.export_changes().unwrap()).unwrap(), 1);
    let from_b = b.export_changes().unwrap();
    assert_eq!(a.apply_changes(&from_b).unwrap(), 0);
    assert_eq!(a.export_changes().unwrap(), from_b);
    assert_eq!(
        b.get_session(&a.list_sessions().unwrap()[0].id)
            .unwrap()
            .unwrap()
            .session,
        s
    );
}

#[test]
fn sync_skips_invalid_rows_and_applies_the_rest() {
    let mut bad_data = workout();
    bad_data.blocks.clear();
    let mut batch: SyncBatch = serde_json::from_str(&workout_batch("good", "ok", 5)).unwrap();
    batch.workouts.push(SyncRow {
        id: "bad".into(),
        created_at_ms: 1,
        updated_at_ms: 5,
        deleted_at_ms: None,
        data: bad_data,
    });
    let mut json = serde_json::to_value(&batch).unwrap();
    json["workouts"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id": "junk"}));
    let b = Store::open(":memory:").unwrap();
    assert_eq!(b.apply_counted(&json.to_string()).unwrap(), (1, 2));
    assert_eq!(b.get_workout("good").unwrap().unwrap().workout.name, "ok");
    assert!(b.get_workout("bad").unwrap().is_none());
    assert!(matches!(
        b.apply_changes("{"),
        Err(StoreError::Invalid { .. })
    ));
    assert_eq!(count(&b, "workouts") + count(&b, "sessions"), 1);
}
