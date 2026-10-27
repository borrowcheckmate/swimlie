//! Compact plain-text brief of one swim for the AI review. Built from the metrics helpers; no raw per-length data.

use crate::metrics::{
    RepMetrics, SessionSummary, SetMetrics, StrokeSplit, SwimmerProfile, mmss, personal_records,
    reps, schema_name, session_summary, sets, total_distance_m,
};
use crate::model::{RestKind, Session, Stroke, WorkoutSet};
use crate::store::StoredSession;
use std::fmt::Write as _;

/// How many earlier swims the brief compares against.
const COMPARE_LAST: usize = 5;
/// Differences under this (s/100 m, or points for SWOLF) count as the same, so rounding noise is never called a gain.
const SAME_S: f64 = 0.5;
const DAY_MS: i64 = 86_400_000;

/// `YYYY-MM-DD` (UTC) of a Unix time in ms.
fn date_utc(ms: i64) -> String {
    // Civil-from-days (Howard Hinnant's algorithm), days since 1970-01-01.
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The planned rest of a set. An interval is a send-off (start every N s), so its rest is the interval minus the
/// swim time of a rep; `swim_avg_s` is that swim time when known.
fn planned_rest(set: &WorkoutSet, swim_avg_s: Option<f64>) -> String {
    match set.rest.as_ref() {
        None => "no rest".into(),
        Some(r) if r.kind == RestKind::Interval => {
            let implied = swim_avg_s.map(|s| {
                format!(
                    ", implied rest about {:.0} s",
                    (f64::from(r.seconds) - s).max(0.0)
                )
            });
            format!(
                "send-off every {}{}",
                mmss(f64::from(r.seconds)),
                implied.unwrap_or_default()
            )
        }
        Some(r) => format!("rest {} s", r.seconds),
    }
}

/// One set line: planned vs done, so a cut-short set is never read as the planned set.
fn set_line(i: usize, st: &SetMetrics, planned: Option<&WorkoutSet>) -> String {
    let done_rep_m = st.distance_m / st.reps.max(1);
    let rest = if st.reps > 1 {
        format!("rest avg {:.0} s", st.rest_s / f64::from(st.reps - 1))
    } else {
        "single rep".into()
    };
    let swim_avg = (st.swim_s > 0.0).then(|| st.swim_s / f64::from(st.reps.max(1)));
    let done = format!("done {} x {} m", st.reps, done_rep_m);
    let pace = format!("pace {} /100 m, {rest}", mmss(st.pace_s_per_100m));
    let mut line = planned.map_or_else(
        || format!("- Set {}: {done}, {pace}", i + 1),
        |p| {
            let cut = if st.reps < p.reps {
                format!(" (cut short: {} of {} reps)", st.reps, p.reps)
            } else {
                String::new()
            };
            format!(
                "- Set {}: planned {} x {} m {} ({}, {}); {done}{cut}, {pace}",
                i + 1,
                p.reps,
                p.distance_m,
                schema_name(&p.stroke),
                schema_name(&p.intensity),
                planned_rest(p, swim_avg),
            )
        },
    );
    if st.goal_reps > 0 {
        let _ = write!(line, ", goal hit {}/{}", st.goal_hits, st.goal_reps);
    }
    line
}

/// Distance-weighted pace (s/100 m) of a run of reps; `None` without distance.
fn pace_of(reps: &[RepMetrics]) -> Option<f64> {
    let dist: u32 = reps.iter().map(|r| r.distance_m).sum();
    let swim: f64 = reps.iter().map(|r| r.swim_s).sum();
    (dist > 0).then(|| swim / f64::from(dist) * 100.0)
}

/// "same as that average" or "4 s/100 m faster" for a pace difference (this minus average).
fn pace_diff(d: f64) -> String {
    if d.abs() < SAME_S {
        "same as that average".into()
    } else {
        format!(
            "{:.0} s/100 m {}",
            d.abs(),
            if d < 0.0 { "faster" } else { "slower" }
        )
    }
}

/// Earlier swims with distance (0 m swims are ignored), newest first.
fn earlier_swims<'a>(
    session: &StoredSession,
    previous: &'a [StoredSession],
    css: Option<f64>,
) -> Vec<(&'a StoredSession, SessionSummary)> {
    let mut out: Vec<_> = previous
        .iter()
        .filter(|p| p.id != session.id && p.session.started_at_ms < session.session.started_at_ms)
        .map(|p| (p, session_summary(&p.session, css)))
        .filter(|(_, x)| x.distance_m > 0)
        .collect();
    out.sort_by_key(|(p, _)| std::cmp::Reverse(p.session.started_at_ms));
    out
}

/// Progress against the newest earlier swims. Only used when earlier swims exist.
fn progress_lines(
    sum: &SessionSummary,
    earlier: &[(&StoredSession, SessionSummary)],
    start_ms: i64,
) -> Vec<String> {
    let recent: Vec<&SessionSummary> = earlier.iter().take(COMPARE_LAST).map(|(_, x)| x).collect();
    let n = recent.len();
    let dist: f64 = recent.iter().map(|x| f64::from(x.distance_m)).sum();
    let avg_m = dist / n as f64;
    let avg_pace = recent.iter().map(|x| x.swim_s).sum::<f64>() / dist * 100.0;
    let mut lines = vec![format!(
        "Previous {n} swims: average {avg_m:.0} m, average pace {} /100 m",
        mmss(avg_pace)
    )];
    if let Some(p) = sum.pace_s_per_100m {
        lines.push(format!("Pace vs that average: {}", pace_diff(p - avg_pace)));
    }
    lines.push(format!(
        "Distance vs that average: {:+.0} m",
        f64::from(sum.distance_m) - avg_m
    ));
    // Same stroke only when this swim and the earlier ones are all one stroke each.
    if let [only] = sum.by_stroke.as_slice() {
        let same: Vec<&StrokeSplit> = earlier
            .iter()
            .filter_map(|(_, x)| match x.by_stroke.as_slice() {
                [one] if one.stroke == only.stroke && one.distance_m > 0 => Some(one),
                _ => None,
            })
            .take(COMPARE_LAST)
            .collect();
        if !same.is_empty() {
            let d: f64 = same.iter().map(|x| f64::from(x.distance_m)).sum();
            let avg = same.iter().map(|x| x.swim_s).sum::<f64>() / d * 100.0;
            lines.push(format!(
                "Same stroke ({}) vs last {} such swims: {}",
                schema_name(&only.stroke),
                same.len(),
                pace_diff(only.pace_s_per_100m - avg)
            ));
        }
    }
    let count = |days: i64| {
        1 + earlier
            .iter()
            .filter(|(p, _)| p.session.started_at_ms >= start_ms - days * DAY_MS)
            .count()
    };
    lines.push(format!(
        "Swims in the last 7 days: {}, last 28 days: {}",
        count(7),
        count(28)
    ));
    lines
}

/// Only facts from the numbers: goals, negative split, sets, plan, personal bests, SWOLF and stroke efficiency.
#[expect(
    clippy::too_many_lines,
    reason = "one linear pipeline whose steps share local state; splitting it is pending"
)]
fn achievement_lines(
    session: &StoredSession,
    sum: &SessionSummary,
    earlier: &[(&StoredSession, SessionSummary)],
    fade: Option<(f64, f64)>,
) -> Vec<String> {
    let s = &session.session;
    let mut lines = vec![];
    let set_list = sets(s);
    let (hits, goals) = set_list
        .iter()
        .fold((0, 0), |(h, g), st| (h + st.goal_hits, g + st.goal_reps));
    if hits > 0 {
        lines.push(format!("Hit the time goal on {hits} of {goals} reps"));
    }
    if let Some((first, last)) = fade
        && last < first - SAME_S
    {
        lines.push(format!(
            "Negative split: last third {} /100 m, faster than first third {} /100 m",
            mmss(last),
            mmss(first)
        ));
    }

    // Earlier time and distance per stroke, to judge this swim's sets against.
    let mut totals: Vec<(Stroke, f64, u32)> = vec![];
    for (_, x) in earlier {
        for b in &x.by_stroke {
            match totals.iter_mut().find(|t| t.0 == b.stroke) {
                Some(t) => {
                    t.1 += b.swim_s;
                    t.2 += b.distance_m;
                }
                None => totals.push((b.stroke, b.swim_s, b.distance_m)),
            }
        }
    }
    let (mut faster, mut total) = (0, 0);
    for st in &set_list {
        let Some(planned) = s
            .workout
            .blocks
            .get(st.block as usize)
            .and_then(|b| b.sets.get(st.set as usize))
        else {
            continue;
        };
        if planned.stroke == Stroke::Any {
            continue;
        }
        let Some(t) = totals.iter().find(|t| t.0 == planned.stroke && t.2 > 0) else {
            continue;
        };
        total += 1;
        if st.pace_s_per_100m < f64::mul_add(t.1 / f64::from(t.2), 100.0, -SAME_S) {
            faster += 1;
        }
    }
    if faster > 0 {
        lines.push(format!(
            "{faster} of {total} sets faster than your earlier average for that stroke"
        ));
    }

    let planned_m = total_distance_m(s.workout.clone());
    if planned_m > 0 && sum.distance_m > planned_m {
        lines.push(format!(
            "Swam {} m, {} m above the plan",
            sum.distance_m,
            sum.distance_m - planned_m
        ));
    }

    if !earlier.is_empty() {
        let longest = earlier.iter().map(|(_, x)| x.distance_m).max().unwrap_or(0);
        if sum.distance_m > longest {
            lines.push(format!(
                "Personal best: longest swim so far, {} m",
                sum.distance_m
            ));
        }
        let fastest = earlier
            .iter()
            .filter_map(|(_, x)| x.pace_s_per_100m)
            .fold(f64::INFINITY, f64::min);
        if let Some(p) = sum.pace_s_per_100m.filter(|p| *p < fastest - SAME_S) {
            lines.push(format!(
                "Personal best: fastest average pace so far, {} /100 m",
                mmss(p)
            ));
        }
        // A rep record counts only when an earlier swim had a time for the same distance and stroke.
        let mut all: Vec<StoredSession> = earlier.iter().map(|(p, _)| (*p).clone()).collect();
        let prior = personal_records(&all);
        all.push(session.clone());
        for r in personal_records(&all) {
            let beaten = prior
                .iter()
                .any(|p| p.distance_m == r.distance_m && p.stroke == r.stroke);
            if r.session_id == session.id
                && beaten
                && matches!(r.distance_m, 50 | 100)
                && r.equipment.is_empty()
            {
                lines.push(format!(
                    "Personal best: {} m {} rep in {}",
                    r.distance_m,
                    schema_name(&r.stroke),
                    mmss(r.time_s)
                ));
            }
        }
    }

    let recent: Vec<&SessionSummary> = earlier.iter().take(COMPARE_LAST).map(|(_, x)| x).collect();
    let swolf: Vec<f64> = recent.iter().filter_map(|x| x.swolf_avg).collect();
    if let (Some(now), false) = (sum.swolf_avg, swolf.is_empty()) {
        let avg = swolf.iter().sum::<f64>() / swolf.len() as f64;
        if now < avg - SAME_S {
            lines.push(format!("SWOLF {now:.1}, better than the {avg:.1} average of the last {} swims (lower is better)", swolf.len()));
        }
    }
    let dps: Vec<f64> = recent
        .iter()
        .filter_map(|x| x.distance_per_stroke_m)
        .collect();
    if let (Some(now), false) = (sum.distance_per_stroke_m, dps.is_empty()) {
        let avg = dps.iter().sum::<f64>() / dps.len() as f64;
        if now > avg + 0.02 {
            lines.push(format!("Distance per stroke {now:.2} m, more than the {avg:.2} m average of the last {} swims", dps.len()));
        }
    }
    lines
}

/// Plain-text brief of one swim for `ai_review`. `previous` may hold any earlier swims; the newest few before this one are compared.
#[uniffi::export]
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "one linear pipeline whose steps share local state; splitting it is pending"
)]
pub fn session_review_brief(
    session: StoredSession,
    profile: SwimmerProfile,
    previous: Vec<StoredSession>,
) -> String {
    let s: &Session = &session.session;
    let css = profile.css_s_per_100m;
    let sum = session_summary(s, css);
    let earlier = earlier_swims(&session, &previous, css);
    let mut out = vec![];

    out.push("Units: distances in metres, paces per 100 m, rests in seconds; a set done with fewer reps than planned was cut short.".into());
    out.push(format!(
        "Date: {}, pool {} m",
        date_utc(s.started_at_ms),
        s.workout.pool_length_m
    ));
    let name = s.workout.name.trim();
    let name = if name.is_empty() { "free swim" } else { name };
    out.push(format!(
        "Planned workout: {name}, {} m planned",
        total_distance_m(s.workout.clone())
    ));
    out.push(format!(
        "Total: {} m in {} elapsed ({} swimming, {} resting)",
        sum.distance_m,
        mmss(sum.elapsed_s),
        mmss(sum.swim_s),
        mmss(sum.rest_s)
    ));
    if let Some(p) = sum.pace_s_per_100m {
        out.push(format!("Average pace: {} /100 m", mmss(p)));
    }

    let set_list = sets(s);
    if !set_list.is_empty() {
        out.push("Sets:".into());
    }
    for (i, st) in set_list.iter().enumerate() {
        let planned = s
            .workout
            .blocks
            .get(st.block as usize)
            .and_then(|b| b.sets.get(st.set as usize));
        out.push(set_line(i, st, planned));
    }

    // Pace fade: first third vs last third of the reps, in swim order.
    let all = reps(s);
    let fade = (all.len() >= 3)
        .then(|| {
            let third = all.len() / 3;
            pace_of(&all[..third]).zip(pace_of(&all[all.len() - third..]))
        })
        .flatten();
    if let Some((first, last)) = fade {
        out.push(format!(
            "Pace fade: first third {} /100 m, last third {} /100 m ({:+.0} s/100 m)",
            mmss(first),
            mmss(last),
            last - first
        ));
    }

    if let Some(swolf) = sum.swolf_avg {
        out.push(format!("SWOLF avg {swolf:.0}"));
    }
    if let Some(rate) = sum.stroke_rate_per_min {
        out.push(format!("Stroke rate {rate:.0} /min"));
    }
    if let Some(dps) = sum.distance_per_stroke_m {
        out.push(format!("Distance per stroke {dps:.2} m"));
    }

    if let (Some(c), Some(p)) = (css, sum.pace_s_per_100m) {
        let diff = p - c;
        let side = if diff >= 0.0 { "slower" } else { "faster" };
        out.push(format!(
            "CSS {} /100 m: average pace {:.0} s/100 m {side} than CSS",
            mmss(c),
            diff.abs()
        ));
    }

    if earlier.is_empty() {
        out.push("Previous swims: none to compare".into());
    } else {
        out.push("Progress:".into());
        out.extend(
            progress_lines(&sum, &earlier, s.started_at_ms)
                .into_iter()
                .map(|l| format!("- {l}")),
        );
    }

    out.push("Achievements:".into());
    let achievements = achievement_lines(&session, &sum, &earlier, fade);
    if achievements.is_empty() {
        out.push("- None in these numbers".into());
    } else {
        out.extend(achievements.into_iter().map(|l| format!("- {l}")));
    }

    out.join("\n")
}

#[cfg(test)]
mod tests;
