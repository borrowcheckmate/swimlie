use super::*;

#[test]
fn rep_completes_at_target_distance() {
    let mut e = engine(r#"{"reps": 2, "distance_m": 50}"#);
    let end = swim(&mut e, 0.0, 1, 20.0);
    let s = e.tick(end + 5.0);
    assert_eq!(
        (s.pos.rep, s.done_m, s.target_m, s.phase),
        (0, 25, 50, Phase::Swimming)
    );
    let end = swim(&mut e, end + 2.0, 1, 20.0);
    // No planned rest: straight on to the next rep.
    let s = e.tick(end + 5.0);
    assert_eq!((s.pos.rep, s.done_m, s.phase), (1, 0, Phase::Swimming));
    assert_eq!(s.last_rep_pace_s_per_100m, Some(84.0));
    swim(&mut e, end + 30.0, 2, 20.0);
    assert_eq!(e.tick(200.0).phase, Phase::Done);
}

#[test]
fn fixed_rest_counts_from_rep_end_interval_from_rep_start() {
    let mut fixed =
        engine(r#"{"reps": 2, "distance_m": 50, "rest": {"type": "fixed", "seconds": 15}}"#);
    let mut interval =
        engine(r#"{"reps": 2, "distance_m": 50, "rest": {"type": "interval", "seconds": 70}}"#);
    let end = swim(&mut fixed, 0.0, 2, 20.0);
    swim(&mut interval, 0.0, 2, 20.0);
    let s = fixed.tick(50.0);
    assert_eq!(
        (s.pos.rep, s.phase, s.rest_remaining_s),
        (1, Phase::Resting, Some(end + 15.0 - 50.0))
    );
    assert_eq!(interval.tick(50.0).rest_remaining_s, Some(20.0));
    // The timer ending starts the next rep even before its first lap arrives.
    let s = fixed.tick(end + 16.0);
    assert_eq!(
        (s.pos.rep, s.phase, s.rest_remaining_s),
        (1, Phase::Swimming, None)
    );
}

#[test]
fn unplanned_stop_shows_resting() {
    let mut e = engine(r#"{"reps": 1, "distance_m": 100}"#);
    let end = swim(&mut e, 0.0, 2, 20.0);
    // The next lap is not overdue yet: still mid-length.
    assert_eq!(e.tick(end + 30.0).phase, Phase::Swimming);
    let s = e.tick(end + 60.0);
    assert_eq!(
        (s.pos.rep, s.done_m, s.phase, s.rest_remaining_s),
        (0, 50, Phase::Resting, None)
    );
}

#[test]
fn pause_and_strokes_do_not_look_like_a_stop() {
    let mut e = engine(r#"{"reps": 1, "distance_m": 100}"#);
    let end = swim(&mut e, 0.0, 2, 20.0);
    e.set_paused(true, end + 5.0);
    let s = e.tick(end + 30.0);
    assert_eq!((s.phase, s.rest_remaining_s), (Phase::Resting, None));
    e.set_paused(false, end + 65.0);
    // 60 s paused: well past the stop threshold in wall time, but only 5 s swum.
    assert_eq!(e.tick(end + 66.0).phase, Phase::Swimming);
    assert_eq!(e.tick(end + 130.0).phase, Phase::Resting);
    // Strokes after the last lap: still swimming although the lap has not been reported.
    e.on_activity(end + 125.0);
    assert_eq!(e.tick(end + 130.0).phase, Phase::Swimming);
}

#[test]
fn lap_swallowing_a_rest_is_one_length() {
    let mut e = engine(r#"{"reps": 2, "distance_m": 100}"#);
    let end = swim(&mut e, 0.0, 4, 25.0);
    // First length of rep 2 is stamped from the end of rep 1: it includes 40 s of rest.
    swim(&mut e, end, 1, 65.0);
    let end = swim(&mut e, end + 67.0, 3, 25.0);
    assert_eq!(e.tick(end + 5.0).phase, Phase::Done);
    assert_eq!(e.tick(end + 5.0).swum_m, 200);
}

#[test]
fn ghost_lap_during_planned_rest_does_not_start_the_next_rep() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 100, "rest": {"type": "fixed", "seconds": 20}}"#);
    let end = swim(&mut e, 0.0, 4, 25.0);
    e.on_lap(
        Lap {
            start_s: end + 3.0,
            end_s: end + 16.0,
            stroke: Stroke::Freestyle,
            strokes: None,
        },
        end + 18.0,
    );
    let s = e.tick(end + 18.0);
    assert_eq!(
        (s.pos.rep, s.done_m, s.phase, s.rest_remaining_s),
        (1, 0, Phase::Resting, Some(2.0))
    );
    let end = swim(&mut e, end + 22.0, 4, 25.0);
    assert_eq!(reps(&e), [0, 0, 0, 0, 1, 1, 1, 1]);
    assert_eq!(e.tick(end + 5.0).phase, Phase::Done);
}

#[test]
fn late_lap_counts_for_its_own_rep() {
    let mut e = engine(r#"{"reps": 2, "distance_m": 50}"#);
    swim(&mut e, 0.0, 1, 20.0);
    // Swimmer presses "done" at the wall (t = 42); the lap for 22–42 arrives 5 s later.
    assert_eq!(e.manual_next(42.0).pos.rep, 1);
    let s = e.on_lap(
        Lap {
            start_s: 22.0,
            end_s: 42.0,
            stroke: Stroke::Freestyle,
            strokes: Some(10),
        },
        47.0,
    );
    assert_eq!((s.pos.rep, s.done_m), (1, 0));
    assert_eq!(reps(&e), [0, 0]);
}

#[test]
fn missing_length_does_not_shift_later_reps() {
    let mut e = engine(r#"{"reps": 3, "distance_m": 100}"#);
    let end = swim(&mut e, 0.0, 4, 20.0);
    // Rep 2: the 3rd length is swum but never reported.
    let end = swim(&mut e, end + 30.0, 2, 20.0);
    let end = swim(&mut e, end + 2.0 + 20.0 + 2.0, 1, 20.0);
    let end = swim(&mut e, end + 30.0, 4, 20.0);
    assert_eq!(e.tick(end + 5.0).phase, Phase::Done);
    assert_eq!(reps(&e), [0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 2]);
}

#[test]
fn merged_lap_counts_as_two_lengths() {
    let mut e = engine(r#"{"reps": 2, "distance_m": 100}"#);
    let end = swim(&mut e, 0.0, 2, 20.0);
    e.on_lap(
        Lap {
            start_s: end + 2.0,
            end_s: end + 44.0,
            stroke: Stroke::Kickboard,
            strokes: None,
        },
        end + 49.0,
    );
    let s = e.tick(end + 50.0);
    assert_eq!((s.pos.rep, s.done_m), (1, 0));
    swim(&mut e, end + 80.0, 1, 20.0);
    assert_eq!(reps(&e), [0, 0, 0, 1]);
}

#[test]
fn manual_next_and_back_mid_rep_with_laps() {
    let mut e =
        engine(r#"{"reps": 3, "distance_m": 100, "rest": {"type": "fixed", "seconds": 20}}"#);
    let end = swim(&mut e, 12.0, 2, 20.0);
    let before = e.tick(end + 5.0);
    let s = e.manual_next(end + 5.0);
    assert_eq!((s.pos.rep, s.done_m, s.phase), (1, 0, Phase::Resting));
    assert!(s.rest_remaining_s.is_some());
    // Back undoes it: rep 1 again, 50 m done.
    assert_eq!(
        e.manual_back(end + 6.0).map(|s| (s.pos.rep, s.done_m)),
        Some((0, before.done_m))
    );
    // A lap arriving after the skip still lands in the rep that was skipped.
    e.manual_next(end + 8.0);
    let s = e.on_lap(
        Lap {
            start_s: end + 2.0,
            end_s: end + 22.0,
            stroke: Stroke::Freestyle,
            strokes: None,
        },
        end + 27.0,
    );
    assert_eq!(s.pos.rep, 1);
}

#[test]
fn manual_override_closes_or_skips_the_rep_on_screen() {
    let mut e = engine(r#"{"reps": 3, "distance_m": 50}"#);
    // A drill HealthKit misses entirely: mark it done, then skip rep 2 during the rest.
    assert_eq!(e.manual_next(60.0).pos.rep, 1);
    assert_eq!(e.manual_next(80.0).pos.rep, 2);
    swim(&mut e, 90.0, 2, 20.0);
    assert_eq!(reps(&e), [2, 2]);
    assert_eq!(e.tick(140.0).phase, Phase::Done);
    // Done is final.
    assert_eq!(e.manual_next(150.0).phase, Phase::Done);
}

#[test]
fn start_countdown_ends_on_timer_lap_or_next() {
    let e = engine(r#"{"reps": 1, "distance_m": 50}"#);
    let s = e.tick(4.0);
    assert_eq!(
        (s.pos.rep, s.phase, s.rest_remaining_s, s.rep_elapsed_s),
        (0, Phase::Resting, Some(6.0), None)
    );
    let s = e.tick(13.0);
    assert_eq!((s.phase, s.rep_elapsed_s), (Phase::Swimming, Some(3.0)));
    // A lap before the timer ends starts the rep at the lap's start.
    let mut e = engine(r#"{"reps": 1, "distance_m": 50}"#);
    let s = e.on_lap(
        Lap {
            start_s: 6.0,
            end_s: 26.0,
            stroke: Stroke::Freestyle,
            strokes: None,
        },
        31.0,
    );
    assert_eq!(
        (s.phase, s.rep_elapsed_s, s.swum_m),
        (Phase::Swimming, Some(25.0), 25)
    );
    // Manual next skips the countdown instead of closing rep 1.
    let mut e = engine(r#"{"reps": 2, "distance_m": 50}"#);
    let s = e.manual_next(3.0);
    assert_eq!(
        (s.pos.rep, s.phase, s.rep_elapsed_s),
        (0, Phase::Swimming, Some(0.0))
    );
    assert_eq!(e.manual_next(20.0).pos.rep, 1);
}

#[test]
fn rep_elapsed_after_rest_and_swum_m() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 50, "rest": {"type": "fixed", "seconds": 15}}"#);
    let end = swim(&mut e, 12.0, 2, 20.0);
    let s = e.tick(end + 5.0);
    assert_eq!(
        (s.phase, s.rep_elapsed_s, s.swum_m),
        (Phase::Resting, None, 50)
    );
    // Rest over at end + 15: the next rep started then, even without a lap.
    assert_eq!(e.tick(end + 19.0).rep_elapsed_s, Some(4.0));
}

#[test]
fn manual_back_undoes_manual_next() {
    let mut e = engine(r#"{"reps": 3, "distance_m": 50}"#);
    assert_eq!(e.manual_back(0.0), None);
    e.manual_next(60.0);
    e.manual_next(80.0);
    // Within the grace period of rep 3: back undoes the second mark, rep 2 swims again.
    let s = e.manual_back(81.0).unwrap();
    assert_eq!(
        (s.pos.rep, s.phase, s.rep_elapsed_s),
        (1, Phase::Swimming, Some(21.0))
    );
    // Past the grace period: back restarts the clock.
    let s = e.manual_back(90.0).unwrap();
    assert_eq!(
        (s.pos.rep, s.phase, s.rep_elapsed_s),
        (1, Phase::Swimming, Some(0.0))
    );
    // During the start countdown "next" is go; back brings the countdown back.
    let mut e = engine(r#"{"reps": 1, "distance_m": 50}"#);
    assert_eq!(e.manual_next(3.0).phase, Phase::Swimming);
    // Back within the grace period: the countdown is back, and it counts in full again.
    let s = e.manual_back(4.0).unwrap();
    assert_eq!(
        (s.phase, s.rest_remaining_s),
        (Phase::Resting, Some(START_COUNTDOWN_S))
    );
    assert_eq!(e.tick(9.0).rest_remaining_s, Some(START_COUNTDOWN_S - 5.0));
}

#[test]
fn manual_next_during_planned_rest_ends_the_rest_only() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 100, "rest": {"type": "fixed", "seconds": 20}}"#);
    let end = swim(&mut e, 0.0, 4, 25.0);
    assert_eq!(e.tick(end + 5.0).rest_remaining_s, Some(15.0));
    // (a) Next ends the rest 1 s later: rep 2 stays and swims, nothing is closed.
    let s = e.manual_next(end + 5.0);
    assert_eq!(
        (s.pos.rep, s.done_m, s.phase, s.rest_remaining_s),
        (1, 0, Phase::Resting, Some(1.0))
    );
    assert_eq!(e.tick(end + 5.9).phase, Phase::Resting);
    assert_eq!(e.tick(end + 6.0).phase, Phase::Swimming);
    let end = swim(&mut e, end + 6.0, 4, 25.0);
    assert_eq!(reps(&e), [0, 0, 0, 0, 1, 1, 1, 1]);
    assert_eq!(e.tick(end + 5.0).phase, Phase::Done);
}

#[test]
fn manual_back_after_rest_skip_restarts_the_rest() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 100, "rest": {"type": "fixed", "seconds": 20}}"#);
    let end = swim(&mut e, 0.0, 4, 25.0);
    e.manual_next(end + 5.0);
    // (b) Back undoes the skip: rest of rep 1 counts in full from the back time.
    let s = e.manual_back(end + 6.0).unwrap();
    assert_eq!(
        (s.pos.rep, s.phase, s.rest_remaining_s),
        (1, Phase::Resting, Some(20.0))
    );
    assert_eq!(e.tick(end + 16.0).rest_remaining_s, Some(10.0));
    // Nothing left to undo: back does nothing.
    assert_eq!(e.manual_back(end + 16.0), None);
    assert_eq!(e.tick(end + 16.0).rest_remaining_s, Some(10.0));
}

#[test]
fn manual_next_with_less_than_the_prep_left_keeps_the_rest() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 100, "rest": {"type": "fixed", "seconds": 20}}"#);
    let end = swim(&mut e, 0.0, 4, 25.0);
    let s = e.manual_next(end + 19.5);
    assert_eq!((s.phase, s.rest_remaining_s), (Phase::Resting, Some(0.5)));
    assert_eq!(e.tick(end + 20.0).phase, Phase::Swimming);
}

#[test]
fn manual_back_while_swimming_restarts_the_rep_clock() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 100, "rest": {"type": "fixed", "seconds": 20}}"#);
    let end = swim(&mut e, 0.0, 4, 25.0);
    e.manual_next(end + 5.0);
    // Rep 2 starts by itself at the skip's end, then one length is swum.
    let lap_end = swim(&mut e, end + 8.0, 1, 25.0);
    let s = e.manual_back(lap_end + 2.0).unwrap();
    assert_eq!(
        (s.pos.rep, s.phase, s.rep_elapsed_s, s.swum_m),
        (1, Phase::Swimming, Some(0.0), 125)
    );
    assert_eq!(e.tick(lap_end + 4.0).rep_elapsed_s, Some(2.0));
    // Back right after a restart undoes the action before it (the skip): rep 2 is swimming again.
    let s = e.manual_back(lap_end + 4.0).unwrap();
    assert_eq!((s.pos.rep, s.phase), (1, Phase::Swimming));
}

#[test]
fn go_after_back_to_the_countdown_starts_the_clock_at_zero() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 50, "rest": {"type": "fixed", "seconds": 20}}"#);
    e.manual_next(3.0); // go
    e.manual_back(10.0); // past the grace period: clock restarts
    let s = e.manual_back(11.0).unwrap(); // within it: back to the countdown, in full
    assert_eq!(
        (s.phase, s.rest_remaining_s),
        (Phase::Resting, Some(START_COUNTDOWN_S))
    );
    let s = e.manual_next(13.0); // go again
    assert_eq!((s.phase, s.rep_elapsed_s), (Phase::Swimming, Some(0.0)));
    assert_eq!(e.tick(15.0).rep_elapsed_s, Some(2.0));
}

#[test]
fn repeated_back_on_the_first_rep_keeps_the_clock_at_zero() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 50, "rest": {"type": "fixed", "seconds": 20}}"#);
    assert_eq!(e.manual_back(25.0).unwrap().rep_elapsed_s, Some(0.0));
    // Within the grace period again: no earlier break, so the clock stays at 0:00 instead of flipping back to the old one.
    assert_eq!(e.manual_back(26.0).unwrap().rep_elapsed_s, Some(0.0));
    assert_eq!(e.manual_back(27.0).unwrap().rep_elapsed_s, Some(0.0));
    assert_eq!(e.tick(30.0).rep_elapsed_s, Some(3.0));
}

#[test]
fn manual_back_resets_the_first_rep_clock() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 50, "rest": {"type": "fixed", "seconds": 20}}"#);
    // No lap and no manual action yet: the first rep started by itself when the countdown ended.
    assert_eq!(e.tick(25.0).rep_elapsed_s, Some(15.0));
    let s = e.manual_back(25.0).unwrap();
    assert_eq!(s.rep_elapsed_s, Some(0.0));
    assert_eq!(e.tick(27.0).rep_elapsed_s, Some(2.0));
}

#[test]
fn manual_back_after_closing_a_rep_swims_it_again() {
    let mut e =
        engine(r#"{"reps": 2, "distance_m": 50, "rest": {"type": "fixed", "seconds": 20}}"#);
    assert_eq!(e.manual_next(3.0).pos.rep, 0);
    // (c) Swimming -> rest -> back: rep 1 is swimming again.
    let s = e.manual_next(20.0);
    assert_eq!(
        (s.pos.rep, s.phase, s.rest_remaining_s),
        (1, Phase::Resting, Some(20.0))
    );
    let s = e.manual_back(25.0).unwrap();
    assert_eq!((s.pos.rep, s.phase), (0, Phase::Swimming));
}
