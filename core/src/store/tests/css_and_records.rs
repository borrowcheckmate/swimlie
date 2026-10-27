use super::*;

#[test]
fn css_round_trip_latest_and_invalid() {
    let s = Store::open(":memory:").unwrap();
    assert_eq!(s.latest_css_s_per_100m().unwrap(), None);
    let a = s.save_css(None, 360.0, 170.0).unwrap();
    assert_eq!((a.t400_s, a.t200_s), (360.0, 170.0));
    assert_eq!(s.latest_css_s_per_100m().unwrap(), Some(95.0));
    let b = s.save_css(None, 340.0, 160.0).unwrap();
    s.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE css_results SET created_at = ?1 WHERE id = ?2",
            params![a.created_at_ms - 5, b.id],
        )
        .unwrap();
    assert_eq!(s.latest_css_s_per_100m().unwrap(), Some(95.0)); // a is now the newest
    s.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE css_results SET created_at = ?1 WHERE id = ?2",
            params![a.created_at_ms + 5, b.id],
        )
        .unwrap();
    assert_eq!(s.latest_css_s_per_100m().unwrap(), Some(90.0));
    let c = s.save_css(Some(b.id.clone()), 350.0, 150.0).unwrap();
    assert_eq!(c.created_at_ms, a.created_at_ms + 5);
    assert!(c.updated_at_ms > b.updated_at_ms && c.id == b.id);
    assert!(s.delete_css(&c.id).unwrap() && !s.delete_css(&c.id).unwrap());
    assert_eq!(s.latest_css_s_per_100m().unwrap(), Some(95.0));
    for (t400, t200) in [(170.0, 360.0), (360.0, 0.0), (f64::NAN, 170.0)] {
        assert!(matches!(
            s.save_css(None, t400, t200),
            Err(StoreError::Invalid { .. })
        ));
    }
    s.conn
        .lock()
        .unwrap()
        .execute("UPDATE css_results SET t200_s = 999", [])
        .unwrap();
    assert!(matches!(
        s.latest_css_s_per_100m(),
        Err(StoreError::Corrupt { .. })
    ));
}

#[test]
fn import_css_tests_saves_once_and_keeps_deletes() {
    let s = Store::open(":memory:").unwrap();
    let id = s
        .save_session(None, &crate::metrics::css_session(360.0, 170.0))
        .unwrap()
        .id;
    assert_eq!(s.import_css_tests().unwrap(), 1);
    assert_eq!(s.latest_css_s_per_100m().unwrap(), Some(95.0));
    assert_eq!(s.import_css_tests().unwrap(), 0);
    assert!(s.delete_css(&id).unwrap());
    assert_eq!(s.import_css_tests().unwrap(), 0);
    assert_eq!(s.latest_css_s_per_100m().unwrap(), None);
}

#[test]
fn delete_session_drops_its_css() {
    let s = Store::open(":memory:").unwrap();
    let id = s
        .save_session(None, &crate::metrics::css_session(360.0, 170.0))
        .unwrap()
        .id;
    assert_eq!(s.import_css_tests().unwrap(), 1);
    assert!(s.delete_session(&id).unwrap());
    assert_eq!(s.latest_css_s_per_100m().unwrap(), None);
    assert_eq!(s.import_css_tests().unwrap(), 0);
}

#[test]
fn tss_of_stored_session_uses_latest_css() {
    let s = Store::open(":memory:").unwrap();
    // 4 x 25 m = 100 m; laps 0-30, 50-80, 100-130, 150-180 so 180 s elapsed.
    let mut sess = session();
    sess.laps = (0..4)
        .map(|i| Lap {
            start_s: f64::from(i) * 50.0,
            end_s: f64::from(i).mul_add(50.0, 30.0),
            stroke: Stroke::Freestyle,
            strokes: None,
        })
        .collect();
    sess.lap_positions = vec![
        Position {
            block: 0,
            round: 0,
            set: 0,
            rep: 0
        };
        4
    ];
    sess.lap_lengths = vec![1; 4];
    let stored = s.save_session(None, &sess).unwrap().session;
    assert_eq!(
        crate::metrics::swim_tss(&stored, s.latest_css_s_per_100m().unwrap()),
        None
    );
    s.save_css(None, 360.0, 170.0).unwrap();
    // 100 m in 180 s: 0.5556 m/s vs CSS 1/0.95 m/s, IF = 0.5556 x 0.95 = 19/36; TSS = 0.05 h x (19/36)^2 x 100.
    let tss = crate::metrics::swim_tss(&stored, s.latest_css_s_per_100m().unwrap()).unwrap();
    assert!((tss - 5.0 * 361.0 / 1296.0).abs() < 1e-9);
}

#[test]
fn personal_records_from_stored_sessions() {
    let store = Store::open(":memory:").unwrap();
    let pos = |rep| Position {
        block: 0,
        round: 0,
        set: 0,
        rep,
    };
    let lap = |start_s, end_s| Lap {
        start_s,
        end_s,
        stroke: Stroke::Freestyle,
        strokes: None,
    };
    let mut s = session();
    s.laps = vec![lap(0.0, 20.0), lap(22.0, 42.0)];
    s.lap_positions = vec![pos(0), pos(0)];
    s.lap_lengths = vec![1, 1];
    store.save_session(None, &s).unwrap();
    let prs = store.personal_records().unwrap();
    assert_eq!(
        prs.iter()
            .map(|p| (p.distance_m, p.time_s))
            .collect::<Vec<_>>(),
        vec![(50, 42.0)]
    );
}
