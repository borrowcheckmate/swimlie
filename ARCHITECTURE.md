# Architecture

Open source, local-first replacement for MySwimPro. iOS app + Apple Watch companion first, a self-hosted Docker server later.

## 1. Architecture

```
┌──────────────────────────── swimlie-core (Rust) ────────────────────────────┐
│ model     Workout, Block, Set, Session, Lap   (serde, versioned JSON)        │
│ engine    live set state machine: laps/distance/time in → "where am I" out   │
│ metrics   pace/100, SWOLF, stroke rate, CSS, training load, PRs              │
│ store     SQLite (rusqlite, bundled), UUID ids + updated_at (sync-ready)     │
│ coach     rule-based workout generator (templates × goal × CSS pace)         │
│ io        JSON import/export, validation against schema                      │
└──────────────┬───────────────────────────────┬───────────────────────────────┘
               │ UniFFI → XCFramework          │ plain Rust crate (later)
       ┌───────┴────────┐               ┌──────┴─────────────────┐
       │ Swift: iOS app │◄─WatchConn.─► │ Swift: watchOS app     │   server: axum + MCP (Docker)
       │ library, import│  (JSON blobs) │ HealthKit live session │
       │ history, charts│               │ schedule + rest UI     │
       └────────────────┘               └────────────────────────┘
```

Rules:
- **Rust owns all data and all calculations.** Swift only does UI, Apple APIs (HealthKit, WatchConnectivity, files) and forwards events to the core.
- **Local SQLite on each device.** Phone ↔ watch sync is JSON over WatchConnectivity. No cloud.
- **The core is pure, with no platform code**, so the same crate runs in the Docker server later without changes.

## 2. Workout format

The contract for AI apps, MCP and the coach. Published as `schema/workout.schema.json` and versioned like a public API.

```json
{
  "version": 1,
  "name": "Threshold 2k",
  "pool_length_m": 25,
  "blocks": [
    { "name": "Warm-up", "repeat": 1, "sets": [
      { "reps": 1, "distance_m": 400, "stroke": "any", "intensity": "easy" } ] },
    { "name": "Main", "repeat": 2, "sets": [
      { "reps": 4, "distance_m": 100, "stroke": "freestyle", "intensity": "threshold",
        "rest": { "type": "interval", "seconds": 105 } },
      { "reps": 1, "distance_m": 200, "stroke": "backstroke", "intensity": "easy",
        "rest": { "type": "fixed", "seconds": 30 } } ] }
  ]
}
```

- `goal_s` (optional, per set): target time per rep in seconds.
- `drill` (optional, per set): an exercise id from the catalogue (`exercises()` in `core/src/exercises.rs`: drills, wall skills, starts, fun games; each with a watch cue, an iPhone how-to and a YouTube search link). The import rejects unknown ids; stored and synced workouts keep them.
- `rest.type`: `interval` = start the next rep every N seconds; `fixed` = rest N seconds after finishing the rep.
- `stroke` uses HealthKit's stroke styles (`freestyle`, `backstroke`, `breaststroke`, `butterfly`, `mixed`, `kickboard`) plus `any`, so planned and detected strokes can be compared.

**Plans** (`schema/plan.schema.json`, example `schema/plan-example.json`): `{ "version": 1, "name", "weeks": [{ "focus"?, "workouts": [Workout, …] }] }`. Weeks, not dates: no calendar. Stored in the `plans` table (migration v4, synced like workouts). Each plan workout is sent to the watch under the fixed id `<plan id>/<week>/<index>` (`plan_workout_id`, 0-based), so the swum session's `workout_id` marks it done; the iPhone tints the first one not done as next. `parse_import` tells a plan (top-level `weeks`) from a workout.

Ways to get a workout in (all produce this JSON):
1. **Import:** paste, share sheet or a `.json` file, plus a ready-made prompt for AI apps. Comes first.
2. **Coach:** rule-based generator in Rust from goal, available time and CSS pace.
3. **MCP:** `create_workout`, `list_sessions`, `get_stats`. Runs on the server, because a phone can't host an MCP server for desktop AI clients.

## 3. Watch: measuring and following the schedule

**Native Apple APIs only (for now):**
- `HKWorkoutSession` (activity `.swimming`, location `.pool`, lap length) + `HKLiveWorkoutBuilder`.
- Data comes from the builder: `distanceSwimming`, `swimmingStrokeCount`, lap events (`HKWorkoutEvent .lap`) and segment events, with the stroke style in `HKMetadataKeySwimmingStrokeStyle`.
- `enableWaterLock()` once the session reports `.running` (earlier calls are ignored).

**Set engine (Rust, testable without a pool):**

```
engine.on_lap(Lap { start_s, end_s, stroke, strokes }, now) / engine.manual_next(now) / engine.tick(now)
      → State { pos: {block, round, set, rep}, done_m, target_m, phase: Swimming|Resting|Done,
                rest_remaining_s, last_rep_pace_s_per_100m, rep_elapsed_s, swum_m }
engine.session() → Session { workout, started_at_ms, laps, lap_positions, lap_lengths }
```

- **Laps carry their own start and end time** (seconds since the session started), so a late lap is placed by when it was swum, not by when it arrived. Distance samples are not used: nothing shows they arrive sooner than laps. `strokes` is HealthKit's `swimmingStrokeCount` for the lap, `None` when it gives none (kickboard); the engine ignores it, metrics use it.
- **Alignment, not counting:** after every lap the engine re-splits all laps so far into the planned reps (a small dynamic program). Costs: a planned length without a lap 1, a length beyond the plan 2, a rest-sized gap (≥ 20 s) inside a rep 1 per 20 s, a rep change without a 10 s rest 0.5, a clear stroke contradiction (free/back/breast/fly vs. a different planned one) 0.5. A missed length then costs one short rep instead of shifting every rep after it. The rep being swum is never penalised for lengths not swum yet.
- **Start countdown:** a planned 10 s rest before the first rep (`Resting`, `pos` = rep 1); the first lap or `manual_next` ends it early. `rep_elapsed_s` = seconds since the current rep started (`None` while resting/done); `swum_m` = metres swum so far.
- **Merged lap:** a lap ≥ 1.7× the median length duration of the last 8 laps is slow. A slow lap between two normal ones counts as two lengths; a run of slow laps (kick, drill) is a slow stroke and counts one length each. The next lap can overturn the call, so lengths are re-decided with every lap.
- **Stray laps:** a lap shorter than a 100 m pace of 40 s (10 s for 25 m, scaled by pool length) is a HealthKit artefact. The engine drops it: it is not in the session (`laps`, `lap_positions` and `lap_lengths` stay in step) and does not move the state.
- **Rep finishes** when its lengths reach the target (distance / pool length).
- **Rest:** `interval` counts down to the next send-off (from the rep's first lap start); `fixed` counts down from rep end. While resting, `pos` is the next rep. Haptics at 5 s and at 0.
- **Next rep starts** on the first new lap after rest, or when the rest timer ends (whichever comes first). With no planned rest that is straight away. Any extra time is logged as rest.
- **Unplanned stop:** `Resting` once the next lap is 20 s overdue (last lap end or rest end + typical length + 9 s lap delay + 20 s). Rest from the gap is all the engine has live (segments are too late).
- **Manual override:** `manual_next` closes the rep on screen ("done" while swimming, "skip" while resting). Laps swum before the press stay in that rep. Water lock disables the touchscreen, so skip/next goes on the Digital Crown (turn it past the gauge). No other manual input is planned while swimming: rests and pauses are detected automatically. The one exception is a deliberate pause (e.g. a toilet break, so motion continues): crown + side button, which HealthKit delivers as a `pauseOrResumeRequest` event.

**What HealthKit gives (measured in the pool, 25 m, Apple Watch Series 8):** recordings of three swims are kept as replay fixtures in [`core/tests/data/`](core/tests/data/) and drive `core/tests/engine_replay.rs`.
- **Lap delay:** laps arrive 5 s (minimum) to 9 s (maximum) after the wall. Good enough for a live display, never instant.
- **Segment delay:** 37 s to 394 s. Useless live, so the engine detects rests from gaps between laps itself.
- **Regular strokes are reliable:** every length of a plain freestyle main set was detected, and freestyle, breaststroke and backstroke were labelled correctly.
- **Drills are not:** lengths go missing at the start and in drill sets, one-arm drills can read as `kickboard`, and a catch-up drill counts as freestyle.
- **Slow strokes:** a kick set arrives as laps of 42 to 127 s, so "slow" cannot simply mean "two lengths" (see the merged-lap rule above).
- **Stray laps:** a 3.5 s lap that arrives minutes late is a HealthKit artefact, dropped by the 40 s per 100 m floor.
- **Long gaps:** HealthKit can miss whole easy pieces, and the engine cannot tell that from a long rest.
- **Replay accuracy:** fixture A: 67 of 70 labelled laps get the right rep live, 66 of 70 after the whole session. Fixture B: 38 of 38 live and final. Fixture C (two 4×25 sets of one length per rep): 44 of 44 live and final; the one-length reps land in the right rep, but the screen lags them by the lap delay.

## 4. Repo layout

```
swimlie/
  core/            Rust crate: all logic and tests, exported to Swift with UniFFI
    src/             one module per concern; a module that outgrows a file becomes a folder of submodules
    tests/           engine replays over recorded swims (tests/data)
  xtask/           repo automation: builds, tests, devices, screenshots, lint (`cargo xtask`)
  schema/          workout.schema.json, plan.schema.json and the AI prompt template
  apple/           XcodeGen spec, iOS app, watchOS app, SwimlieCore Swift package (wraps the XCFramework)
  docs/            README images and recorded AI replies
```

Files stay under 600 lines (`cargo xtask lint` enforces it): a longer file is split into submodules, with its tests in a sibling `tests` module.

## 5. Coach

Rule-based, pure Rust (`core/src/coach.rs` and `core/src/coach/`). `generate_plan` and `build` are deterministic (same inputs, same output). `generate_workout` is shuffled: a random template variant, exercise seed and set order per call (same goal and distance), so the creator's shuffle button gives another swim.

**Creator choices** (`CreatorInputs`, single swims only; plans ignore them):
- `rest_percent` (-50..+50): scales the rest of timed sets (to 5 s); goals do not move.
- `strokes`: the main sets are spread over the chosen strokes (IM only on multiples of 4 lengths, butterfly ≤ 100 m and not hard); one stroke leaves no other swim stroke, warm-up build set included. Empty = the coach picks.
- **Rounds**: a main block with `repeat` = N, one of six shapes (stroke round, ladder, pyramid, descend, fast-slow-fast, long to short), named `Main: <shape>`. Used when ≥ 2 strokes are chosen, and on ~half of the shuffles with none chosen (D ≥ 1000 m, no gear, no starting blocks, not easy recovery). Never for technique. N fits ~65% of D (warm-up stays ≤ 1000 m); no end skill set, no sprint-rep cap. The watch shows "Round x/N", both apps a "Repeat N×" pill and an accent bar.

**CSS test** (bundled `apple/Watch/Workouts/css-test.json`, 25 m): warm-up 1×400 easy + 4×50 build, 1×400 freestyle sprint (fixed rest 300 s), 1×200 freestyle sprint, cool-down 1×200 easy.
- `metrics::css_test_times(session) -> Option<CssTest { t400_s, t200_s }>`: a session is a CSS test when its workout has a set with `reps` 1, block `repeat` 1, freestyle, `sprint`, 400 m, and one like it of 200 m. The times are the `swim_s` of those complete reps (all lengths swum); `None` if a trial is missing or incomplete or `css_s_per_100m` rejects the times. Structural, so a copied or AI-made CSS test counts too.
- `Store.import_css_tests() -> u32`: for every live session that is a CSS test and has no `css_results` row with the session's id (tombstones count, so a deleted result stays deleted), save the CSS under that id with `created_at` = the session's start. The iPhone calls it after applying a sync and sends the store back if it saved any; the watch only receives CSS rows.

**Paces** (s/100 m) = CSS × intensity × stroke: intensity easy ×1.15, moderate ×1.08, threshold ×1.02, hard ×0.96, sprint ×0.88; stroke backstroke ×1.10, butterfly and mixed (IM) ×1.12, breaststroke ×1.20, freestyle/any/kick ×1. Usable CSS = finite and > 0. Without one: 120 s/100 m (`metrics::css_pace`, the estimate fallback), and the first warm-up set's notes say to swim the CSS test (`NO_CSS_NOTE`). Coach paces for no CSS come from the level, see below, and the first warm-up set's notes say to swim the CSS test (`NO_CSS_NOTE`).
- `goal_s` = pace × distance / 100, rounded, on main swim sets (not on warm-up, cool-down, kick or drill sets).
- Rest per rep on sets with a goal: `interval` = goal + rest, rounded up to 5 s, rest = 10 s + 5 s per 100 m (hard: 15 s + 10 s per 100 m). A 1-rep set gets `fixed` = rest, rounded up to 5 s, instead. Sprints get `fixed` = goal clamped to 20–60 s, rounded up to 5 s (about 1:1). Warm-up, cool-down, kick and drill sets get `fixed` 15–20 s.

**generate_workout(CreatorInputs, SwimmerProfile) -> Workout**: `kind` maps to a type (lower-cased, trimmed): `easy recovery`/`easy`/`recovery`, `endurance`, `threshold`, `speed`/`sprint`, `technique`/`drill`, anything else `mixed`.
- Target distance D: `target_distance_m`, else from `target_time_min` (below), else 2000 m. Clamped to 400–8000 m.
- Shape: warm-up block (1×W easy any, plus 4×50 build moderate for threshold/speed when D ≥ 1500), main block (the type's template), cool-down block (1×C easy any).
- Every distance is a multiple of the pool (`round_to_pool`: nearest multiple, at least one length). Main nominal ≈ 65% of D: each template set has a nominal rep distance and a share of the main volume; reps = max(1, round(share × main / rep)). Sprint sets stop at 8 reps; their lost metres go to the 100 moderate set.
- C = 10% of D, at least two lengths and at most 300 m. Main target = 65% of D less the end skill set. W = D − main − build − skill sets − C, rounded to the pool. The easy warm-up keeps a floor of max(10% of D, two lengths), rounded down to the pool: below it, the warm-up skill set gives way first (4 → 2 → none), then the end skill set (4 → 2 → none), and only then a rep comes out of the largest main set.
- Fill: while W − rep ≥ max(10% of D, two lengths), add one rep to the non-sprint main set furthest below its share whose rep fits. Then W above 1000 m is capped at 1000 m, and the excess goes to main the same way. So a distance target is hit within half a length.
- Time target T: bisection on D between 100 and 8000 m, keeping the D whose `estimated_duration_s` is closest to T. The estimate steps with rep counts, so D can fall below 400 m (short sprint-heavy swims).
- Templates (nominal rep m: stroke, intensity, share):
  - easy recovery: 200 free easy .5, 100 backstroke easy .25, gear slot .25 (pull 100 easy with pull buoy (+paddles), else kick 50 with kickboard, else 100 free easy).
  - endurance: 400 free moderate .6, 200 .4 as a pull set with pull buoy (+paddles) if given, else free moderate.
  - threshold: 100 free threshold .7 + 200 free moderate .3 (plan variant 1: 200s threshold; variant 2: 300s threshold).
  - speed: 50 free sprint .35 (with fins if given), 100 free moderate .4, 25 free sprint .25. From pool 50 the 25 is dropped and 50 sprint is .6.
  - technique: two drill slots, each 50 free drill easy .15 (catalogue drills for seeds 2s and 2s+1; snorkel added on freestyle drills if given, not head-up), 50 kick .25 (stroke `kickboard` with kickboard, else stroke `any` "kick on your back"; fins if given), 100 free moderate .45 ("count strokes").
  - mixed: IM (`mixed`) moderate .3, 50 backstroke easy .2, 50 breaststroke moderate .2, 100 free threshold .3; a kickboard adds kick to the backstroke slot. The IM rep is the nearest multiple of 4 lengths (100 m at 25 m, 132 at 33 m, 200 at 50 m, 80 at 20 m).
- Only equipment from the inputs is used. Name: `<Type> <total> m`.
- **Exercises** (catalogue in `core/src/exercises.rs`, picked by `pick`; the entry is the seed mod the candidates, `variant` only changes threshold):
  - Filter: kind wanted, all its equipment in the inputs, no breath-hold: only the `BREATH_HOLD` entries are excluded (the dive has its own safety note and is picked only with starting blocks), `Start` only with starting blocks.
  - Exercise sets use `stroke: any`: the drill names its stroke, and the engine does not flag a stroke mismatch. A wall or start rep is at least two lengths (a flip turn needs a turn; a dive ends back at the block end: 50 m in a 25 m pool, 100 m in a 50 m pool).
  - Technique drills: two slots from seeds 2s and 2s+1, each 1×50 easy. Catch-up is always a drill with no gear, so a slot always has a pick.
  - Warm-up skill: D ≥ 1000 m, 4 reps of a drill or wall skill (technique: wall only), after the easy swim, from seed s. Its metres come out of the warm-up, and it gives way first below the warm-up floor.
  - End skill: every type but technique, D ≥ 1500 m, 4 reps after the timed main sets, fixed 20 s rest, from seed s. Its metres count against main. Easy recovery: a fun game only. Speed with blocks: the start. Otherwise a fun game or a start.
  - Records: exercise reps never become personal records (`record_key` and `personal_records` skip sets with a drill).
  - Ids: the import checks drill ids against the catalogue. Stored and synced workouts keep unknown ids, since a phone and a watch can run different catalogues.
  - Known misses (tests allow them): endurance 500 m in a 50 m pool (main 40%: a 400 m rep would leave under two lengths each for warm-up and cool-down); speed in a 25 m pool up to 600 m sprint (the 25 m sprint stays there).

**generate_plan(CreatorInputs, SwimmerProfile) -> Plan**: weeks = `plan_weeks` (default 4, 1–16), swims = `swims_per_week` (default 3, 1–7).
- Week-1 volume V1 = `profile.weekly_m_28d` if ≥ 500 m, else swims × the per-swim target (distance, or the distance `generate_workout` picks for the time target, else 1500 m).
- Every 4th week (4, 8, 12, 16) is lighter: 70% of the week before. The other weeks build: V = V1 × min(1.07^k, 1.5), k = number of earlier build weeks (so week 5 continues from week 3; growth stops at 1.5 × V1).
- Distance per swim: V / swims (clamped to 400–8000 m). With a time target and no history, each swim instead gets the distance `distance_for_time` picks for its own type at T × V/V1 minutes (T = target minutes), so every swim takes its share of the time (not clamped).
- Types rotate: the requested type first, then endurance, technique, threshold, speed, mixed, easy recovery (no repeat until all are used; neighbours always differ); a lighter week uses easy recovery, technique, endurance, mixed. Template variant = week index, but only threshold has week-to-week variants (its second set changes); other types repeat their main-set shape, and their sizes still change with volume. Exercise seed = (week − 1) × swims + swim index (from 0), so exercises rotate within a week too.
- Week focus: "Base" (week 1), "Build", or "Lighter week". Name: `<weeks>-week <type> plan`, plus ` (default pace)` without a usable CSS (its workouts still carry the note).
