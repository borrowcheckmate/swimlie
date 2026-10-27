use super::{Session, Stroke, counted, hr_stats, hr_zone, laps, reps, stroke_stats, swim_tss};

/// Totals of one session for the history list and charts.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SessionSummary {
    /// Sum of `lap_lengths` x pool length.
    pub distance_m: u32,
    /// Sum of the reps' `swim_s`: turns in, rests between reps out.
    pub swim_s: f64,
    /// Last lap end minus first lap start (rests included); 0 without laps.
    pub elapsed_s: f64,
    /// `swim_s / distance_m * 100`; `None` if distance is 0.
    pub pace_s_per_100m: Option<f64>,
    /// Mean of all per-lap `Some` SWOLF values over all reps.
    pub swolf_avg: Option<f64>,
    /// `swim_tss(session, css)`.
    pub tss: Option<f64>,
    /// `elapsed_s - swim_s`.
    pub rest_s: f64,
    /// Lengths swum: sum of `lap_lengths`.
    pub laps: u32,
    /// Sum of the laps' stroke counts; `None` if no lap has one.
    pub strokes: Option<u32>,
    pub stroke_rate_per_min: Option<f64>,
    pub distance_per_stroke_m: Option<f64>,
    pub avg_bpm: Option<f64>,
    pub max_bpm: Option<f64>,
    /// Seconds in HR zone 1..=5. Empty without HR samples or age. Each sample counts until the next, at most 30 s.
    pub zone_s: Vec<f64>,
    /// Per stroke, most distance first.
    pub by_stroke: Vec<StrokeSplit>,
}

/// Totals of one stroke over a session's laps.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct StrokeSplit {
    pub stroke: Stroke,
    /// Number of lengths in this stroke (a merged lap counts 2).
    pub lengths: u32,
    pub distance_m: u32,
    pub swim_s: f64,
    pub pace_s_per_100m: f64,
}

#[uniffi::export]
#[must_use]
pub fn session_summary(session: &Session, css_s_per_100m: Option<f64>) -> SessionSummary {
    let rs = reps(session);
    let distance_m: u32 = rs.iter().map(|r| r.distance_m).sum();
    let swim_s: f64 = rs.iter().map(|r| r.swim_s).sum();
    let elapsed_s = match (session.laps.first(), session.laps.last()) {
        (Some(first), Some(last)) => last.end_s - first.start_s,
        _ => 0.0,
    };
    let known: Vec<f64> = rs
        .iter()
        .flat_map(|r| r.swolf.iter().flatten().copied())
        .collect();
    let counted_laps = counted(session, 0..session.laps.len());
    let (stroke_rate_per_min, distance_per_stroke_m) = stroke_stats(&counted_laps);
    let mut splits: Vec<(Stroke, u32, u32, f64)> = vec![];
    for l in laps(session) {
        match splits.iter_mut().find(|s| s.0 == l.stroke) {
            Some(s) => {
                s.1 += l.lengths;
                s.2 += l.distance_m;
                s.3 += l.swim_s;
            }
            None => splits.push((l.stroke, l.lengths, l.distance_m, l.swim_s)),
        }
    }
    let mut by_stroke: Vec<StrokeSplit> = splits
        .into_iter()
        .map(|(stroke, n, distance_m, swim_s)| StrokeSplit {
            stroke,
            lengths: n,
            distance_m,
            swim_s,
            pace_s_per_100m: swim_s / f64::from(distance_m) * 100.0,
        })
        .collect();
    by_stroke.sort_by_key(|a| std::cmp::Reverse(a.distance_m));
    let zone_s = match session.age_years {
        Some(age) if !session.hr.is_empty() => {
            let mut z = vec![0.0; 5];
            // Gaps over 30 s are signal dropouts (real swims: median gap 5 s, some 10-30 s), not time spent in the zone.
            for w in session.hr.windows(2) {
                if let Some(zone) = hr_zone(w[0].bpm, age) {
                    z[zone as usize - 1] += (w[1].t_s - w[0].t_s).min(30.0);
                }
            }
            z
        }
        _ => vec![],
    };
    let (avg_bpm, max_bpm) = hr_stats(session.hr.iter().map(|h| h.bpm));
    SessionSummary {
        distance_m,
        swim_s,
        elapsed_s,
        pace_s_per_100m: (distance_m > 0).then(|| swim_s / f64::from(distance_m) * 100.0),
        swolf_avg: (!known.is_empty()).then(|| known.iter().sum::<f64>() / known.len() as f64),
        tss: swim_tss(session, css_s_per_100m),
        rest_s: elapsed_s - swim_s,
        laps: session.lap_lengths.iter().sum(),
        strokes: (!counted_laps.is_empty()).then(|| counted_laps.iter().map(|c| c.0).sum()),
        stroke_rate_per_min,
        distance_per_stroke_m,
        avg_bpm,
        max_bpm,
        zone_s,
        by_stroke,
    }
}
