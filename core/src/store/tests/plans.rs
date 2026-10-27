use super::*;

pub(super) fn plan() -> Plan {
    parse_plan(include_str!("../../../../schema/plan-example.json").into()).unwrap()
}

#[test]
fn plan_save_list_delete() {
    let s = Store::open(":memory:").unwrap();
    let a = s.save_plan(None, &plan()).unwrap();
    assert_eq!(s.list_plans().unwrap(), vec![a.clone()]);
    assert!(s.delete_plan(&a.id).unwrap());
    assert!(!s.delete_plan(&a.id).unwrap());
    assert_eq!(s.list_plans().unwrap(), Vec::<StoredPlan>::new());
    let mut bad = plan();
    bad.weeks.clear();
    assert!(matches!(
        s.save_plan(None, &bad),
        Err(StoreError::Invalid { .. })
    ));
    assert_eq!(count(&s, "plans"), 1); // the tombstone only
}

#[test]
fn plan_sync_and_tombstone() {
    let a = Store::open(":memory:").unwrap();
    let p = a.save_plan(None, &plan()).unwrap();
    let b = Store::open(":memory:").unwrap();
    assert_eq!(b.apply_changes(&a.export_changes().unwrap()).unwrap(), 1);
    assert_eq!(b.list_plans().unwrap(), vec![p.clone()]);
    a.delete_plan(&p.id).unwrap();
    assert_eq!(b.apply_changes(&a.export_changes().unwrap()).unwrap(), 1);
    assert_eq!(b.list_plans().unwrap(), Vec::<StoredPlan>::new());
}

#[test]
fn sync_batch_without_plans_key_applies() {
    let a = Store::open(":memory:").unwrap();
    a.save_workout(None, &workout()).unwrap();
    let mut old: serde_json::Value = serde_json::from_str(&a.export_changes().unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("plans");
    let b = Store::open(":memory:").unwrap();
    assert_eq!(b.apply_changes(&old.to_string()).unwrap(), 1);
}
