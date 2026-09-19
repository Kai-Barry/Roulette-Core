# Design decisions (ADR log)

Normative source for mechanics is `docs/ROULETTE_OS_GAME_BLUEPRINT.md`; decisions here
**override** blueprint text when they conflict. Each entry cites origin.

---

## DEC-001 — Prediction is probabilistic, never certain (2026-09-14)

**Origin:** game-design-lens audit F1 (`.agents_tmp/design_audit.md`), approved by user.

**Context.** Blueprint §5.3/§10.6 state the prediction dry-run uses *the same seeds* as
the live spin, and "prediction accuracy is real". §10.1(4) then applies a **penalty
ladder that rewards narrow sectors with *no* penalty** (size 1 → ×1.0, size 9 → ×0.3).
Together these make a size-1 prediction **certain knowledge of the winning slot**:
bet the whole pool on that number (36×) → guaranteed ~1080 PTS per spin vs enemy pools
of 30–60; with `omniscience` (×3) and `golden_zero` (300×) up to ~30,000 PTS. A
dominant strategy that voids every §14.3 balance band; only the Fog curse (elite/boss
floors only) counters it.

**Decision (refined during Phase 2 implementation).** Prediction is probabilistic,
never certain. The initially chosen mechanism — purely decoupled RNG substreams —
proved insufficient in practice: the spin is chaotic, so *any* independent launch
jitter diverges completely (measured size-1 hit rate ≈ 0.034 ≈ chance), making
prediction useless rather than merely uncertain. The implemented model:

- The prediction dry-run runs the full §5 simulation on the **live spin stream**
  (`derive("physics")`) — same spin parameters, same launch jitter, same outcome.
- An **oracle-honesty roll** on the independent `derive("prediction")` stream decides
  (probability `PhysicsModifiers::prediction_accuracy`, default **0.90**) whether the
  reported band is centered on the **true** landing or on a **plausible decoy** (an
  independent dry-run's landing — a physically plausible wheel outcome).
- Therefore: hit rate is a direct, tunable design knob; it is always < 100% while
  `prediction_accuracy < 1.0`; content may not set it to 1.0 (validator policy,
  Phase 5). A size-9 band is strictly more informative than size-1 (honest bands
  contain the truth; a lie covers 9/37 vs 1/37 of the wheel), which preserves the
  §10.1(4) width-pricing ladder.

This matches blueprint Pillar 5's own wording — the band "shows which slots the ball
will **likely** land in" — and keeps every mechanic meaningful: prediction remains a
real dry-run, Fog still hard-counters it, Faraday still kills magnetism.

**Consequences.**
- TASK-016 builds `predict()` on the oracle model; REQ-003 wording ("identical
  seeds") is amended to "identical spin parameters; honesty roll on a separate
  substream".
- The §10.1(4) penalty ladder stays as written (punishing wide sectors is now
  meaningful: wide = *cheaper information*, narrow = *better information, paid in card
  cost and still uncertain*).
- `prediction_never_matches_with_certainty` asserts hit-rate ≈ 0.90 ± 0.07 and < 100%
  at every width; `prediction_accuracy_knob_bounds_hit_rate` asserts the knob bounds
  the rate directly; Phase 8 balancer adds an EV-dominance gate (no prediction policy
  may exceed its band ceiling).
- Fog curse still hard-counters prediction; Faraday still kills magnetism.

**Rejected alternatives.** Inverting the penalty ladder (size 1 → ×0.1) alone: keeps
the "psychic certainty" fantasy, which breaks immersion in a physics-cheating game.
Pure stream-decoupling (tried first): chaotic divergence makes the band worthless
(hit ≈ chance), which betrays Pillar 5's promise of a *useful* prediction. The
oracle-honesty model delivers "informative but never certain" with one tunable
parameter.

---

## Related pending decisions (not yet made)

- **F3:** which pool wins battles (`chipsPool` vs `playerScore`) and what carries out
  of combat into `RunState.chips` — must be pinned before Phase 4 (TASK-024).
- **F2:** special-color bets are dominated by number bets on the same slot — fix in
  Phase 3 (TASK-019) via effect-on-win or color-bet-exclusive bonuses.
- **F4:** enemy `max_hp` is vestigial in points mode — document as damage-mode-only or
  wire a score→HP path (Phase 4).