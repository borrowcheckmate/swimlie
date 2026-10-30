use super::{
    Block, DEFAULT_REST_S, Equipment, ExerciseKind, FORMAT_VERSION, Intensity, NO_CSS_NOTE,
    SPRINT_REPS, Slot, Stroke, SwimLevel, SwimType, Template, Workout, WorkoutSet, add_rep, cut,
    exercise_set, fixed, main_slots, new_set, pace, pick, round_to_pool, spread_gear, swim_rest,
    timed, tr,
};

/// Reshapes the main sets without changing their distance: easy and moderate freestyle sets split (1 x 400 -> 2 x 200)
/// or merge (4 x 100 -> 2 x 200), then the order of the sets changes. `r` is the random pick.
pub(super) fn shuffle_main(main: &mut [Slot], pool: u32, mut r: u64) {
    let mut next = |n: usize| {
        r = r
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((r >> 33) as usize) % n
    };
    for i in 0..main.len() {
        let s = &main[i];
        if !(s.goal
            && s.set.stroke == Stroke::Freestyle
            && matches!(s.set.intensity, Intensity::Easy | Intensity::Moderate))
            || next(2) == 0
        {
            continue;
        }
        let (reps, d) = (s.set.reps, s.set.distance_m);
        let (reps, d) = if reps % 2 == 0 && d <= 200 {
            (reps / 2, d * 2)
        } else if d >= 200 && (d / 2) % pool == 0 {
            (reps * 2, d / 2)
        } else {
            continue;
        };
        // No two main sets may look alike (the grid rule).
        let same = |o: &Slot| {
            o.set.distance_m == d
                && o.set.stroke == main[i].set.stroke
                && o.set.intensity == main[i].set.intensity
        };
        if !main.iter().enumerate().any(|(j, o)| j != i && same(o)) {
            (main[i].set.reps, main[i].set.distance_m) = (reps, d);
        }
    }
    for i in (1..main.len()).rev() {
        main.swap(i, next(i + 1));
    }
}

/// A repeated round, one of six shapes picked by `r`: a rep of each stroke, a ladder, a pyramid, a descending
/// set, fast-slow-fast or long-to-short. The shapes after the first need one plain stroke (free, back or breast).
/// Returns the round's name and slots; reps, lengths and strokes vary with `r` too.
#[expect(
    clippy::too_many_lines,
    reason = "one linear pipeline whose steps share local state; splitting it is pending"
)]
pub(super) fn round_slots(pool: u32, strokes: &[Stroke], r: u64) -> (&'static str, Vec<Slot>) {
    let plain: Vec<Stroke> = strokes
        .iter()
        .copied()
        .filter(|x| {
            matches!(
                x,
                Stroke::Freestyle | Stroke::Backstroke | Stroke::Breaststroke
            )
        })
        .collect();
    let shape = if plain.is_empty() { 0 } else { (r >> 16) % 6 };
    if shape > 0 {
        let x = plain[r as usize % plain.len()];
        let (name, parts): (&str, &[(f64, Intensity)]) = match shape {
            1 => (
                "Ladder",
                &[
                    (50.0, Intensity::Moderate),
                    (100.0, Intensity::Moderate),
                    (150.0, Intensity::Threshold),
                    (200.0, Intensity::Threshold),
                ],
            ),
            2 => (
                "Pyramid",
                &[
                    (50.0, Intensity::Moderate),
                    (100.0, Intensity::Moderate),
                    (200.0, Intensity::Threshold),
                    (100.0, Intensity::Hard),
                    (50.0, Intensity::Sprint),
                ],
            ),
            3 => (
                "Descend",
                &[
                    (100.0, Intensity::Easy),
                    (100.0, Intensity::Moderate),
                    (100.0, Intensity::Threshold),
                    (100.0, Intensity::Hard),
                ],
            ),
            4 => (
                "Fast-slow-fast",
                &[
                    (50.0, Intensity::Sprint),
                    (100.0, Intensity::Easy),
                    (50.0, Intensity::Hard),
                ],
            ),
            _ => (
                "Long to short",
                &[
                    (200.0, Intensity::Moderate),
                    (100.0, Intensity::Threshold),
                    (50.0, Intensity::Hard),
                ],
            ),
        };
        let total: f64 = parts.iter().map(|p| p.0).sum();
        return (
            name,
            parts
                .iter()
                .map(|&(m, i)| {
                    timed(
                        m / total,
                        new_set(round_to_pool(m, pool), x, i, vec![], None),
                    )
                })
                .collect(),
        );
    }
    let picked: Vec<Stroke> = (0..strokes.len().min(3))
        .map(|k| strokes[(r as usize + k) % strokes.len()])
        .collect();
    let unit: f64 = [100.0, 100.0, 50.0, 200.0][(r >> 8) as usize % 4];
    let reps = [1, 2, 1, 3][(r >> 12) as usize % 4];
    let dist = |x: Stroke| match x {
        Stroke::Mixed => round_to_pool(unit.max(100.0), 4 * pool),
        Stroke::Butterfly => round_to_pool(50.0, pool),
        _ => round_to_pool(unit, pool),
    };
    let total: u32 = picked.iter().map(|&x| dist(x)).sum();
    (
        "Stroke round",
        picked
            .into_iter()
            .map(|x| {
                timed(
                    f64::from(dist(x)) / f64::from(total),
                    WorkoutSet {
                        reps,
                        ..new_set(dist(x), x, Intensity::Moderate, vec![], None)
                    },
                )
            })
            .collect(),
    )
}

/// Spreads the swimmer's chosen strokes over the main sets (kick and drill sets stay): IM needs a multiple of
/// four lengths, butterfly stays at 100 m or less and not at threshold or sprint pace; a set that fits none keeps its stroke.
pub(super) fn spread_strokes(main: &mut [Slot], pool: u32, strokes: &[Stroke], r: u64) {
    if strokes.is_empty() {
        return;
    }
    let swims = |s: &Slot| {
        s.goal
            && matches!(
                s.set.stroke,
                Stroke::Freestyle
                    | Stroke::Backstroke
                    | Stroke::Breaststroke
                    | Stroke::Butterfly
                    | Stroke::Mixed
            )
    };
    let first = (r >> 20) as usize;
    for (n, s) in (first..).zip(main.iter_mut().filter(|s| swims(s))) {
        let (d, hard) = (
            s.set.distance_m,
            matches!(
                s.set.intensity,
                Intensity::Threshold | Intensity::Hard | Intensity::Sprint
            ),
        );
        let fits = |x: &Stroke| match x {
            Stroke::Mixed => d % (4 * pool) == 0,
            Stroke::Butterfly => d <= 100 && !hard,
            _ => true,
        };
        if let Some(x) = (0..strokes.len())
            .map(|k| strokes[(n + k) % strokes.len()])
            .find(fits)
        {
            s.set.stroke = x;
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the generator inputs one to one"
)]
#[expect(
    clippy::too_many_lines,
    reason = "one linear pipeline whose steps share local state; splitting it is pending"
)]
pub(super) fn build_shuffled(
    kind: SwimType,
    pool: u32,
    equipment: &[Equipment],
    css: Option<f64>,
    level: SwimLevel,
    distance_m: u32,
    variant: u32,
    seed: u32,
    blocks: bool,
    shuffle: Option<(u64, &[Stroke])>,
    lang: &str,
) -> Workout {
    let d = f64::from(distance_m);
    let cool = round_to_pool(0.1 * d, pool)
        .min(round_to_pool(300.0, pool))
        .max(2 * pool);
    let build_set = (matches!(kind, SwimType::Threshold | SwimType::Speed) && distance_m >= 1500)
        .then(|| WorkoutSet {
            reps: 4,
            rest: fixed(15),
            ..new_set(
                round_to_pool(50.0, pool),
                Stroke::Freestyle,
                Intensity::Moderate,
                vec![],
                Some(tr(lang, "Build each 50")),
            )
        });
    // 4-rep skill sets: in the warm-up from 1000 m, and after the main block from 1500 m (technique has drills in its main block).
    let skill = |kinds: &[ExerciseKind], n: u32| {
        pick(kinds, equipment, blocks, n, lang).map(|e| exercise_set(&e, 4, pool))
    };
    let warm_kinds: &[ExerciseKind] = if kind == SwimType::Technique {
        &[ExerciseKind::Wall]
    } else {
        &[ExerciseKind::Drill, ExerciseKind::Wall]
    };
    let end_kinds: &[ExerciseKind] = match (kind, blocks) {
        (SwimType::EasyRecovery, _) => &[ExerciseKind::Fun],
        (SwimType::Speed, true) => &[ExerciseKind::Start],
        _ => &[ExerciseKind::Fun, ExerciseKind::Start],
    };
    let mut warm_skill = (distance_m >= 1000)
        .then(|| skill(warm_kinds, seed))
        .flatten();
    let mut end_skill = (distance_m >= 1500 && kind != SwimType::Technique)
        .then(|| skill(end_kinds, seed))
        .flatten();

    // Two or more chosen strokes make the main block a round (a few reps of each stroke), the block repeated.
    // With no choice (and no gear, which rounds do not carry) a third of the shuffles make one anyway, for fun.
    let (r, chosen) = shuffle.map_or((0, &[][..]), |(r, st)| (r, st));
    let fun = shuffle.is_some()
        && distance_m >= 1000
        && chosen.is_empty()
        && equipment.is_empty()
        && !blocks
        && !matches!(kind, SwimType::EasyRecovery)
        && (r >> 40) % 2 == 0;
    let strokes: Vec<Stroke> = if fun {
        vec![Stroke::Freestyle, Stroke::Backstroke, Stroke::Breaststroke]
    } else {
        chosen.to_vec()
    };
    let round = (kind != SwimType::Technique && strokes.len() >= 2)
        .then_some(())
        .and_then(|()| {
            // The varied round first; a plain 1 x 100 m of each when that leaves fewer than two rounds.
            [r >> 20, 0]
                .into_iter()
                .map(|x| round_slots(pool, &strokes, x))
                .find_map(|(name, slots)| {
                    let m: u32 = slots.iter().map(|s| s.set.reps * s.set.distance_m).sum();
                    let n = ((0.65 * d).max(d - f64::from(cool) - 1000.0) / f64::from(m.max(1)))
                        .round() as u32; // the warm-up stays under 1000 m
                    (n >= 2).then_some((name, slots, n))
                })
        });
    let rounds = round.is_some();
    let mut round_n = round.as_ref().map_or(1, |r| r.2);
    if rounds {
        end_skill = None;
    }
    let in_reps: Vec<u32> = round
        .as_ref()
        .map_or(vec![], |r| r.1.iter().map(|s| s.set.reps).collect());
    let round_name = round.as_ref().map_or("Main", |r| r.0);
    let mut main = match round {
        Some((_, slots, _)) => slots,
        None => main_slots(
            kind,
            &Template {
                pool,
                have: equipment,
                variant,
                seed,
                lang,
            },
        ),
    };
    // A beginner gets no sprint or hard sets: they become threshold and moderate.
    if level == SwimLevel::Beginner {
        for s in &mut main {
            s.set.intensity = match s.set.intensity {
                Intensity::Sprint => Intensity::Threshold,
                Intensity::Hard => Intensity::Moderate,
                i => i,
            };
        }
    }
    // The end skill set is part of the main block, so the timed sets share what it leaves.
    let end_m = end_skill.as_ref().map_or(0, |s| s.reps * s.distance_m);
    let main_m = 0.65f64.mul_add(d, -f64::from(end_m));
    for s in &mut main {
        // A round keeps its reps; the block repeats instead.
        s.set.reps = if rounds {
            s.set.reps * round_n
        } else {
            ((s.share * main_m / f64::from(s.set.distance_m)).round() as u32).max(1)
        };
    }
    let mut excess_m = 0;
    for s in main
        .iter_mut()
        .filter(|s| !rounds && s.set.intensity == Intensity::Sprint && s.set.reps > SPRINT_REPS)
    {
        excess_m += (s.set.reps - SPRINT_REPS) * s.set.distance_m;
        s.set.reps = SPRINT_REPS;
    }
    if let Some(s) = main
        .iter_mut()
        .find(|s| s.set.intensity == Intensity::Moderate)
    {
        s.set.reps += (f64::from(excess_m) / f64::from(s.set.distance_m)).round() as u32;
    }
    if let Some((r, strokes)) = shuffle
        && !rounds
    {
        shuffle_main(&mut main, pool, r);
        spread_strokes(&mut main, pool, strokes, r);
    }
    // The warm-up takes what is left of D, after the sets outside the main slots.
    let sets_m = |s: &Option<WorkoutSet>| s.as_ref().map_or(0, |s| s.reps * s.distance_m);
    let warm_of =
        |main: &[Slot], warm_skill: &Option<WorkoutSet>, end_skill: &Option<WorkoutSet>| {
            let main_sum: u32 = main.iter().map(|s| s.set.reps * s.set.distance_m).sum();
            round_to_pool(
                (d - f64::from(
                    main_sum + sets_m(warm_skill) + sets_m(end_skill) + sets_m(&build_set) + cool,
                ))
                .max(0.0),
                pool,
            )
        };
    // The easy warm-up keeps its floor, rounded down to the pool. Below it, the skill sets give way first (warm-up
    // skill, then end skill, each 4 -> 2 -> none); only then does a rep come out of the largest main set. A set with
    // one rep left goes once every set is down to one (small distances, rounded-up reps).
    let floor = (0.10 * d).max(2.0 * f64::from(pool));
    let floor_m = (floor / f64::from(pool)).floor() as u32 * pool;
    let mut warm = warm_of(&main, &warm_skill, &end_skill);
    while warm < floor_m {
        if warm_skill.is_some() {
            cut(&mut warm_skill);
        } else if end_skill.is_some() {
            cut(&mut end_skill);
        } else if rounds {
            // Rounding up left no room for the warm-up: one round less.
            if round_n == 1 {
                break;
            }
            round_n -= 1;
            main.iter_mut()
                .zip(&in_reps)
                .for_each(|(s, r)| s.set.reps = r * round_n);
        } else {
            let Some(i) = (0..main.len()).max_by_key(|&i| {
                (
                    main[i].set.reps > 1,
                    main[i].set.reps * main[i].set.distance_m,
                )
            }) else {
                break;
            };
            main[i].set.reps -= 1;
            if main[i].set.reps == 0 {
                main.remove(i);
            }
        }
        warm = warm_of(&main, &warm_skill, &end_skill);
    }
    // Rounding leaves main short: reps move from the warm-up while it stays above the floor.
    // Then the warm-up is capped at 1000 m and the rest (cool-down beyond 300 m too) goes to main.
    while !rounds && add_rep(&mut main, main_m, f64::from(warm) - floor) {
        warm = warm_of(&main, &warm_skill, &end_skill);
    }
    while !rounds && warm > 1000 && add_rep(&mut main, main_m, f64::MAX) {
        warm = warm_of(&main, &warm_skill, &end_skill);
    }

    let mut warm_sets = vec![new_set(
        warm,
        Stroke::Any,
        Intensity::Easy,
        vec![],
        css.is_none().then(|| tr(lang, NO_CSS_NOTE)),
    )];
    warm_sets.extend(warm_skill);
    // The build set follows the chosen strokes too (a plain stroke: no fly or IM builds).
    warm_sets.extend(build_set.map(|mut b| {
        let chosen = shuffle.map_or(&[][..], |(_, st)| st);
        if let Some(x) = chosen
            .iter()
            .find(|x| {
                matches!(
                    x,
                    Stroke::Freestyle | Stroke::Backstroke | Stroke::Breaststroke
                )
            })
            .filter(|_| !chosen.contains(&b.stroke))
        {
            b.stroke = *x;
        }
        b
    }));
    let mut sets: Vec<WorkoutSet> = main
        .into_iter()
        .enumerate()
        .map(|(i, Slot { goal, mut set, .. })| {
            if rounds {
                set.reps = in_reps[i];
            }
            if goal {
                let goal = (pace(
                    css.or_else(|| Some(level.no_css_pace())),
                    set.stroke,
                    set.intensity,
                ) * f64::from(set.distance_m)
                    / 100.0)
                    .round() as u32;
                set.goal_s = Some(goal);
                let rest_x = if css.is_none() {
                    level.no_css_rest_x()
                } else {
                    1.0
                };
                set.rest = Some(swim_rest(&set, goal, rest_x));
            } else {
                set.rest = fixed(DEFAULT_REST_S);
            }
            set
        })
        .collect();
    spread_gear(&mut sets, equipment);
    // Added after the rest mapping, so the fixed 20 s rest stays.
    sets.extend(end_skill);

    let mut w = Workout {
        version: FORMAT_VERSION,
        name: String::new(),
        pool_length_m: pool,
        blocks: vec![
            Block {
                name: tr(lang, "Warm-up").into(),
                repeat: 1,
                sets: warm_sets,
            },
            Block {
                name: if rounds {
                    format!("{}: {}", tr(lang, "Main"), tr(lang, round_name))
                } else {
                    tr(lang, "Main").into()
                },
                repeat: round_n,
                sets,
            },
            Block {
                name: tr(lang, "Cool-down").into(),
                repeat: 1,
                sets: vec![new_set(cool, Stroke::Any, Intensity::Easy, vec![], None)],
            },
        ],
        description: None,
    };
    w.name = format!("{} {} m", kind.name(lang), w.total_distance_m());
    w
}
