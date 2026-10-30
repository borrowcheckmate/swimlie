//! Exercise catalogue: drills, wall skills, starts and games. Each has a `YouTube` search link until a video is pinned.

use crate::i18n;
use crate::model::{Equipment, Stroke};

const SEARCH_URL: &str = "https://www.youtube.com/results?search_query=";
pub(crate) const BREATH_HOLD: &str = "Keep breath-holds short, never hyperventilate before, and never do them alone: shallow-water blackout has no warning.";

/// What an exercise is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ExerciseKind {
    /// Technique drill.
    Drill,
    /// Skills at the wall.
    Wall,
    /// Dive start.
    Start,
    /// Game.
    Fun,
}

/// One catalogue entry, ready for the apps.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Exercise {
    /// Kebab-case, stable.
    pub id: String,
    pub name: String,
    /// One short line for the watch, 40 characters or fewer.
    pub cue: String,
    /// 1-3 sentences for the iPhone.
    pub how_to: String,
    /// The pinned URL if set, else a `YouTube` search URL.
    pub video_url: String,
    /// `Any` for entries that are not stroke-specific.
    pub stroke: Stroke,
    /// Equipment the exercise needs (all of it).
    pub equipment: Vec<Equipment>,
    pub kind: ExerciseKind,
    /// Nominal rep distance: 50, or 25 for starts.
    pub rep_m: u32,
    /// Set on breath-hold and underwater entries.
    pub safety: Option<String>,
}

/// Catalogue data. `search` is the `YouTube` search words: letters, digits and spaces only.
struct Entry {
    id: &'static str,
    name: &'static str,
    cue: &'static str,
    how_to: &'static str,
    search: &'static str,
    pinned: Option<&'static str>,
    stroke: Stroke,
    equipment: &'static [Equipment],
    kind: ExerciseKind,
    rep_m: u32,
    safety: Option<&'static str>,
}

impl Entry {
    /// The exercise in `lang`: name and cue in Dutch for "nl", else English.
    fn exercise(&self, lang: &str) -> Exercise {
        let (name, cue, how_to) = i18n::drill(self.id, lang, (self.name, self.cue, self.how_to));
        Exercise {
            id: self.id.into(),
            name: name.into(),
            cue: cue.into(),
            how_to: how_to.into(),
            video_url: self.pinned.map_or_else(
                || format!("{SEARCH_URL}{}", self.search.replace(' ', "+")),
                String::from,
            ),
            stroke: self.stroke,
            equipment: self.equipment.to_vec(),
            kind: self.kind,
            rep_m: self.rep_m,
            safety: self.safety.map(|s| i18n::tr(lang, s).into()),
        }
    }
}

const CATALOGUE: &[Entry] = &[
    Entry {
        id: "catch-up",
        name: "Catch-up drill",
        cue: "Next stroke starts when the hands meet",
        how_to: "Swim freestyle, but start each arm only when the other hand touches in front. This forces a long glide and a slow, even stroke. Keep the kick steady.",
        search: "catch up drill freestyle swimming",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "fingertip-drag",
        name: "Fingertip drag",
        cue: "Trail fingertips low on each recovery",
        how_to: "Freestyle with the fingertips skimming the water on recovery, elbow high. The arm stays relaxed and close to the surface.",
        search: "fingertip drag freestyle drill",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "single-arm",
        name: "Single arm freestyle",
        cue: "One arm swims, the other rests in front",
        how_to: "Swim freestyle with one arm only, the other resting in front or at your side. Switch arms every 25 m. Roll with the hips so the swimming arm stays in line.",
        search: "single arm freestyle drill",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "six-kick-switch",
        name: "6-kick switch",
        cue: "Six kicks on each side, then switch",
        how_to: "Lie on your side, one arm out front and the other at your hip. Kick six beats, roll to the other side and repeat. This builds body line and rotation.",
        search: "6 kick switch drill freestyle",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "fist-swim",
        name: "Fist swim",
        cue: "Closed fists: feel the forearm pull",
        how_to: "Swim freestyle with closed fists, so the forearm does the catching, because the palm cannot. Open the hands after a few strokes to feel the difference.",
        search: "fist swimming drill freestyle",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "zipper",
        name: "Zipper drill",
        cue: "Thumb runs up the side, hip to armpit",
        how_to: "Freestyle where the thumb runs up the side of the body from the hip to the armpit during the recovery. The arm recovers close to the body with a high elbow.",
        search: "zipper drill freestyle swimming",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "head-up",
        name: "Head-up freestyle (Tarzan)",
        cue: "Keep the head up, eyes on the far wall",
        how_to: "Freestyle with the head held high and eyes forward. The hips and legs sink, so kick harder to float. Use it to feel body balance, not as your normal stroke.",
        search: "tarzan freestyle drill head up",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "front-scull",
        name: "Front scull",
        cue: "Sculling with the hands, body flat",
        how_to: "Lie on your front with a pull buoy between your thighs and your arms out in front. Sweep the hands in small figure-eights, palms angled to push the water.",
        search: "front scull swimming drill",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[Equipment::PullBuoy],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "breaststroke-2-kicks-1-pull",
        name: "Breaststroke 2 kicks 1 pull",
        cue: "Two kicks, then one pull, long glide",
        how_to: "Breaststroke with two kicks and one pull per cycle, and a long glide between them. Keep the head down and the arms reaching forward.",
        search: "breaststroke 2 kick 1 pull drill",
        pinned: None,
        stroke: Stroke::Breaststroke,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "backstroke-single-arm",
        name: "Backstroke single arm",
        cue: "One arm by your side, rotate the hips",
        how_to: "Backstroke with one arm at your side and the other swimming a full pull. Switch arms each length. Rotate the hips and shoulders with every stroke.",
        search: "backstroke single arm drill",
        pinned: None,
        stroke: Stroke::Backstroke,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "backstroke-spin",
        name: "Backstroke spin",
        cue: "Very fast arms, short strokes, hips high",
        how_to: "Backstroke with very fast arm turnover: short, quick strokes while the hips and legs stay high, a few metres at a time. Count strokes from the flags so you don't hit the wall.",
        search: "backstroke spin drill",
        pinned: None,
        stroke: Stroke::Backstroke,
        equipment: &[],
        kind: ExerciseKind::Drill,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "streamline-push-off",
        name: "Streamline push-off",
        cue: "Arms locked overhead, squeeze, kick out",
        how_to: "Push off the wall on your front, arms stacked overhead with the biceps by the ears. Hold the streamline for a few kicks, then come up gently and swim.",
        search: "streamline push off wall swimming",
        pinned: None,
        stroke: Stroke::Any,
        equipment: &[],
        kind: ExerciseKind::Wall,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "roll-off-wall",
        name: "Roll off the wall onto the front",
        cue: "Push on your back, roll to your front",
        how_to: "Hold the wall on your back, push off and roll onto your front in one smooth move, arms reaching into a streamline. Practise it before the flip turn.",
        search: "roll onto front off wall swimming drill",
        pinned: None,
        stroke: Stroke::Any,
        equipment: &[],
        kind: ExerciseKind::Wall,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "flip-turn",
        name: "Flip turn",
        cue: "Somersault, push off back, roll to front",
        how_to: "Freestyle flip turn: somersault at the wall, push off on your back or side, then rotate to the front during the streamline. Exhale through your nose while turning, and start the turn far enough from the wall that your heels don't hit it. Learn it slowly first.",
        search: "freestyle flip turn tutorial",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Wall,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "open-turn",
        name: "Open turn",
        cue: "Touch, turn your body, feet on the wall",
        how_to: "Touch the wall with one hand, then turn the body to plant both feet on the wall and push off on your front. Breathe as you turn. Easier to learn than a flip turn.",
        search: "open turn freestyle swimming",
        pinned: None,
        stroke: Stroke::Any,
        equipment: &[],
        kind: ExerciseKind::Wall,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "dive-and-swim",
        name: "Dive from the block and swim easy",
        cue: "Deep end only: shallow dive, then easy",
        how_to: "Dive into a streamline, swim the first 15 m fast, then easy back to the block end (the coach makes this rep at least two lengths).",
        search: "dive from the block swimming start",
        pinned: None,
        stroke: Stroke::Any,
        equipment: &[],
        kind: ExerciseKind::Start,
        rep_m: 25,
        safety: Some(
            "Dive only where the pool allows it and the water is at least 1.35 m deep (the competition minimum), and only if you were taught: a shallow racing dive, hands locked, head between the arms. Diving into shallow water causes spinal injuries.",
        ),
    },
    Entry {
        id: "touch-the-floor",
        name: "Touch the floor 3 times in a 50",
        cue: "Touch the floor 3x in a 50, never alone",
        how_to: "Swim a 50 and touch the pool floor three times with short dives. Breathe normally between touches, and stop if you feel dizzy. Only where the floor is in easy reach.",
        search: "touch the bottom of the pool swimming game",
        pinned: None,
        stroke: Stroke::Any,
        equipment: &[],
        kind: ExerciseKind::Fun,
        rep_m: 50,
        safety: Some(BREATH_HOLD),
    },
    Entry {
        id: "fewest-strokes",
        name: "Fewest strokes",
        cue: "Swim a 50 in as few strokes as you can",
        how_to: "Swim one 50 with the fewest strokes you can, gliding long and kicking steadily. Count your strokes, then aim for one fewer each time at an easy speed.",
        search: "fewest strokes swimming challenge",
        pinned: None,
        stroke: Stroke::Any,
        equipment: &[],
        kind: ExerciseKind::Fun,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "golf",
        name: "Golf",
        cue: "Score = seconds + strokes per length",
        how_to: "Swim a set of lengths and score each one: seconds plus strokes. The lowest total wins. A long glide saves strokes but costs time, so find your balance.",
        search: "swimming golf game time strokes",
        pinned: None,
        stroke: Stroke::Any,
        equipment: &[],
        kind: ExerciseKind::Fun,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "kick-race",
        name: "Kick race against yourself",
        cue: "Kick a 50 on a board, beat your time",
        how_to: "Kick a 50 on your front holding a kickboard, and time it. Rest, then race your own time. Keep the kick from the hips with soft knees.",
        search: "kick race kickboard freestyle swimming",
        pinned: None,
        stroke: Stroke::Kickboard,
        equipment: &[Equipment::Kickboard],
        kind: ExerciseKind::Fun,
        rep_m: 50,
        safety: None,
    },
    Entry {
        id: "breathe-every-5th",
        name: "Breathe every 5th stroke",
        cue: "Breathe every 5th stroke, never alone",
        how_to: "Freestyle with one breath every fifth stroke, so you hold a short breath each cycle. Breathe out steadily under water. Keep the set easy and stop if you feel dizzy.",
        search: "breathe every 5th stroke freestyle",
        pinned: None,
        stroke: Stroke::Freestyle,
        equipment: &[],
        kind: ExerciseKind::Fun,
        rep_m: 50,
        safety: Some(BREATH_HOLD),
    },
];

/// Catalogue in order.
#[uniffi::export]
#[must_use]
pub fn exercises() -> Vec<Exercise> {
    CATALOGUE.iter().map(|e| e.exercise("en")).collect()
}

/// One exercise by id in `lang` ("nl" for Dutch, else English); `None` if unknown.
#[uniffi::export]
#[must_use]
pub fn exercise_in(id: String, lang: String) -> Option<Exercise> {
    CATALOGUE
        .iter()
        .find(|e| e.id == id)
        .map(|e| e.exercise(&lang))
}

/// One exercise by id; `None` if unknown.
#[uniffi::export]
#[must_use]
pub fn exercise(id: String) -> Option<Exercise> {
    CATALOGUE
        .iter()
        .find(|e| e.id == id)
        .map(|e| e.exercise("en"))
}

/// Every id, in catalogue order. Used by import validation.
pub(crate) fn exercise_ids() -> impl Iterator<Item = &'static str> {
    CATALOGUE.iter().map(|e| e.id)
}

/// Gear a drill needs; `None` for an id not in the catalogue.
pub(crate) fn drill_equipment(id: &str) -> Option<&'static [Equipment]> {
    CATALOGUE.iter().find(|e| e.id == id).map(|e| e.equipment)
}

/// Entry `n` (rotating) of the kinds the swimmer can do: gear in `have`, no breath-hold entries (the coach cannot
/// know the swimmer is not alone), starts only with blocks. None if nothing fits.
pub(crate) fn pick(
    kinds: &[ExerciseKind],
    have: &[Equipment],
    blocks: bool,
    n: u32,
    lang: &str,
) -> Option<Exercise> {
    let fit: Vec<&Entry> = CATALOGUE
        .iter()
        .filter(|e| {
            kinds.contains(&e.kind)
                && e.equipment.iter().all(|g| have.contains(g))
                && e.safety != Some(BREATH_HOLD)
                && (blocks || e.kind != ExerciseKind::Start)
        })
        .collect();
    (!fit.is_empty()).then(|| fit[n as usize % fit.len()].exercise(lang))
}

#[cfg(test)]
mod tests;
