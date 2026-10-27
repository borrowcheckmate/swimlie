use super::{Intensity, Session, Stroke, reps};

/// Heart-rate zone 1..=5 from max HR = 220 - age: below 60 % of max is 1 (so is anything lower),
/// then 60-70 % is 2, 70-80 % is 3, 80-90 % is 4, 90 % and up is 5. `None` for a bad bpm or age.
#[uniffi::export]
#[must_use]
pub fn hr_zone(bpm: f64, age_years: u32) -> Option<u8> {
    let max = 220.0 - f64::from(age_years);
    if !bpm.is_finite() || bpm <= 0.0 || max <= 0.0 {
        return None;
    }
    let f = bpm / max;
    Some(if f < 0.6 {
        1
    } else if f < 0.7 {
        2
    } else if f < 0.8 {
        3
    } else if f < 0.9 {
        4
    } else {
        5
    })
}

/// HR zones that fit an intensity, lowest first.
#[uniffi::export]
#[must_use]
pub fn target_zones(intensity: Intensity) -> Vec<u8> {
    match intensity {
        Intensity::Easy => vec![1, 2],
        Intensity::Moderate => vec![2, 3],
        Intensity::Threshold => vec![3, 4],
        // HR lags on short sprints, so zone 4 still counts.
        Intensity::Hard | Intensity::Sprint => vec![4, 5],
    }
}

/// Critical swim speed pace from 400 m and 200 m times: `(t400 - t200) / 2`; `None` unless `0 < t200 < t400`.
#[must_use]
pub fn css_s_per_100m(t400_s: f64, t200_s: f64) -> Option<f64> {
    let ok = t400_s.is_finite() && t200_s.is_finite() && t200_s > 0.0 && t200_s < t400_s;
    ok.then(|| (t400_s - t200_s) / 2.0)
}

/// Times of a CSS test swim.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CssTest {
    pub t400_s: f64,
    pub t200_s: f64,
}

/// CSS test times: the first complete rep of a 400 m and a 200 m freestyle sprint trial (1 rep, block repeat 1).
///
/// Every matching set counts in workout order, so an aborted try is skipped. `None` unless `t400 > 2 x t200` and the CSS is 50-300 s/100 m.
#[uniffi::export]
#[must_use]
pub fn css_test_times(session: &Session) -> Option<CssTest> {
    let w = &session.workout;
    let r = reps(session);
    let trial = |distance_m: u32| {
        w.blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.repeat == 1)
            .flat_map(|(bi, b)| {
                b.sets
                    .iter()
                    .enumerate()
                    .filter(move |(_, s)| {
                        s.reps == 1
                            && s.stroke == Stroke::Freestyle
                            && s.intensity == Intensity::Sprint
                            && s.distance_m == distance_m
                    })
                    .map(move |(si, _)| (bi as u32, si as u32))
            })
            .find_map(|(block, set)| {
                r.iter()
                    .find(|r| {
                        r.pos.block == block
                            && r.pos.set == set
                            && r.lengths == distance_m / w.pool_length_m
                    })
                    .map(|r| r.swim_s)
            })
    };
    let (t400_s, t200_s) = (trial(400)?, trial(200)?);
    let css = css_s_per_100m(t400_s, t200_s)?;
    (t400_s > 2.0 * t200_s && (50.0..=300.0).contains(&css)).then_some(CssTest { t400_s, t200_s })
}

/// Swim TSS: elapsed hours x IF^2 x 100, IF = session speed / CSS speed. Rests count (`TrainingPeaks` convention).
/// `None` without a usable CSS or laps.
#[must_use]
pub fn swim_tss(session: &Session, css_s_per_100m: Option<f64>) -> Option<f64> {
    let css = css_s_per_100m.filter(|c| c.is_finite() && *c > 0.0)?;
    let elapsed = session.laps.last()?.end_s - session.laps.first()?.start_s;
    if elapsed.is_nan() || elapsed <= 0.0 {
        return None;
    }
    let distance = session.lap_lengths.iter().sum::<u32>() * session.workout.pool_length_m;
    let intensity = f64::from(distance) / elapsed / (100.0 / css);
    Some(elapsed / 3600.0 * intensity * intensity * 100.0)
}

/// Session of the bundled CSS test: every length swum, the 400 in `t400_s` and the 200 in `t200_s`, with even laps and no turns.
#[cfg(test)]
#[must_use]
pub fn css_session(t400_s: f64, t200_s: f64) -> Session {
    use crate::model::{Lap, Position};
    let workout = crate::model::parse_workout(
        include_str!("../../../apple/Watch/Workouts/css-test.json").into(),
    )
    .unwrap();
    let mut s = Session {
        workout,
        started_at_ms: 1_760_000_000_000,
        workout_id: None,
        laps: vec![],
        lap_positions: vec![],
        lap_lengths: vec![],
        hr: vec![],
        age_years: None,
        location: None,
    };
    let mut t = 0.0;
    for (block, n, dur) in [(1u32, 16u32, t400_s), (2, 8, t200_s)] {
        for i in 0..n {
            let (a, b) = (
                t + dur * f64::from(i) / f64::from(n),
                t + dur * f64::from(i + 1) / f64::from(n),
            );
            s.laps.push(Lap {
                start_s: a,
                end_s: b,
                stroke: Stroke::Freestyle,
                strokes: None,
            });
            s.lap_positions.push(Position {
                block,
                round: 0,
                set: 0,
                rep: 0,
            });
            s.lap_lengths.push(1);
        }
        t += dur + 300.0;
    }
    s
}
