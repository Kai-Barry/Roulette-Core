# Roulette.OS — Game Design Lens Audit (pre-Phase 2)

Scope: `docs/ROULETTE_OS_GAME_BLUEPRINT.md` (design doc) + Phase 1 implementation
(`content/*.ron`, `roulette-content` schema/loader/validator). Method: Schell lens
workflow (Steps 1–8), lens numbers cited. Companion checkbox checklist:
`.agents_tmp/design_audit_checklist.md` (34 lenses, 123 questions).

---

## 1. Essential experience & theme (Lens #1, #9)

**Essential experience (stated):** *"A 3D roulette table in a cursed tavern is your
weapon: manipulate physics, board rules, and odds themselves with cheat cards to
out-gamble sinister croupiers."* Core verb: **bending odds**, not betting well on fair
odds. Pillars: physics-is-a-weapon, cheating-is-the-loop, oppressive atmosphere,
points-mode battles, readable-odds-with-corruption.

**Tetrad as implemented so far (Lens #9):**
- Mechanics — content crate carries payout/physics/economy vocabulary; physics+combat pending (Phases 2–4).
- Story/world — tavern framing lives in names/flavor strings; intact in RON data.
- Aesthetics — out of scope (CON-003); carried by content descriptions.
- Technology — headless deterministic engine: *foundational* (determinism is what makes the prediction mechanic and balancer possible), and intentional. ✔

Theme coherence is strong: curses corrupt *tools* (§8), boss wheel is *fair* (50 slots,
number pays 50× → EV exactly 0.0) while player wheels are house-edged (−2.7%), so the
only way to beat THE HOUSE is to cheat harder than it does. The economy line "chips you
extracted from the wheel" (§2.5) reinforces it. **Theme: unified.**

---

## 2. Gameplay-blocking findings (fix before/at Phase 2–4)

### 🔴 F1 — Prediction certainty is a dominant strategy (Lens #31, #33, #40)

The single most serious finding, and it lands exactly in Phase 2 (physics) + Phase 4
(damage pipeline) — perfect timing.

§5.3/§10.6: prediction is a dry-run of *the same simulation with the same seeds*, and
"prediction accuracy is real" (§5.3, §16.5). If the live spin consumes the same seeded
randomness as the dry-run, the sector **always contains the actual outcome**:

| Prediction width | Penalty (§10.1.4) | Knowledge | Bet all 30 on covered number → guaranteed PTS |
|---|---|---|---|
| 9 slots | ×0.3 | ball in 1 of 9, certain | 324 (net ≈ 2.8× stake after covering all 9) |
| 5 slots | ×0.5 | ball in 1 of 5, certain | 540 |
| 3 slots | ×0.5 | ball in 1 of 3, certain | 540 (net ≈ 13× stake) |
| **1 slot** | **×1.0** | **exact slot, certain** | **1080 (net +36× stake, zero risk)** |

The penalty ladder is inverted: it punishes *wide* (less certain) sectors and rewards
pinpoint with **no penalty at all**. `eagle_eye` (4⚡, rare, `content/cards.ron`) +
`omniscience` (5⚡, legendary, ×3 omniscience mult) then guarantee ~1080–3240 PTS per
spin against enemy pools of 30–60 per round. Only counters in the whole design are the
Fog curse (elite/boss floors only) and Faraday (magnetism only — prediction unaffected).

Worst case: pinpoint prediction + `golden_zero` (300× on 0) or `lucky_seven` (200×)
+ omniscience ×3 → ~30,000 PTS. Every balance band in §14.3 is vaporized.

**Fixes (pick one, or blend):**
1. **Decouple streams (recommended):** live spin and dry-run draw from *different*
   RNG substreams (GUD-004 already gives you `derive("physics")` vs
   `derive("prediction")`); prediction becomes a *probabilistic* band whose hit-rate
   you can tune by width. This matches "prediction band shows which slots the ball
   will *likely* land in" (Pillar 5) — the blueprint's own wording says "likely".
2. **Invert the penalty ladder:** size 1 → ×0.1–0.05 (you pay for certainty), wide →
   ×1.0. Currently §10.1.4 says the opposite.
3. Add a "prediction drift": dry-run uses launch params ± noise scaled to width.

Whatever the choice, add a Monte Carlo assertion in Phase 8: *no physics-card policy may
exceed X× baseline EV* (the §15 balancer already measures ΔEV — make it a gate).

### 🔴 F2 — Special-color bets are strictly dominated by number bets (Lens #31, #28)

On any wheel, a bet on gold pays 4×; the *number bet on the same gold slot* pays 36×
(classic). Both bets trigger the same landing; the on-land gold effects (convert
adjacent, +PTS/+⚡) fire for the wheel owner *regardless of what was bet*. So
`gold`/`purple`/`cyan` bets (4×) are dominated by number bets on their slots; even at
color level 10 (4× + 0.5×9 = 8.5×) they lose to 36×. `crimson` 6×/12× has the same
problem (12× consolation < 36× number bet on the crimson slot, which also *always*
fires the crimson landing bonus).

Players will discover this in one session and never bet special colors — a whole
signature mechanic (§4.3) becomes dead vocabulary.

**Fixes:** make on-land special effects fire only for bets *on that color*
(effect-on-win, not effect-on-land); or give special bets a bonus multiplier the
number bet doesn't get (e.g. gold bet also ×2 on-land PTS); or reduce number payout
for special-colored slots. Cheap to encode now in `PayoutTable` (Phase 3, TASK-019).

### 🟠 F3 — Spec ambiguity: which pool is the score? (Lens #21)

§16.3 `BattleState` carries **both** `playerScore=30` and `chipsPool=30`; §10.7 says
"chipsPool is the live score" but also "playerScore is the base score… changed only by
Sacrifice"; §2.5 says battle chips "carry out via the shared RunState.chips pool — the
same pool used for combat betting" (implying run-economy snowball: one 1080-PTS win =
run-wide wealth), while §3.1 says the battle pool starts at 30 *per battle* (implying
it does NOT carry out intact). §3.2 says stakes come from the pool and payouts return
to it, so the score is simultaneously your bankroll (elegant!) — but then §10.7's
separate `playerScore` is vestigial state.

This will fork implementations in Phase 4/6. **Decide now and pin in a test**
(TEST-013 exists for base scores 30/30 — extend it with: *what exactly carries out of
combat into RunState.chips, and what wins the battle: chipsPool or playerScore*).

### 🟠 F4 — Enemy HP is vestigial in points mode (Lens #21)

§10.7/§3.3: battles are decided by score at the round limit; the player's HP drains
from intents/curses, but **enemy HP (50/40/60/95/220) is never decremented by any
points-mode rule** — yet `enemies.ron` carries `max_hp` and the boss's 220 is treated
as a fidelity marker. Either (a) points mode should have a score-damage path to enemy
HP (e.g. score > enemy lead by X → HP damage), or (b) mark `max_hp` legacy/damage-mode
in schema docs. Right now it's state that never matters → confusion for future
contributors and balance tooling.

### 🟠 F5 — Data-vs-blueprint drift (Lens #20 judgment; fidelity risk)

Phase 1 content is 97% faithful (verified programmatically: all 6 wave-1 wheel payout
tables exact; all 100 comparable cards match blueprint cost+rarity **0 mismatches**;
curses/wheels/boss numbers correct). Exceptions:

| Item | Authored | Blueprint | Section |
|---|---|---|---|
| `dealers_claw` difficulty | **0.6** | **0.85** | §14.2 |
| `compound_interest` card | missing (effect also missing from DSL) | UC, +50% pool, in §6.4 hook list | §6.3/§6.4 |
| `number_duplicate`, `number_swap`, `slot_expansion`, `onyx_void`, `emerald_dream` | missing (wave-2, acceptable per CON-004) | — | §6.3 |

The elite difficulty 0.6 vs 0.85 changes elite feel materially (0.6 = optimal line 60%
of the time). Either fix data to 0.85 or record a deliberate deviation.

---

## 3. Balance-area findings (Lens #28–#46)

- **Skill vs. chance (#35):** good *shape* — chance (37 slots) compressed by skill
  (prediction, magnet bias, nudge) into near-certainty, with curses re-randomizing on
  elite floors. But that compression is currently total (F1): no chance survives
  prediction, so "skill" degenerates into one lookup. Tuning target: keep pinpoint
  prediction ~85–95% accurate, not 100%.
- **Economy feedback loops (#29, triangularity #33):** payout-as-chips + carry-out
  = positive feedback (win → more chips → bigger bets → bigger wins). Balatro
  deliberately rides this; with score=battle-local (pool resets to 30 per fight) the
  runaway is fight-scoped and fine. **If** pools carry out intact (F3 ambiguity), the
  run-level economy runs away after one good fight. Pick fight-scoped + carry a
  *bounded* extraction (e.g. min(pool, 20)+X) — preserves "beat the table" fantasy
  without infinite wealth.
- **Rewards (#39):** card rarity table 3/9/28/60 over a 107-card pool where uncommon
  = 45% of pool — the modal reward is fine. 6 unported cards are all UC/R — no gap
  for wave-1 balance.
- **Punishment (#40):** enemy intents hit HP while battles are scored in PTS →
  punishment channel is orthogonal to score (good tension), but player has *no*
  defense axis except stuns (purple/stun cards) and gold heals; verify HP drain rate
  in Phase 8 sim (avg intents/round × values 3–20 × rounds 3–8 vs 80 HP + ~4.4 HP
  per Blood Infusion purchase at 12⚡).
- **Avarice curse (#28):** flat 3⚡ per draw replaces 0/3/5/7/9 ladder — strictly a
  buff from the 4th draw onward (3 < 7). A curse that becomes a buff is a design
  smell; consider flat 3 *plus* losing the free first draw only (already implied) —
  then it's a buff only from draw 5. Acceptable, but note it.

## 4. Interface & interest (Lens #48–#60, #61–#70)

- Out of scope this phase (CON-003), but two forward notes: (a) **prediction band
  rendering** must make *uncertainty* legible (Pillar 5); with F1 fixed, band width =
  confidence — design the visual (color intensity) accordingly in the web phase.
  (b) `format_description` points-mode rewriter (§6.5) is specified and cheap — keep
  it in the effect-DSL query layer (Phase 5, TASK-034), not in content files.
- Interest curve (#71): 7 floors × (3/5/8-round fights) is a tight, rising curve with
  elite curse spikes at floor 3 and finale boss — good shape. One flat-segment risk:
  floor 0 has **3 identical combat nodes, no choice** (§2.3) — intentional onboarding
  (Lens #48 easy start), fine.

## 5. Process (Lens #89–#100)

- Eight filters: engineering ✔ (tests+clippy green, determinism planned), playtesting ✗
  (no playable loop until Phase 7 CLI — mitigated by the Phase 8 Monte Carlo acting as
  automated playtesting; make win-rate-by-floor a reported metric so difficulty curve
  is measurable, not assumed).
- RISK-001 (physics tuning) and F1 interact: golden tests must assert *statistical*
  bias effects (TEST-003 ≥5k spins) — already planned; add an assertion that
  prediction-sector hit-rate < 100% at every width.
- Validator gap found this session: `value <= 0.0` passes **NaN** (`nan` parses in RON
  f32). Add `is_finite()` checks to `validate.rs` payout/effect checks before wave-2
  data entry multiplies the surface (5-line fix; REQ-010).

---

## 6. Prioritized fix list

| # | Fix | Phase | Cost |
|---|---|---|---|
| 1 | F1: prediction determinism decision — decouple live/dry-run RNG streams OR invert penalty ladder; encode as golden+statistical tests | Phase 2 (TASK-016) | medium (design decision now, code small) |
| 2 | F3: pin the score/pool/carry-out semantics in one test (extend TEST-013) | Phase 4 (TASK-024) | small |
| 3 | F2: special-color bet dominance — effect-on-win for color bets or bet-specific bonuses | Phase 3 (TASK-019) | small |
| 4 | F4: declare enemy `max_hp` damage-mode-only in schema docs or wire a score→HP path | Phase 4 | trivial (docs) |
| 5 | F5: elite difficulty 0.6→0.85 to match §14.2 (or record deviation) | now | trivial |
| 6 | Validator: reject NaN/non-finite floats in payouts/effects | now | trivial |
| 7 | Port `compound_interest` (+ `CompoundInterest` DSL variant or GrantChips-percent) | Phase 9 wave-2 | trivial |
| 8 | Phase 8: add EV-dominance gate (no card/policy > band ceiling) to balancer | Phase 8 | medium |

**Verdict:** the design is coherent, unusually well-specified, and theme-unified; the
one structural threat is prediction certainty (F1) — everything else is data polish.
Phase 2 is the right moment: the fix is cheapest there and the blueprint's own language
("will *likely* land") already points the intended direction.