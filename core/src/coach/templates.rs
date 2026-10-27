use super::{
    Equipment, Intensity, Slot, Stroke, SwimType, drill_slot, gear, new_set, round_to_pool, timed,
    tr, untimed, with_kick,
};
use Intensity::{Easy, Moderate, Sprint, Threshold};
use Stroke::{Any, Backstroke, Breaststroke, Butterfly, Freestyle, Kickboard, Mixed};

/// Everything besides the swim type that the main-block templates depend on.
#[derive(Clone, Copy, Debug)]
pub(super) struct Template<'a> {
    pub(super) pool: u32,
    pub(super) have: &'a [Equipment],
    pub(super) variant: u32,
    pub(super) seed: u32,
    pub(super) lang: &'a str,
}

/// Main-block templates of a type, with distances rounded to the pool.
pub(super) fn main_slots(kind: SwimType, t: &Template) -> Vec<Slot> {
    match kind {
        SwimType::EasyRecovery => easy_recovery(t),
        SwimType::Endurance => endurance(t),
        SwimType::Threshold => threshold(t),
        SwimType::Speed => speed(t),
        SwimType::Technique => technique(t),
        SwimType::Mixed => mixed(t),
    }
}

fn easy_recovery(t: &Template) -> Vec<Slot> {
    let Template {
        pool,
        have,
        variant,
        ..
    } = *t;
    let rp = |m: u32| round_to_pool(f64::from(m), pool);
    vec![
        timed(0.5, new_set(rp(200), Freestyle, Easy, vec![], None)),
        timed(
            0.25,
            new_set(
                rp(100),
                if variant.is_multiple_of(2) {
                    Backstroke
                } else {
                    Breaststroke
                },
                Easy,
                vec![],
                None,
            ),
        ),
        if !gear(have, &[Equipment::PullBuoy, Equipment::Paddles]).is_empty() {
            timed(
                0.25,
                new_set(
                    rp(100),
                    Freestyle,
                    Easy,
                    gear(have, &[Equipment::PullBuoy, Equipment::Paddles]),
                    None,
                ),
            )
        } else if have.contains(&Equipment::Kickboard) {
            untimed(
                0.25,
                new_set(rp(50), Kickboard, Easy, vec![Equipment::Kickboard], None),
            )
        } else {
            timed(0.25, new_set(rp(100), Freestyle, Easy, vec![], None))
        },
    ]
}

fn endurance(t: &Template) -> Vec<Slot> {
    let Template {
        pool,
        have,
        variant,
        ..
    } = *t;
    let rp = |m: u32| round_to_pool(f64::from(m), pool);
    let pull = gear(have, &[Equipment::PullBuoy, Equipment::Paddles]);
    with_kick(
        match variant % 4 {
            0 => vec![
                timed(0.6, new_set(rp(400), Freestyle, Moderate, vec![], None)),
                timed(0.4, new_set(rp(200), Freestyle, Moderate, pull, None)),
            ],
            1 => vec![
                timed(0.6, new_set(rp(400), Freestyle, Moderate, vec![], None)),
                timed(0.4, new_set(rp(100), Backstroke, Moderate, vec![], None)),
            ],
            2 => vec![
                timed(0.55, new_set(rp(400), Freestyle, Moderate, vec![], None)),
                timed(0.25, new_set(rp(200), Freestyle, Moderate, pull, None)),
                timed(0.2, new_set(rp(100), Breaststroke, Easy, vec![], None)),
            ],
            _ => vec![
                timed(0.4, new_set(rp(200), Freestyle, Moderate, vec![], None)),
                timed(0.3, new_set(rp(200), Backstroke, Moderate, vec![], None)),
                timed(0.3, new_set(rp(300), Freestyle, Moderate, pull, None)),
            ],
        },
        pool,
        have,
    )
}

fn threshold(t: &Template) -> Vec<Slot> {
    let Template {
        pool,
        have,
        variant,
        ..
    } = *t;
    let rp = |m: u32| round_to_pool(f64::from(m), pool);
    // Variants change the second set: 200 moderate, 200 threshold, then 300 threshold.
    let (second, stroke, intensity) = match variant % 4 {
        0 => (200, Freestyle, Moderate),
        1 => (200, Freestyle, Threshold),
        2 => (300, Freestyle, Threshold),
        _ => (200, Backstroke, Moderate),
    };
    with_kick(
        vec![
            timed(0.7, new_set(rp(100), Freestyle, Threshold, vec![], None)),
            timed(0.3, new_set(rp(second), stroke, intensity, vec![], None)),
        ],
        pool,
        have,
    )
}

/// From 50 m pools the 25 m sprint is dropped and its share goes to the 50.
fn speed(t: &Template) -> Vec<Slot> {
    if t.pool >= 50 {
        speed_long_pool(t)
    } else {
        speed_short_pool(t)
    }
}

fn speed_long_pool(t: &Template) -> Vec<Slot> {
    let Template {
        pool,
        have,
        variant,
        ..
    } = *t;
    let rp = |m: u32| round_to_pool(f64::from(m), pool);
    let second_stroke = if variant.is_multiple_of(2) {
        Freestyle
    } else {
        Backstroke
    };
    with_kick(
        vec![
            timed(
                0.6,
                new_set(
                    rp(50),
                    Freestyle,
                    Sprint,
                    gear(have, &[Equipment::Fins]),
                    None,
                ),
            ),
            timed(0.4, new_set(rp(100), second_stroke, Moderate, vec![], None)),
        ],
        pool,
        have,
    )
}

fn speed_short_pool(t: &Template) -> Vec<Slot> {
    let Template {
        pool,
        have,
        variant,
        ..
    } = *t;
    let rp = |m: u32| round_to_pool(f64::from(m), pool);
    let middle_stroke = if variant % 3 == 1 {
        Backstroke
    } else {
        Freestyle
    };
    let sprint_stroke = match variant % 3 {
        0 => Freestyle,
        1 => Backstroke,
        _ => Butterfly,
    };
    with_kick(
        vec![
            timed(
                0.35,
                new_set(
                    rp(50),
                    Freestyle,
                    Sprint,
                    gear(have, &[Equipment::Fins]),
                    None,
                ),
            ),
            timed(0.4, new_set(rp(100), middle_stroke, Moderate, vec![], None)),
            timed(0.25, new_set(rp(25), sprint_stroke, Sprint, vec![], None)),
        ],
        pool,
        have,
    )
}

fn technique(t: &Template) -> Vec<Slot> {
    let Template {
        pool,
        have,
        variant,
        seed,
        lang,
        ..
    } = *t;
    let rp = |m: u32| round_to_pool(f64::from(m), pool);
    vec![
        drill_slot(2 * seed, have, pool, lang),
        drill_slot(2 * seed + 1, have, pool, lang),
        if have.contains(&Equipment::Kickboard) {
            untimed(
                0.25,
                new_set(
                    rp(50),
                    Kickboard,
                    Easy,
                    gear(have, &[Equipment::Kickboard, Equipment::Fins]),
                    None,
                ),
            )
        } else {
            untimed(
                0.25,
                new_set(
                    rp(50),
                    Any,
                    Easy,
                    gear(have, &[Equipment::Fins]),
                    Some(tr(lang, "Kick on your back")),
                ),
            )
        },
        timed(
            0.45,
            new_set(
                rp(100),
                match variant % 3 {
                    0 => Freestyle,
                    1 => Backstroke,
                    _ => Breaststroke,
                },
                Moderate,
                vec![],
                Some(tr(lang, "Count strokes")),
            ),
        ),
    ]
}

fn mixed(t: &Template) -> Vec<Slot> {
    let Template { pool, have, .. } = *t;
    let rp = |m: u32| round_to_pool(f64::from(m), pool);
    vec![
        timed(
            0.3,
            new_set(
                round_to_pool(100.0, 4 * pool),
                Mixed,
                Moderate,
                vec![],
                None,
            ),
        ),
        if have.contains(&Equipment::Kickboard) {
            untimed(
                0.2,
                new_set(rp(50), Kickboard, Easy, vec![Equipment::Kickboard], None),
            )
        } else {
            timed(0.2, new_set(rp(50), Backstroke, Easy, vec![], None))
        },
        timed(0.2, new_set(rp(50), Breaststroke, Moderate, vec![], None)),
        timed(0.3, new_set(rp(100), Freestyle, Threshold, vec![], None)),
    ]
}
