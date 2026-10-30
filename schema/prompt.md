# Prompt for AI apps

Paste this into ChatGPT, Claude or similar, then import the JSON it returns into Swimlie.

---

Create a swim workout as JSON (or a multi-week training plan, if my request asks for one) that validates against the schema below. Reply with only the JSON.

Rules:
- `version` is 1.
- Every `distance_m` must be a multiple of `pool_length_m`.
- Every set needs a `rest`, warm-up, cool-down, kick and drill sets too. `rest.type` is `interval` (start the next rep every N seconds) or `fixed` (rest N seconds after the rep). Pick it like the built-in coach: a set with `goal_s` and 2 or more reps gets `interval` = goal_s + rest; a single rep gets `fixed` = rest; rest = 10 s + 5 s per 100 m (hard: 15 s + 10 s per 100 m), rounded up to a multiple of 5 s; a sprint gets `fixed` = goal_s clamped to 20–60 s; warm-up, cool-down, kick and drill sets get `fixed` 15–20 s.
- `goal_s` (optional) is the target time per rep in seconds.
- `stroke`: any, freestyle, backstroke, breaststroke, butterfly, mixed, kickboard.
- `intensity`: easy, moderate, threshold, hard, sprint.
- `equipment` (optional): fins, paddles, pull_buoy, kickboard, snorkel.
- `drill` (optional): an exercise id for drill, wall-skill, start or fun sets. Put the coaching cue in `notes` too. Ids: `catch-up`, `fingertip-drag`, `single-arm`, `six-kick-switch`, `fist-swim`, `zipper`, `head-up`, `front-scull`, `breaststroke-2-kicks-1-pull`, `backstroke-single-arm`, `backstroke-spin`, `streamline-push-off`, `roll-off-wall`, `flip-turn`, `open-turn`, `dive-and-swim`, `touch-the-floor`, `fewest-strokes`, `golf`, `kick-race`, `breathe-every-5th`.
- No breath-hold, hypoxic or underwater-distance sets unless my request asks for them; if it does, keep them short and put "never alone, never hyperventilate" in `notes`. Never write dives unless my request says starting blocks/dives are allowed.
- A plan has `version` 1, a `name` and `weeks`; each week has an optional `focus` and its `workouts`, each a full workout that follows these rules. Build up gradually from week to week.

My request: <describe the workout: goal, duration, pool length, level>

Schema: <paste schema/workout.schema.json>

Example: <paste schema/example.json>
